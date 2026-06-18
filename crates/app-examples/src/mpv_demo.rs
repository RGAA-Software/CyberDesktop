use std::path::PathBuf;

use app_mpv_ffi::MpvEmbedPlayer;
use app_ui::TitleBar;
use gpui::{
    div, prelude::*, px, App, Bounds, Context, ParentElement, Pixels, Render, SharedString, Styled,
    Window,
};
use gpui_component::{
    alert::Alert, button::Button, h_flex, label::Label, v_flex, ActiveTheme as _, ElementExt,
};
use raw_window_handle::RawWindowHandle;
use rfd::FileDialog;
use tracing::{error, info, warn};

#[cfg(windows)]
use windows::core::w;
#[cfg(windows)]
use windows::Win32::Foundation::{HWND, POINT};
#[cfg(windows)]
use windows::Win32::Graphics::Gdi::ClientToScreen;
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, MoveWindow, ShowWindow, SW_HIDE, SW_SHOW, WS_BORDER, WS_CHILD,
    WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP, WS_VISIBLE,
};

const VIDEO_MIN_HEIGHT: f32 = 420.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostMode {
    Child,
    Overlay,
}

impl HostMode {
    fn label(self) -> &'static str {
        match self {
            Self::Child => "WS_CHILD",
            Self::Overlay => "Overlay",
        }
    }

    fn subtitle(self) -> &'static str {
        match self {
            Self::Child => "Traditional child HWND embedded in the GPUI window.",
            Self::Overlay => "Owned popup overlay pinned to the GPUI video bounds.",
        }
    }
}

#[cfg(windows)]
fn window_hwnd(window: &Window) -> Option<isize> {
    let handle = raw_window_handle::HasWindowHandle::window_handle(window).ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(window) => Some(window.hwnd.get() as isize),
        _ => None,
    }
}

#[cfg(not(windows))]
fn window_hwnd(_window: &Window) -> Option<isize> {
    None
}

#[cfg(windows)]
struct NativeVideoSurface {
    hwnd: HWND,
    mode: HostMode,
    parent_hwnd: HWND,
}

#[cfg(windows)]
impl NativeVideoSurface {
    fn new(parent_hwnd: isize, mode: HostMode) -> anyhow::Result<Self> {
        let (ex_style, style, title) = match mode {
            HostMode::Child => (
                Default::default(),
                WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN | WS_CLIPSIBLINGS | WS_BORDER,
                w!("MPV CHILD"),
            ),
            HostMode::Overlay => (
                WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                WS_POPUP | WS_VISIBLE | WS_CLIPSIBLINGS | WS_BORDER,
                w!("MPV OVERLAY"),
            ),
        };

        let hwnd = unsafe {
            CreateWindowExW(
                ex_style,
                w!("STATIC"),
                title,
                style,
                0,
                0,
                1,
                1,
                HWND(parent_hwnd as _),
                None,
                None,
                None,
            )
        }?;

        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOW);
        }

        info!(
            parent_hwnd,
            child_hwnd = hwnd.0 as isize,
            mode = mode.label(),
            "created native video host"
        );

        Ok(Self {
            hwnd,
            mode,
            parent_hwnd: HWND(parent_hwnd as _),
        })
    }

    fn hwnd(&self) -> isize {
        self.hwnd.0 as isize
    }

    fn set_visible(&self, visible: bool) {
        unsafe {
            let _ = ShowWindow(self.hwnd, if visible { SW_SHOW } else { SW_HIDE });
        }
    }

    fn set_bounds(&self, window: &Window, bounds: Bounds<Pixels>) {
        let scale = window.scale_factor();
        let mut left = (f32::from(bounds.origin.x) * scale).round() as i32;
        let mut top = (f32::from(bounds.origin.y) * scale).round() as i32;
        let mut right =
            ((f32::from(bounds.origin.x) + f32::from(bounds.size.width)) * scale).round() as i32;
        let mut bottom =
            ((f32::from(bounds.origin.y) + f32::from(bounds.size.height)) * scale).round() as i32;

        if self.mode == HostMode::Overlay {
            let mut origin = POINT { x: left, y: top };
            unsafe {
                let _ = ClientToScreen(self.parent_hwnd, &mut origin);
            }
            let width = right - left;
            let height = bottom - top;
            left = origin.x;
            top = origin.y;
            right = left + width;
            bottom = top + height;
        }

        if right <= left || bottom <= top {
            self.set_visible(false);
            return;
        }

        self.set_visible(true);
        info!(
            child_hwnd = self.hwnd.0 as isize,
            mode = self.mode.label(),
            left,
            top,
            right,
            bottom,
            scale,
            "move native video host"
        );
        unsafe {
            let _ = MoveWindow(self.hwnd, left, top, right - left, bottom - top, true);
        }
    }
}

#[cfg(windows)]
impl Drop for NativeVideoSurface {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

pub struct MpvDemo {
    host_mode: HostMode,
    player: Option<MpvEmbedPlayer>,
    #[cfg(windows)]
    native_surface: Option<NativeVideoSurface>,
    video_bounds: Option<Bounds<Pixels>>,
    opened_path: Option<PathBuf>,
    playing: bool,
    ended: bool,
    status: SharedString,
}

impl MpvDemo {
    pub fn view(_window: &mut Window, cx: &mut App) -> gpui::Entity<Self> {
        cx.new(|_cx| Self {
            host_mode: HostMode::Child,
            player: None,
            #[cfg(windows)]
            native_surface: None,
            video_bounds: None,
            opened_path: None,
            playing: false,
            ended: false,
            status: "Open a video file to compare WS_CHILD and Overlay hosts.".into(),
        })
    }

    #[cfg(windows)]
    fn ensure_native_surface(&mut self, window: &Window, cx: &mut Context<Self>) -> Option<isize> {
        let parent_hwnd = window_hwnd(window)?;

        let recreate = self
            .native_surface
            .as_ref()
            .map(|surface| surface.mode != self.host_mode)
            .unwrap_or(false);
        if recreate {
            self.native_surface = None;
            self.player = None;
        }

        if let Some(surface) = self.native_surface.as_ref() {
            return Some(surface.hwnd());
        }

        info!(
            parent_hwnd,
            mode = self.host_mode.label(),
            "resolved GPUI top-level hwnd for mpv host"
        );
        match NativeVideoSurface::new(parent_hwnd, self.host_mode) {
            Ok(surface) => {
                let hwnd = surface.hwnd();
                self.native_surface = Some(surface);
                cx.notify();
                Some(hwnd)
            }
            Err(error) => {
                error!(?error, "create native video surface failed");
                self.status = format!("Create native video surface failed: {error:#}").into();
                cx.notify();
                None
            }
        }
    }

    #[cfg(not(windows))]
    fn ensure_native_surface(
        &mut self,
        _window: &Window,
        _cx: &mut Context<Self>,
    ) -> Option<isize> {
        None
    }

    #[cfg(windows)]
    fn update_native_surface_bounds(&mut self, window: &Window) {
        let Some(surface) = self.native_surface.as_ref() else {
            return;
        };
        let Some(bounds) = self.video_bounds else {
            surface.set_visible(false);
            return;
        };
        surface.set_bounds(window, bounds);
    }

    #[cfg(not(windows))]
    fn update_native_surface_bounds(&mut self, _window: &Window) {}

    fn load_path(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let Some(target_wid) = self.ensure_native_surface(window, cx) else {
            warn!("no native video host available");
            self.status = "No native video host available for this platform.".into();
            cx.notify();
            return;
        };
        info!(
            target_wid,
            mode = self.host_mode.label(),
            path = %path.display(),
            "load path into embedded mpv player"
        );

        if self.player.is_none() {
            match MpvEmbedPlayer::new(target_wid) {
                Ok(player) => {
                    info!(
                        target_wid,
                        mode = self.host_mode.label(),
                        "created embedded mpv player"
                    );
                    self.player = Some(player);
                }
                Err(error) => {
                    error!(?error, target_wid, "create embedded mpv player failed");
                    self.status = format!("Create embedded mpv player failed: {error:#}").into();
                    cx.notify();
                    return;
                }
            }
        }

        let Some(player) = self.player.as_mut() else {
            return;
        };

        if let Err(error) = player.load_file(&path) {
            error!(?error, path = %path.display(), "open video failed");
            self.status = format!("Open video failed: {error:#}").into();
            cx.notify();
            return;
        }

        self.opened_path = Some(path.clone());
        self.playing = true;
        self.ended = false;
        self.status = format!("{} playback: {}", self.host_mode.label(), path.display()).into();
        self.update_native_surface_bounds(window);
        cx.notify();
    }

    fn switch_mode(&mut self, mode: HostMode, window: &mut Window, cx: &mut Context<Self>) {
        if self.host_mode == mode {
            return;
        }

        self.host_mode = mode;
        self.player = None;
        #[cfg(windows)]
        {
            self.native_surface = None;
        }

        self.status = format!("Switched to {} host.", mode.label()).into();
        if let Some(path) = self.opened_path.clone() {
            self.load_path(path, window, cx);
            return;
        }

        cx.notify();
    }

    fn render_title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let subtitle = self
            .opened_path
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "No video loaded".to_string());

        TitleBar::new().child(
            h_flex()
                .w_full()
                .justify_between()
                .items_center()
                .pr_3()
                .child(
                    v_flex()
                        .gap_0p5()
                        .child(
                            Label::new("CyberDesktop Examples")
                                .text_sm()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(cx.theme().foreground),
                        )
                        .child(
                            Label::new(subtitle)
                                .text_xs()
                                .text_color(cx.theme().muted_foreground),
                        ),
                )
                .child(
                    Label::new("Drag here")
                        .text_xs()
                        .text_color(cx.theme().muted_foreground),
                ),
        )
    }

    fn open_file(
        &mut self,
        _event: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let entity = cx.entity().downgrade();
        let window_handle = window.window_handle();
        cx.spawn(async move |_, cx| {
            let path = cx.background_spawn(async move { pick_video_file() }).await;
            let Some(path) = path else {
                return;
            };

            let _ = window_handle.update(cx, |_, window, cx| {
                let _ = entity.update(cx, |this, cx| {
                    this.load_path(path, window, cx);
                });
            });
        })
        .detach();
    }

    fn use_child(
        &mut self,
        _event: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.switch_mode(HostMode::Child, window, cx);
    }

    fn use_overlay(
        &mut self,
        _event: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.switch_mode(HostMode::Overlay, window, cx);
    }

    fn toggle_playback(
        &mut self,
        _event: &gpui::ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(player) = self.player.as_mut() else {
            self.status = "Open a video file first.".into();
            cx.notify();
            return;
        };

        if self.ended {
            if let Some(path) = self.opened_path.clone() {
                if let Err(error) = player.load_file(&path) {
                    self.status = format!("Replay failed: {error:#}").into();
                    cx.notify();
                    return;
                }
                self.playing = true;
                self.ended = false;
                self.status = format!("Replaying {}", path.display()).into();
                cx.notify();
            }
            return;
        }

        let pause = self.playing;
        if let Err(error) = player.set_pause(pause) {
            self.status = format!("Toggle playback failed: {error:#}").into();
            cx.notify();
            return;
        }

        self.playing = !pause;
        self.status = if self.playing {
            format!("Resumed {} playback.", self.host_mode.label()).into()
        } else {
            format!("Paused {} playback.", self.host_mode.label()).into()
        };
        cx.notify();
    }

    fn stop(&mut self, _event: &gpui::ClickEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(player) = self.player.as_mut() {
            if let Err(error) = player.stop() {
                self.status = format!("Stop playback failed: {error:#}").into();
                cx.notify();
                return;
            }
        }

        self.playing = false;
        self.ended = true;
        self.status = format!("Stopped {} playback.", self.host_mode.label()).into();
        cx.notify();
    }
}

impl Render for MpvDemo {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.update_native_surface_bounds(window);

        let opened_path = self
            .opened_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "No file selected".to_string());
        let play_label = if self.ended {
            "Replay"
        } else if self.playing {
            "Pause"
        } else {
            "Play"
        };

        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .child(self.render_title_bar(cx))
            .child(
                v_flex()
                    .size_full()
                    .gap_4()
                    .p_4()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                Label::new("app-examples / mpv host compare")
                                    .text_xl()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(cx.theme().foreground),
                            )
                            .child(
                                Label::new("Compare WS_CHILD and Overlay mpv host behavior in the same GPUI layout.")
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(Button::new("mpv-open").label("Open Video").on_click(cx.listener(Self::open_file)))
                            .child(Button::new("mpv-toggle").label(play_label).on_click(cx.listener(Self::toggle_playback)))
                            .child(Button::new("mpv-stop").label("Stop").on_click(cx.listener(Self::stop))),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("mpv-mode-child")
                                    .label("Use WS_CHILD")
                                    .on_click(cx.listener(Self::use_child)),
                            )
                            .child(
                                Button::new("mpv-mode-overlay")
                                    .label("Use Overlay")
                                    .on_click(cx.listener(Self::use_overlay)),
                            )
                            .child(
                                Label::new(format!("Current host: {}", self.host_mode.label()))
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground),
                            ),
                    )
                    .child({
                        let entity = cx.entity().downgrade();
                        div()
                            .w_full()
                            .h(px(VIDEO_MIN_HEIGHT))
                            .rounded(cx.theme().radius)
                            .border_1()
                            .border_color(cx.theme().primary.opacity(0.35))
                            .bg(gpui::rgba(0x0d121aff))
                            .flex()
                            .items_center()
                            .justify_center()
                            .on_prepaint(move |bounds, _, cx| {
                                let _ = entity.update(cx, |this, cx| {
                                    let changed = this
                                        .video_bounds
                                        .map(|prev| {
                                            (prev.origin.x - bounds.origin.x).abs() > px(0.5)
                                                || (prev.origin.y - bounds.origin.y).abs() > px(0.5)
                                                || (prev.size.width - bounds.size.width).abs() > px(0.5)
                                                || (prev.size.height - bounds.size.height).abs() > px(0.5)
                                        })
                                        .unwrap_or(true);
                                    if changed {
                                        this.video_bounds = Some(bounds);
                                        cx.notify();
                                    }
                                });
                            })
                            .child(
                                Label::new(format!("{} host surface", self.host_mode.label()))
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground),
                            )
                    })
                    .child(Alert::info(
                        "mpv-demo-status",
                        format!(
                            "{}\n{}\n{}",
                            opened_path,
                            self.host_mode.subtitle(),
                            self.status
                        ),
                    )),
            )
    }
}

fn pick_video_file() -> Option<PathBuf> {
    std::thread::Builder::new()
        .name("app-examples-video-dialog".into())
        .spawn(|| {
            FileDialog::new()
                .set_title("Open Video")
                .add_filter("Video", &["mp4", "mkv", "mov", "avi", "webm"])
                .pick_file()
        })
        .expect("failed to spawn video dialog thread")
        .join()
        .expect("video dialog thread panicked")
}
