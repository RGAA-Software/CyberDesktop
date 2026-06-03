use std::path::{Path, PathBuf};
use std::time::Duration;

#[cfg(windows)]
use app_mpv_ffi::{probe_media as probe_mpv_media, MpvEmbedPlayer};
use app_media::{AudioFileMetadata, MediaSessionState, VideoFileMetadata};
use files_fs::{
    count_directory_entries, directory_tree_size, extension_type_counts, multi_select_summary,
    parse_tag_color_hex, preview_kind, read_audio_metadata, read_text_preview,
    DirectoryReadOptions, FileItem, FileItemKind, FolderEntryCounts, PreviewKind,
};
use gpui::{img, prelude::*, rgb, ObjectFit, *};
use gpui_component::{
    alert::Alert,
    button::Button,
    description_list::{DescriptionItem, DescriptionList},
    h_flex,
    label::Label,
    scroll::ScrollableElement as _,
    slider::{Slider, SliderEvent, SliderState},
    v_flex, ActiveTheme as _, ElementExt, IconName,
};
#[cfg(windows)]
use raw_window_handle::RawWindowHandle;
use rust_i18n::t;
#[cfg(windows)]
use windows::Win32::Foundation::HWND;
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, MoveWindow, ShowWindow, SW_HIDE, SW_SHOW, WS_BORDER,
    WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_VISIBLE,
};
#[cfg(windows)]
use windows::core::w;

use crate::audio_log::audio_log;
use crate::audio_player::AudioPlayer;
use crate::icons::icon_foreground;
use app_ui::tab::{Tab, TabBar};

#[derive(Debug, Clone)]
pub enum InfoPaneSelection {
    None,
    Single(FileItem),
    Multiple(Vec<FileItem>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FolderSizeState {
    Idle,
    Computing,
    Ready(u64),
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FolderCountsState {
    Loading,
    Ready(FolderEntryCounts),
    Failed,
}

#[derive(Debug, Clone)]
struct FolderStats {
    path: PathBuf,
    generation: u64,
    counts: FolderCountsState,
    size: FolderSizeState,
}

#[derive(Debug, Clone)]
struct AudioPreview {
    path: PathBuf,
    generation: u64,
    metadata: AudioMetadataState,
    play_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AudioMetadataState {
    Loading,
    Ready(AudioFileMetadata),
    Unknown,
}

#[derive(Debug)]
struct VideoPreview {
    path: PathBuf,
    generation: u64,
    metadata: VideoMetadataState,
    playback: VideoPlaybackState,
    current_position: Duration,
    preview_error: Option<String>,
}

#[cfg(windows)]
struct NativeVideoSurface {
    hwnd: HWND,
}

#[cfg(windows)]
fn window_hwnd(window: &Window) -> Option<isize> {
    let handle = raw_window_handle::HasWindowHandle::window_handle(window).ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(window) => Some(window.hwnd.get() as isize),
        _ => None,
    }
}

#[cfg(windows)]
impl NativeVideoSurface {
    fn new(parent_hwnd: isize) -> anyhow::Result<Self> {
        let hwnd = unsafe {
            CreateWindowExW(
                Default::default(),
                w!("STATIC"),
                w!("MPV INFO PANE"),
                WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN | WS_CLIPSIBLINGS | WS_BORDER,
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

        Ok(Self {
            hwnd,
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
        let left = (f32::from(bounds.origin.x) * scale).round() as i32;
        let top = (f32::from(bounds.origin.y) * scale).round() as i32;
        let right =
            ((f32::from(bounds.origin.x) + f32::from(bounds.size.width)) * scale).round() as i32;
        let bottom =
            ((f32::from(bounds.origin.y) + f32::from(bounds.size.height)) * scale).round() as i32;

        if right <= left || bottom <= top {
            self.set_visible(false);
            return;
        }

        unsafe {
            let _ = MoveWindow(self.hwnd, left, top, right - left, bottom - top, true);
        }
        self.set_visible(true);
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum VideoMetadataState {
    Loading,
    Ready(VideoFileMetadata),
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VideoPlaybackState {
    Idle,
    Starting,
    Playing,
    Paused,
    Finished,
}

pub struct InfoPane {
    selected_tab: usize,
    selection: InfoPaneSelection,
    selection_key: Option<Vec<PathBuf>>,
    folder_stats: Option<FolderStats>,
    folder_generation: u64,
    audio_player: AudioPlayer,
    audio_preview: Option<AudioPreview>,
    audio_generation: u64,
    audio_poll_generation: u64,
    audio_seek_dragging: bool,
    audio_seek_preview_position: Option<Duration>,
    audio_status: Option<String>,
    audio_seek_slider: Entity<SliderState>,
    _audio_seek_slider_subscriptions: Vec<Subscription>,
    video_preview: Option<VideoPreview>,
    video_generation: u64,
    video_poll_generation: u64,
    video_seek_dragging: bool,
    video_status: Option<String>,
    video_seek_slider: Entity<SliderState>,
    _video_seek_slider_subscriptions: Vec<Subscription>,
    #[cfg(windows)]
    video_host_bounds: Option<Bounds<Pixels>>,
    #[cfg(windows)]
    native_video_surface: Option<NativeVideoSurface>,
    #[cfg(windows)]
    embedded_video_player: Option<MpvEmbedPlayer>,
}

impl InfoPane {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let audio_seek_slider =
            cx.new(|_| SliderState::new().min(0.0).max(1.0).step(0.001).default_value(0.0));
        let audio_seek_slider_subscriptions = vec![cx.subscribe(
            &audio_seek_slider,
            |this, _, event: &SliderEvent, cx| match event {
                SliderEvent::Change(value) => {
                    this.audio_seek_dragging = true;
                    this.preview_audio_seek_fraction(value.start(), cx);
                }
                SliderEvent::Release(value) => {
                    this.audio_seek_dragging = false;
                    this.commit_audio_seek_fraction(value.start(), cx);
                }
            },
        )];
        let video_seek_slider =
            cx.new(|_| SliderState::new().min(0.0).max(1.0).step(0.001).default_value(0.0));
        let video_seek_slider_subscriptions = vec![cx.subscribe(
            &video_seek_slider,
            |this, _, event: &SliderEvent, cx| match event {
                SliderEvent::Change(value) => {
                    this.video_seek_dragging = true;
                    this.preview_video_seek_fraction(value.start(), cx);
                }
                SliderEvent::Release(value) => {
                    this.video_seek_dragging = false;
                    this.commit_video_seek_fraction(value.start(), cx);
                }
            },
        )];

        Self {
            selected_tab: 0,
            selection: InfoPaneSelection::None,
            selection_key: None,
            folder_stats: None,
            folder_generation: 0,
            audio_player: AudioPlayer::start(),
            audio_preview: None,
            audio_generation: 0,
            audio_poll_generation: 0,
            audio_seek_dragging: false,
            audio_seek_preview_position: None,
            audio_status: None,
            audio_seek_slider,
            _audio_seek_slider_subscriptions: audio_seek_slider_subscriptions,
            video_preview: None,
            video_generation: 0,
            video_poll_generation: 0,
            video_seek_dragging: false,
            video_status: None,
            video_seek_slider,
            _video_seek_slider_subscriptions: video_seek_slider_subscriptions,
            #[cfg(windows)]
            video_host_bounds: None,
            #[cfg(windows)]
            native_video_surface: None,
            #[cfg(windows)]
            embedded_video_player: None,
        }
    }

    fn should_ignore_selection_change(&self, new_key: &[PathBuf]) -> bool {
        let current_media = self
            .audio_player
            .snapshot()
            .active_path
            .or_else(|| self.audio_preview.as_ref().map(|preview| preview.path.clone()))
            .or_else(|| self.video_preview.as_ref().map(|preview| preview.path.clone()));

        let Some(active_path) = current_media else {
            return false;
        };
        if new_key.iter().any(|path| path == &active_path) {
            return true;
        }
        if new_key.is_empty() {
            return true;
        }
        if new_key.len() == 1 {
            if active_path
                .parent()
                .is_some_and(|parent| new_key[0].as_path() == parent)
            {
                return true;
            }
        }
        false
    }

    fn selected_audio_path(&self) -> Option<PathBuf> {
        if let Some(preview) = self.audio_preview.as_ref() {
            if preview_kind(&preview.path) == Some(PreviewKind::Audio) {
                return Some(preview.path.clone());
            }
        }
        match &self.selection {
            InfoPaneSelection::Single(item) if preview_kind(&item.path) == Some(PreviewKind::Audio) => {
                Some(item.path.clone())
            }
            _ => None,
        }
    }

    fn ensure_audio_preview_state(&mut self, path: &Path) {
        if self
            .audio_preview
            .as_ref()
            .is_some_and(|preview| preview.path == path)
        {
            return;
        }
        self.audio_generation = self.audio_generation.wrapping_add(1);
        self.audio_preview = Some(AudioPreview {
            path: path.to_path_buf(),
            generation: self.audio_generation,
            metadata: AudioMetadataState::Loading,
            play_error: None,
        });
    }

    fn schedule_audio_metadata_load(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let generation = self
            .audio_preview
            .as_ref()
            .filter(|preview| preview.path == path)
            .map(|preview| preview.generation)
            .unwrap_or(self.audio_generation);
        let path_for_probe = path.clone();
        cx.spawn(async move |pane, cx| {
            let metadata = cx
                .background_spawn(async move { read_audio_metadata(&path_for_probe) })
                .await;
            let _ = pane.update(cx, |pane, cx| {
                let Some(preview) = pane.audio_preview.as_mut() else {
                    return;
                };
                if preview.generation != generation || preview.path != path {
                    return;
                }
                preview.metadata = match metadata {
                    Some(metadata) => AudioMetadataState::Ready(metadata),
                    None => AudioMetadataState::Unknown,
                };
                cx.notify();
            });
        })
        .detach();
    }

    fn ensure_video_preview_state(&mut self, path: &Path) {
        if self
            .video_preview
            .as_ref()
            .is_some_and(|preview| preview.path == path)
        {
            return;
        }
        self.video_generation = self.video_generation.wrapping_add(1);
        self.video_preview = Some(VideoPreview {
            path: path.to_path_buf(),
            generation: self.video_generation,
            metadata: VideoMetadataState::Loading,
            playback: VideoPlaybackState::Idle,
            current_position: Duration::ZERO,
            preview_error: None,
        });
    }

    fn schedule_video_metadata_load(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let generation = self
            .video_preview
            .as_ref()
            .filter(|preview| preview.path == path)
            .map(|preview| preview.generation)
            .unwrap_or(self.video_generation);
        let path_for_probe = path.clone();
        cx.spawn(async move |pane, cx| {
            let metadata = cx
                .background_spawn(async move {
                    #[cfg(windows)]
                    {
                        probe_mpv_media(&path_for_probe)
                            .map(|result| {
                                Some(VideoFileMetadata {
                                    duration: result.duration,
                                    codec: result.video_codec,
                                    width: result.width,
                                    height: result.height,
                                    frame_rate_milli: result.frame_rate_milli,
                                    bitrate_kbps: result.bitrate_kbps,
                                    file_size: result.file_size,
                                    has_audio: result.has_audio,
                                })
                            })
                            .map_err(|error| error.to_string())
                    }
                    #[cfg(not(windows))]
                    {
                        Ok(None)
                    }
                })
                .await;
            let _ = pane.update(cx, |pane, cx| {
                let Some(preview) = pane.video_preview.as_mut() else {
                    return;
                };
                if preview.generation != generation || preview.path != path {
                    return;
                }
                match metadata {
                    Ok(Some(metadata)) => {
                        preview.metadata = VideoMetadataState::Ready(metadata);
                        preview.preview_error = None;
                    }
                    Ok(None) => {
                        preview.metadata = VideoMetadataState::Unknown;
                    }
                    Err(error) => {
                        preview.metadata = VideoMetadataState::Unknown;
                        preview.preview_error = Some(error);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn set_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        if !visible {
            self.stop_video_poll();
            self.video_seek_dragging = false;
            self.stop_video_preview(cx);
            self.video_status = None;
        }
    }

    pub fn set_selection(
        &mut self,
        selection: InfoPaneSelection,
        read_options: DirectoryReadOptions,
        cx: &mut Context<Self>,
    ) {
        let key = selection_paths_key(&selection);
        if self.selection_key.as_ref() == Some(&key) {
            return;
        }
        if self.should_ignore_selection_change(&key) {
            audio_log!(
                "set_selection: ignore {:?} while active {:?}",
                key,
                self.audio_player
                    .snapshot()
                    .active_path
                    .or_else(|| self.video_preview.as_ref().map(|preview| preview.path.clone()))
            );
            return;
        }
        audio_log!(
            "set_selection APPLY old={:?} new={:?}",
            self.selection_key,
            key
        );
        self.selection_key = Some(key);
        self.selection = selection;
        self.folder_stats = None;
        self.audio_player.stop();
        self.stop_audio_poll();
        self.audio_preview = None;
        self.audio_status = None;
        self.audio_seek_preview_position = None;
        self.stop_video_poll();
        self.video_seek_dragging = false;
        self.stop_video_preview(cx);
        self.video_preview = None;
        self.video_status = None;

        if let InfoPaneSelection::Single(item) = &self.selection {
            if item.kind == FileItemKind::Folder {
                self.start_folder_counts(item.path.clone(), read_options, cx);
            } else if preview_kind(&item.path) == Some(PreviewKind::Audio) {
                self.start_audio_preview(item.path.clone(), cx);
            } else if preview_kind(&item.path) == Some(PreviewKind::Video) {
                self.start_video_preview(item.path.clone(), cx);
            }
        }
        cx.notify();
    }

    fn start_audio_preview(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.ensure_audio_preview_state(&path);
        self.schedule_audio_metadata_load(path, cx);
    }

    fn start_video_preview(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.ensure_video_preview_state(&path);
        self.schedule_video_metadata_load(path.clone(), cx);
    }

    #[cfg(windows)]
    fn embedded_video_active_for_path(&self, path: &Path) -> bool {
        self.video_preview.as_ref().is_some_and(|preview| {
            preview.path == path
                && matches!(
                    preview.playback,
                    VideoPlaybackState::Starting
                        | VideoPlaybackState::Playing
                        | VideoPlaybackState::Paused
                )
                && self.embedded_video_player.is_some()
                && self.native_video_surface.is_some()
        })
    }

    #[cfg(windows)]
    fn embedded_video_loaded_for_path(&self, path: &Path) -> bool {
        self.video_preview.as_ref().is_some_and(|preview| {
            preview.path == path
                && self.embedded_video_player.is_some()
                && self.native_video_surface.is_some()
                && matches!(
                    preview.playback,
                    VideoPlaybackState::Paused
                        | VideoPlaybackState::Starting
                        | VideoPlaybackState::Playing
                        | VideoPlaybackState::Finished
                )
        })
    }

    #[cfg(windows)]
    fn ensure_native_video_surface(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<isize> {
        if let Some(surface) = self.native_video_surface.as_ref() {
            return Ok(surface.hwnd());
        }

        let parent_hwnd =
            window_hwnd(window).ok_or_else(|| anyhow::anyhow!("resolve top-level hwnd"))?;
        let surface = NativeVideoSurface::new(parent_hwnd)?;
        let hwnd = surface.hwnd();
        self.native_video_surface = Some(surface);
        cx.notify();
        Ok(hwnd)
    }

    #[cfg(windows)]
    fn update_native_video_surface_bounds(&mut self, window: &Window) {
        let Some(surface) = self.native_video_surface.as_ref() else {
            return;
        };
        if self.selected_tab != 1 {
            surface.set_visible(false);
            return;
        }
        let Some(bounds) = self.video_host_bounds else {
            surface.set_visible(false);
            return;
        };
        surface.set_bounds(window, bounds);
    }

    #[cfg(windows)]
    fn play_embedded_video_preview(
        &mut self,
        path: &Path,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let target_wid = self.ensure_native_video_surface(window, cx)?;
        if self.embedded_video_player.is_none() {
            self.embedded_video_player = Some(MpvEmbedPlayer::new(target_wid)?);
        }
        let player = self
            .embedded_video_player
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("create embedded mpv player"))?;
        player.load_file(path)?;
        self.update_native_video_surface_bounds(window);
        Ok(())
    }

    #[cfg(windows)]
    fn prepare_embedded_video_preview(
        &mut self,
        path: &Path,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        self.play_embedded_video_preview(path, window, cx)?;
        let player = self
            .embedded_video_player
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("create embedded mpv player"))?;
        player.set_pause(true)?;
        self.update_native_video_surface_bounds(window);
        Ok(())
    }

    #[cfg(windows)]
    fn sync_embedded_video_state(&mut self, cx: &mut Context<Self>) {
        let Some(player) = self.embedded_video_player.as_mut() else {
            return;
        };
        let mut changed = false;
        player.poll_events();
        if let Some(preview) = self.video_preview.as_mut() {
            if matches!(
                preview.playback,
                VideoPlaybackState::Starting | VideoPlaybackState::Playing | VideoPlaybackState::Paused
            ) {
                if let Ok(Some(position)) = player.time_pos() {
                    if preview.current_position != position {
                        preview.current_position = position;
                        changed = true;
                    }
                }
            }
        }
        if player.ended() {
            if let Some(preview) = self.video_preview.as_mut() {
                if matches!(
                    preview.playback,
                    VideoPlaybackState::Starting | VideoPlaybackState::Playing | VideoPlaybackState::Paused
                ) {
                    preview.playback = VideoPlaybackState::Finished;
                    self.video_status = Some(t!("info_pane.video.ended").to_string());
                    self.stop_video_poll();
                    changed = true;
                }
            }
        }
        if changed {
            cx.notify();
        }
    }

    fn stop_video_poll(&mut self) {
        self.video_poll_generation = self.video_poll_generation.wrapping_add(1);
    }

    fn start_video_poll(&mut self, path: &Path, cx: &mut Context<Self>) {
        self.video_poll_generation = self.video_poll_generation.wrapping_add(1);
        let generation = self.video_poll_generation;
        let path = path.to_path_buf();
        cx.spawn(async move |pane, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(200))
                    .await;

                let mut keep_polling = false;
                let update_ok = pane.update(cx, |pane, cx| {
                    if pane.video_poll_generation != generation {
                        return;
                    }
                    keep_polling = pane.video_preview.as_ref().is_some_and(|preview| {
                        preview.path == path
                            && matches!(
                                preview.playback,
                                VideoPlaybackState::Starting
                                    | VideoPlaybackState::Playing
                                    | VideoPlaybackState::Paused
                            )
                    });
                    if !keep_polling {
                        return;
                    }
                    #[cfg(windows)]
                    pane.sync_embedded_video_state(cx);
                });
                if update_ok.is_err() || !keep_polling {
                    break;
                }
            }
        })
        .detach();
    }

    fn selected_video_path(&self) -> Option<PathBuf> {
        if let Some(preview) = self.video_preview.as_ref() {
            if preview_kind(&preview.path) == Some(PreviewKind::Video) {
                return Some(preview.path.clone());
            }
        }
        match &self.selection {
            InfoPaneSelection::Single(item) if preview_kind(&item.path) == Some(PreviewKind::Video) => {
                Some(item.path.clone())
            }
            _ => None,
        }
    }

    fn toggle_video_playback(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.selected_video_path() else {
            self.video_status = Some(t!("info_pane.video.no_selection").to_string());
            cx.notify();
            return;
        };

        let current_state = self
            .video_preview
            .as_ref()
            .filter(|preview| preview.path == path)
            .map(|preview| preview.playback);

        match current_state {
            Some(VideoPlaybackState::Starting | VideoPlaybackState::Playing) => {
                self.pause_video_preview(cx);
            }
            Some(VideoPlaybackState::Paused) => {
                self.resume_video_preview(cx);
            }
            _ => {
                self.ensure_video_preview_state(&path);
                self.play_video_preview(path, _window, cx);
            }
        }
        cx.notify();
    }

    fn play_video_preview(&mut self, path: PathBuf, window: &Window, cx: &mut Context<Self>) {
        self.stop_video_poll();
        self.stop_video_preview(cx);

        self.ensure_video_preview_state(&path);
        self.selected_tab = 1;

        if let Some(preview) = self.video_preview.as_mut() {
            preview.playback = VideoPlaybackState::Starting;
            preview.current_position = Duration::ZERO;
            preview.preview_error = None;
        } else {
            return;
        }
        self.video_status = Some(t!("info_pane.video.starting").to_string());

        #[cfg(windows)]
        {
            match self.play_embedded_video_preview(&path, window, cx) {
                Ok(()) => {
                    if let Some(preview) = self.video_preview.as_mut() {
                        preview.playback = VideoPlaybackState::Playing;
                    }
                    self.video_status = Some(format!(
                        "mpv embedded playback: {}",
                        path.display()
                    ));
                    self.start_video_poll_for_current(cx);
                    return;
                }
                Err(error) => {
                    if let Some(preview) = self.video_preview.as_mut() {
                        preview.playback = VideoPlaybackState::Idle;
                        preview.preview_error =
                            Some(format!("mpv embed failed: {error:#}"));
                    }
                    self.embedded_video_player = None;
                    self.native_video_surface = None;
                    self.video_status = None;
                    return;
                }
            }
        }
    }

    fn prepare_video_preview(&mut self, path: PathBuf, window: &Window, cx: &mut Context<Self>) {
        self.stop_video_poll();
        self.stop_video_preview(cx);
        self.ensure_video_preview_state(&path);

        #[cfg(windows)]
        {
            match self.prepare_embedded_video_preview(&path, window, cx) {
                Ok(()) => {
                    if let Some(preview) = self.video_preview.as_mut() {
                        preview.playback = VideoPlaybackState::Paused;
                        preview.preview_error = None;
                    }
                    self.video_status = None;
                    self.start_video_poll_for_current(cx);
                }
                Err(error) => {
                    if let Some(preview) = self.video_preview.as_mut() {
                        preview.playback = VideoPlaybackState::Idle;
                        preview.preview_error = Some(format!("mpv embed failed: {error:#}"));
                    }
                    self.embedded_video_player = None;
                    self.native_video_surface = None;
                    self.video_status = None;
                }
            }
        }
    }

    fn pause_video_preview(&mut self, cx: &mut Context<Self>) {
        #[cfg(windows)]
        {
            if let Some(player) = self.embedded_video_player.as_mut() {
                if player.set_pause(true).is_ok() {
                    if let Some(preview) = self.video_preview.as_mut() {
                        preview.playback = VideoPlaybackState::Paused;
                    }
                    self.video_status = Some(t!("info_pane.video.paused").to_string());
                    cx.notify();
                }
            }
        }
    }

    fn resume_video_preview(&mut self, cx: &mut Context<Self>) {
        #[cfg(windows)]
        {
            if let Some(player) = self.embedded_video_player.as_mut() {
                if player.set_pause(false).is_ok() {
                    if let Some(preview) = self.video_preview.as_mut() {
                        preview.playback = VideoPlaybackState::Playing;
                    }
                    self.video_status = None;
                    self.start_video_poll_for_current(cx);
                    cx.notify();
                }
            }
        }
    }

    fn seek_video_relative(&mut self, seconds: f64, cx: &mut Context<Self>) {
        #[cfg(windows)]
        {
            if let Some(player) = self.embedded_video_player.as_mut() {
                if player.seek_relative(seconds).is_ok() {
                    if let Some(preview) = self.video_preview.as_mut() {
                        if let Ok(Some(position)) = player.time_pos() {
                            preview.current_position = position;
                        } else if seconds.is_sign_positive() {
                            preview.current_position += Duration::from_secs_f64(seconds);
                        } else {
                            preview.current_position = preview
                                .current_position
                                .saturating_sub(Duration::from_secs_f64(-seconds));
                        }
                    }
                    cx.notify();
                }
            }
        }
    }

    fn preview_video_seek_fraction(&mut self, fraction: f32, cx: &mut Context<Self>) {
        let Some(total) = self.video_preview_total_duration() else {
            return;
        };
        if let Some(preview) = self.video_preview.as_mut() {
            let secs = total.as_secs_f64() * f64::from(fraction.clamp(0.0, 1.0));
            preview.current_position = Duration::from_secs_f64(secs);
            cx.notify();
        }
    }

    fn commit_video_seek_fraction(&mut self, fraction: f32, cx: &mut Context<Self>) {
        let Some(total) = self.video_preview_total_duration() else {
            return;
        };
        let target = Duration::from_secs_f64(
            total.as_secs_f64() * f64::from(fraction.clamp(0.0, 1.0)),
        );
        #[cfg(windows)]
        {
            if let Some(player) = self.embedded_video_player.as_mut() {
                if player.seek_to(target).is_ok() {
                    if let Some(preview) = self.video_preview.as_mut() {
                        preview.current_position = target;
                        if matches!(preview.playback, VideoPlaybackState::Finished) {
                            preview.playback = VideoPlaybackState::Paused;
                        }
                    }
                    self.video_status = None;
                    self.start_video_poll_for_current(cx);
                    cx.notify();
                }
            }
        }
    }

    fn stop_video_playback(&mut self, cx: &mut Context<Self>) {
        self.stop_video_poll();
        self.stop_video_preview(cx);
        self.video_seek_dragging = false;
        self.video_status = None;
        cx.notify();
    }

    fn start_video_poll_for_current(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = self.video_preview.as_ref().map(|preview| preview.path.clone()) {
            self.start_video_poll(&path, cx);
        }
    }

    fn video_preview_total_duration(&self) -> Option<Duration> {
        self.video_preview
            .as_ref()
            .and_then(video_preview_metadata)
            .and_then(|metadata| metadata.duration)
    }

    fn sync_video_seek_slider(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.video_seek_dragging {
            return;
        }
        let fraction = self
            .video_preview
            .as_ref()
            .and_then(|preview| {
                video_preview_metadata(preview).map(|metadata| {
                    video_progress_fraction(preview, Some(metadata))
                })
            })
            .unwrap_or(0.0);
        let current = self.video_seek_slider.read(cx).value().start();
        if (current - fraction).abs() <= 0.001 {
            return;
        }
        self.video_seek_slider.update(cx, |slider, cx| {
            slider.set_value(fraction, window, cx);
        });
    }

    fn stop_video_preview(&mut self, _cx: &mut Context<Self>) {
        #[cfg(windows)]
        let was_active =
            self.embedded_video_player.is_some() || self.native_video_surface.is_some();
        #[cfg(not(windows))]
        let was_active = false;
        if !was_active {
            return;
        }
        #[cfg(windows)]
        {
            if let Some(player) = self.embedded_video_player.as_mut() {
                let _ = player.stop();
            }
            self.embedded_video_player = None;
            self.native_video_surface = None;
            self.video_host_bounds = None;
        }
        if let Some(preview) = self.video_preview.as_mut() {
            preview.playback = VideoPlaybackState::Idle;
            preview.current_position = Duration::ZERO;
        }
        self.video_seek_dragging = false;
    }

    fn toggle_audio_playback(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        audio_log!("toggle_playback: click");
        let Some(path) = self.selected_audio_path() else {
            audio_log!("toggle_playback: no selection");
            self.audio_status = Some(t!("info_pane.audio.no_selection").to_string());
            cx.notify();
            return;
        };
        audio_log!("toggle_playback: path={}", path.display());

        self.ensure_audio_preview_state(&path);
        if self
            .audio_preview
            .as_ref()
            .is_some_and(|preview| preview.metadata == AudioMetadataState::Loading)
        {
            self.schedule_audio_metadata_load(path.clone(), cx);
        }

        if self.audio_player.is_active_path(&path) {
            audio_log!("toggle_playback: pause/resume");
            self.audio_player.toggle_pause();
            self.audio_status = None;
            if let Some(preview) = self.audio_preview.as_mut() {
                preview.play_error = None;
            }
            self.start_audio_poll(&path, cx);
            cx.notify();
            audio_log!("toggle_playback: done (pause)");
            return;
        }

        audio_log!("toggle_playback: starting play");
        let output_state = self.audio_player.snapshot();
        if !output_state.output_ok && output_state.play_error.is_some() {
            let message = output_state
                .play_error
                .unwrap_or_else(|| t!("info_pane.audio.no_output").to_string());
            self.audio_status = Some(message.clone());
            if let Some(preview) = self.audio_preview.as_mut() {
                preview.play_error = Some(message);
            }
            cx.notify();
            audio_log!("toggle_playback: output failed");
            return;
        }
        self.audio_status = Some(t!("info_pane.audio.starting").to_string());
        if let Some(preview) = self.audio_preview.as_mut() {
            preview.play_error = None;
        }
        self.audio_player.play(path.clone());
        self.start_audio_poll(&path, cx);
        cx.notify();
        audio_log!("toggle_playback: done (play sent)");
    }

    fn stop_audio_playback(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.audio_player.stop();
        self.stop_audio_poll();
        self.audio_seek_dragging = false;
        self.audio_seek_preview_position = None;
        self.audio_status = None;
        if let Some(preview) = self.audio_preview.as_mut() {
            preview.play_error = None;
        }
        cx.notify();
    }

    fn stop_audio_poll(&mut self) {
        self.audio_poll_generation = self.audio_poll_generation.wrapping_add(1);
    }

    fn start_audio_poll(&mut self, path: &Path, cx: &mut Context<Self>) {
        self.audio_poll_generation = self.audio_poll_generation.wrapping_add(1);
        let generation = self.audio_poll_generation;
        let path = path.to_path_buf();
        audio_log!(
            "start_audio_poll gen={generation} path={}",
            path.display()
        );
        cx.spawn(async move |pane, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(200))
                    .await;

                let mut keep_polling = false;
                let mut needs_notify = false;
                let update_ok = pane.update(cx, |pane, _cx| {
                    if pane.audio_poll_generation != generation {
                        audio_log!("poll gen={generation} stale, stop");
                        return;
                    }
                    keep_polling = true;
                    let player = &pane.audio_player;
                    let state = player.snapshot();

                        if player.is_active_path(&path) {
                            pane.audio_status = None;
                        if let Some(target) = pane.audio_seek_preview_position {
                            if state.position.abs_diff(target) <= Duration::from_millis(250) {
                                pane.audio_seek_preview_position = None;
                            }
                        }
                        if let Some(total) = state.total {
                            if let Some(preview) = pane.audio_preview.as_mut() {
                                if preview.path == path {
                                    match &mut preview.metadata {
                                        AudioMetadataState::Loading => {
                                            preview.metadata = AudioMetadataState::Ready(AudioFileMetadata {
                                                duration: Some(total),
                                                ..AudioFileMetadata::default()
                                            });
                                        }
                                        AudioMetadataState::Ready(metadata) if metadata.duration.is_none() => {
                                            metadata.duration = Some(total);
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                        if player.take_finished(&path) {
                            audio_log!("poll gen={generation} track finished");
                            pane.audio_seek_preview_position = None;
                            pane.audio_status = Some(t!("info_pane.audio.ended").to_string());
                            pane.stop_audio_poll();
                            keep_polling = false;
                        }
                    } else if let Some(error) = state.play_error.clone() {
                        audio_log!("poll gen={generation} play_error: {error}");
                        if let Some(preview) = pane.audio_preview.as_mut() {
                            if preview.path == path {
                                preview.play_error = Some(error);
                            }
                        }
                        pane.audio_status = None;
                        pane.stop_audio_poll();
                        keep_polling = false;
                    } else if pane.audio_status.is_none() {
                        audio_log!("poll gen={generation} idle stop");
                        keep_polling = false;
                    }

                    needs_notify = true;
                });
                if update_ok.is_err() {
                    audio_log!("poll gen={generation} pane gone");
                    break;
                }
                if needs_notify {
                    let pane_notify = pane.clone();
                    let _ = pane.update(cx, |_, cx| {
                        cx.defer(move |cx| {
                            let _ = pane_notify.update(cx, |_, cx| cx.notify());
                        });
                    });
                }
                if !keep_polling {
                    audio_log!("poll gen={generation} loop exit");
                    break;
                }
            }
        })
        .detach();
    }

    fn audio_total_duration(&self, path: &Path) -> Option<Duration> {
        if let Some(total) = self.audio_player.total_duration(path) {
            return Some(total);
        }
        let preview = self.audio_preview.as_ref()?;
        if preview.path != path {
            return None;
        }
        match &preview.metadata {
            AudioMetadataState::Ready(metadata) => metadata.duration,
            _ => None,
        }
    }

    fn audio_progress_fraction(&self, path: &Path) -> f32 {
        let Some(total) = self.audio_total_duration(path) else {
            return 0.;
        };
        if total.is_zero() {
            return 0.;
        }
        let position = self.audio_current_position(path);
        (position.as_secs_f32() / total.as_secs_f32()).clamp(0., 1.)
    }

    fn audio_time_line(&self, path: &Path) -> String {
        let position = self.audio_current_position(path);
        let total = self
            .audio_total_duration(path)
            .map(format_audio_duration)
            .unwrap_or_else(|| "--:--".to_string());
        format!(
            "{} / {}",
            format_audio_duration(position),
            total
        )
    }

    fn audio_current_position(&self, path: &Path) -> Duration {
        if self
            .selected_audio_path()
            .as_deref()
            .is_some_and(|selected| selected == path)
        {
            if let Some(position) = self.audio_seek_preview_position {
                return position;
            }
        }
        self.audio_player.position(path).unwrap_or(Duration::ZERO)
    }

    fn audio_play_button_label(&self, path: &Path) -> String {
        let player = &self.audio_player;
        if player.is_active_path(path) && !player.is_paused() {
            t!("info_pane.audio.pause").to_string()
        } else if player.is_active_path(path) && player.is_paused() {
            t!("info_pane.audio.resume").to_string()
        } else if self.audio_is_finished(path) {
            t!("info_pane.audio.replay").to_string()
        } else {
            t!("info_pane.audio.play").to_string()
        }
    }

    fn audio_is_finished(&self, path: &Path) -> bool {
        let state = self.audio_player.snapshot();
        state.active_path.is_none() && state.media_state == MediaSessionState::Ended && self
            .audio_preview
            .as_ref()
            .is_some_and(|preview| preview.path == path)
    }

    fn preview_audio_seek_fraction(&mut self, fraction: f32, cx: &mut Context<Self>) {
        let Some(path) = self.selected_audio_path() else {
            return;
        };
        let Some(total) = self.audio_total_duration(&path) else {
            return;
        };
        let position = Duration::from_secs_f64(total.as_secs_f64() * f64::from(fraction.clamp(0.0, 1.0)));
        self.audio_seek_preview_position = Some(position);
        cx.notify();
    }

    fn commit_audio_seek_fraction(&mut self, fraction: f32, cx: &mut Context<Self>) {
        let Some(path) = self.selected_audio_path() else {
            return;
        };
        let Some(total) = self.audio_total_duration(&path) else {
            return;
        };
        if !self.audio_player.is_active_path(&path) {
            return;
        }
        let target = Duration::from_secs_f64(total.as_secs_f64() * f64::from(fraction.clamp(0.0, 1.0)));
        self.audio_seek_preview_position = Some(target);
        self.audio_player.seek_to(target);
        self.start_audio_poll(&path, cx);
        cx.notify();
    }

    fn seek_audio_relative(&mut self, seconds: f64, cx: &mut Context<Self>) {
        let Some(path) = self.selected_audio_path() else {
            return;
        };
        if !self.audio_player.is_active_path(&path) {
            return;
        }
        self.audio_seek_preview_position = None;
        self.audio_player.seek_relative(seconds);
        self.start_audio_poll(&path, cx);
        cx.notify();
    }

    fn sync_audio_seek_slider(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.audio_seek_dragging {
            return;
        }
        let Some(path) = self.selected_audio_path() else {
            return;
        };
        let fraction = self.audio_progress_fraction(&path);
        let current = self.audio_seek_slider.read(cx).value().start();
        if (current - fraction).abs() <= 0.001 {
            return;
        }
        self.audio_seek_slider.update(cx, |slider, cx| {
            slider.set_value(fraction, window, cx);
        });
    }

    fn start_folder_counts(&mut self, path: PathBuf, read_options: DirectoryReadOptions, cx: &mut Context<Self>) {
        self.folder_generation = self.folder_generation.wrapping_add(1);
        let generation = self.folder_generation;
        self.folder_stats = Some(FolderStats {
            path: path.clone(),
            generation,
            counts: FolderCountsState::Loading,
            size: FolderSizeState::Idle,
        });
        cx.spawn(async move |pane, cx| {
            let counts = cx
                .background_spawn(async move { count_directory_entries(&path, read_options) })
                .await;
            let _ = pane.update(cx, |pane, cx| {
                let Some(stats) = pane.folder_stats.as_mut() else {
                    return;
                };
                if stats.generation != generation {
                    return;
                }
                stats.counts = match counts {
                    Ok(counts) => FolderCountsState::Ready(counts),
                    Err(_) => FolderCountsState::Failed,
                };
                cx.notify();
            });
        })
        .detach();
    }

    fn start_folder_size_calculation(&mut self, cx: &mut Context<Self>) {
        let Some(stats) = &mut self.folder_stats else {
            return;
        };
        if matches!(stats.size, FolderSizeState::Computing) {
            return;
        }
        let path = stats.path.clone();
        let generation = stats.generation;
        stats.size = FolderSizeState::Computing;
        cx.spawn(async move |pane, cx| {
            let size = cx
                .background_spawn(async move { directory_tree_size(&path) })
                .await;
            let _ = pane.update(cx, |pane, cx| {
                let Some(stats) = pane.folder_stats.as_mut() else {
                    return;
                };
                if stats.generation != generation {
                    return;
                }
                stats.size = match size {
                    Ok(bytes) => FolderSizeState::Ready(bytes),
                    Err(_) => FolderSizeState::Failed,
                };
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn folder_contains_line(&self) -> Option<(String, String)> {
        let stats = self.folder_stats.as_ref()?;
        let label = t!("info_pane.folder.contains").to_string();
        let value = match stats.counts {
            FolderCountsState::Loading => t!("info_pane.folder.loading").to_string(),
            FolderCountsState::Ready(counts) => format_folder_contains(&counts),
            FolderCountsState::Failed => t!("info_pane.folder.counts_error").to_string(),
        };
        Some((label, value))
    }

    fn folder_size_line(&self) -> Option<(String, String)> {
        let stats = self.folder_stats.as_ref()?;
        let label = t!("info_pane.folder.size_on_disk").to_string();
        let value = match stats.size {
            FolderSizeState::Idle => return None,
            FolderSizeState::Computing => t!("info_pane.folder.calculating").to_string(),
            FolderSizeState::Ready(bytes) => format_size(bytes),
            FolderSizeState::Failed => t!("info_pane.folder.size_error").to_string(),
        };
        Some((label, value))
    }

    fn show_calculate_size_button(&self) -> bool {
        self.folder_stats.as_ref().is_some_and(|stats| {
            matches!(
                stats.size,
                FolderSizeState::Idle | FolderSizeState::Failed
            )
        })
    }
}

impl AudioPreview {
    fn metadata(&self) -> Option<&AudioFileMetadata> {
        match &self.metadata {
            AudioMetadataState::Ready(metadata) => Some(metadata),
            _ => None,
        }
    }
}

fn selection_paths_key(selection: &InfoPaneSelection) -> Vec<PathBuf> {
    match selection {
        InfoPaneSelection::None => Vec::new(),
        InfoPaneSelection::Single(item) => vec![item.path.clone()],
        InfoPaneSelection::Multiple(items) => items.iter().map(|item| item.path.clone()).collect(),
    }
}

fn format_folder_contains(counts: &FolderEntryCounts) -> String {
    let mut parts = Vec::new();
    if counts.files > 0 {
        parts.push(t!("info_pane.multi.files", count = counts.files).to_string());
    }
    if counts.folders > 0 {
        parts.push(t!("info_pane.multi.folders", count = counts.folders).to_string());
    }
    if counts.other > 0 {
        parts.push(t!("info_pane.multi.other", count = counts.other).to_string());
    }
    if parts.is_empty() {
        t!("info_pane.folder.empty").to_string()
    } else {
        parts.join(", ")
    }
}

impl Render for InfoPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        #[cfg(windows)]
        {
            self.sync_embedded_video_state(cx);
            self.update_native_video_surface_bounds(window);
            if self.selected_tab == 1 {
                if let Some(path) = self.selected_video_path() {
                    if !self.embedded_video_loaded_for_path(&path) {
                        self.prepare_video_preview(path, window, cx);
                    }
                }
            }
        }
        self.sync_audio_seek_slider(window, cx);
        self.sync_video_seek_slider(window, cx);

        let selected_tab = self.selected_tab;
        let selection = self.selection.clone();
        let show_calc_size = self.show_calculate_size_button();

        v_flex()
            .id("info-pane")
            .size_full()
            .min_w_0()
            .border_l_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(
                TabBar::new("info-pane-tabs")
                    .w_full()
                    .selected_index(selected_tab)
                    .on_click(cx.listener(|this, ix: &usize, _, cx| {
                        this.selected_tab = *ix;
                        cx.notify();
                    }))
                    .child(Tab::new().label(t!("info_pane.tab.details")))
                    .child(Tab::new().label(t!("info_pane.tab.preview"))),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .p_3()
                    .gap_3()
                    .child(tab_content(selected_tab, &selection, show_calc_size, self, window, cx)),
            )
    }
}

fn tab_content(
    selected_tab: usize,
    selection: &InfoPaneSelection,
    show_calc_size: bool,
    pane: &InfoPane,
    window: &mut Window,
    cx: &mut Context<InfoPane>,
) -> AnyElement {
    if selected_tab == 0 {
        details_panel(selection, show_calc_size, pane, window, cx)
    } else {
        preview_panel(selection, show_calc_size, pane, window, cx).into_any_element()
    }
}

fn details_panel(
    selection: &InfoPaneSelection,
    show_calc_size: bool,
    pane: &InfoPane,
    window: &mut Window,
    cx: &mut Context<InfoPane>,
) -> AnyElement {
    match selection {
        InfoPaneSelection::None => v_flex()
            .w_full()
            .child(Alert::info(
                "info-pane-empty",
                t!("info_pane.empty").to_string(),
            ))
            .into_any_element(),
        InfoPaneSelection::Multiple(items) => multi_details_panel(items, cx).into_any_element(),
        InfoPaneSelection::Single(item) => {
            single_details_panel(item, show_calc_size, pane, window, cx).into_any_element()
        }
    }
}

fn multi_details_panel(items: &[FileItem], cx: &mut Context<InfoPane>) -> impl IntoElement {
    let summary = multi_select_summary(items);
    let title = t!("info_pane.multi.title", count = summary.count).to_string();
    let mut lines = vec![(
        t!("info_pane.multi.total_size").to_string(),
        format_size(summary.total_bytes),
    )];

    let type_line = format_multi_type_summary(&summary);
    if !type_line.is_empty() {
        lines.push((t!("info_pane.multi.types").to_string(), type_line));
    }

    let ext_counts = extension_type_counts(items);
    if !ext_counts.is_empty() {
        let ext_line = ext_counts
            .iter()
            .map(|(ext, count)| format_extension_count(ext, *count))
            .collect::<Vec<_>>()
            .join(", ");
        lines.push((t!("info_pane.multi.extensions").to_string(), ext_line));
    }

    details_header_and_list(title, lines, None::<&FileItem>, cx)
}

fn calculate_size_button(_window: &mut Window, cx: &mut Context<InfoPane>) -> impl IntoElement {
    Button::new("info-pane-calc-folder-size")
        .label(t!("info_pane.folder.calc_size").to_string())
        .on_click(cx.listener(|this, _, _, cx| {
            this.start_folder_size_calculation(cx);
        }))
}

fn single_details_panel(
    item: &FileItem,
    show_calc_size: bool,
    pane: &InfoPane,
    window: &mut Window,
    cx: &mut Context<InfoPane>,
) -> impl IntoElement {
    let name = item.display_name.clone();

    let mut lines = vec![
        (
            t!("info_pane.path").to_string(),
            item.path.display().to_string(),
        ),
        (t!("info_pane.type").to_string(), item_type_label(item)),
    ];

    if let Some(size) = item.size {
        lines.push((t!("info_pane.size").to_string(), format_size(size)));
    }
    if let Some(created) = item.created {
        lines.push((
            t!("info_pane.created").to_string(),
            format_system_time(created),
        ));
    }
    if let Some(modified) = item.modified {
        lines.push((
            t!("info_pane.modified").to_string(),
            format_system_time(modified),
        ));
    }
    if let Some(accessed) = item.accessed {
        lines.push((
            t!("info_pane.accessed").to_string(),
            format_system_time(accessed),
        ));
    }

    let attributes = format_attributes(item);
    if !attributes.is_empty() {
        lines.push((t!("info_pane.attributes").to_string(), attributes));
    }

    if item.kind == FileItemKind::Folder {
        if let Some(line) = pane.folder_contains_line() {
            lines.push(line);
        }
        if let Some(line) = pane.folder_size_line() {
            lines.push(line);
        }
    }

    v_flex()
        .w_full()
        .gap_3()
        .child(details_header_and_list(name, lines, Some(item), cx))
        .when(show_calc_size, |panel| panel.child(calculate_size_button(window, cx)))
}

fn details_header_and_list(
    title: String,
    lines: Vec<(String, String)>,
    tag_item: Option<&FileItem>,
    cx: &mut Context<InfoPane>,
) -> impl IntoElement {
    let rich_value_labels = [
        t!("info_pane.path").to_string(),
        t!("info_pane.type").to_string(),
        t!("info_pane.created").to_string(),
        t!("info_pane.modified").to_string(),
        t!("info_pane.accessed").to_string(),
    ];

    v_flex()
        .w_full()
        .gap_3()
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .text_color(cx.theme().foreground)
                .child(icon_foreground(IconName::Info, cx))
                .child(Label::new(title).text_sm().text_color(cx.theme().foreground)),
        )
        .when_some(tag_item.filter(|item| !item.tags.is_empty()), |panel, item| {
            panel.child(tag_chips_row(item, cx))
        })
        .child(
            DescriptionList::vertical()
                .bordered(false)
                .columns(1)
                .children(lines.into_iter().map(|(label, value)| {
                    let item = DescriptionItem::new(label.clone());
                    if rich_value_labels.iter().any(|rich| rich == &label) {
                        item.value(div().text_sm().child(value).into_any_element())
                    } else {
                        item.value(value)
                    }
                })),
        )
}

fn tag_chips_row(item: &FileItem, cx: &mut Context<InfoPane>) -> impl IntoElement {
    h_flex()
        .w_full()
        .gap_2()
        .flex_wrap()
        .child(
            Label::new(t!("info_pane.tags").to_string())
                .text_xs()
                .text_color(cx.theme().muted_foreground),
        )
        .children(item.tags.iter().map(|tag| {
            h_flex()
                .gap_1()
                .items_center()
                .child(tag_color_dot(tag.color.as_deref()))
                .child(
                    Label::new(tag.name.clone())
                        .text_xs()
                        .text_color(cx.theme().foreground),
                )
        }))
}

fn tag_color_dot(color: Option<&str>) -> impl IntoElement {
    let fill = color
        .and_then(parse_tag_color_hex)
        .map(rgb)
        .unwrap_or(rgb(0x54_6E_7A));
    div()
        .size(px(10.))
        .rounded_full()
        .flex_none()
        .bg(fill)
}

fn preview_panel(
    selection: &InfoPaneSelection,
    show_calc_size: bool,
    pane: &InfoPane,
    window: &mut Window,
    cx: &mut Context<InfoPane>,
) -> impl IntoElement {
    v_flex()
        .w_full()
        .gap_2()
        .when(matches!(selection, InfoPaneSelection::None), |panel| {
            panel.child(empty_preview())
        })
        .when(
            matches!(selection, InfoPaneSelection::Multiple(_)),
            |panel| {
                panel.child(Alert::info(
                    "info-pane-preview-multi",
                    t!("info_pane.preview.multi").to_string(),
                ))
            },
        )
        .when_some(
            match selection {
                InfoPaneSelection::Single(item) => Some(item),
                _ => None,
            },
            |panel, item| {
                if item.kind == FileItemKind::Folder {
                    panel.child(folder_preview_panel(show_calc_size, pane, window, cx))
                } else {
                    panel.child(file_preview_content(&item.path, pane, window, cx))
                }
            },
        )
}

fn file_preview_content(
    path: &Path,
    pane: &InfoPane,
    window: &mut Window,
    cx: &mut Context<InfoPane>,
) -> AnyElement {
    match preview_kind(path) {
        Some(PreviewKind::Image | PreviewKind::Svg) => preview_image_content(path).into_any_element(),
        Some(
            kind @ (
                PreviewKind::Markdown
                | PreviewKind::Html
                | PreviewKind::Code
                | PreviewKind::Text
            ),
        ) => preview_text_content(path, kind, cx),
        Some(PreviewKind::Pdf) => Alert::info(
            "info-pane-preview-pdf",
            t!("info_pane.preview.pdf").to_string(),
        )
        .into_any_element(),
        Some(PreviewKind::Audio) => audio_preview_panel(path, pane, window, cx).into_any_element(),
        Some(PreviewKind::Video) => video_preview_panel(path, pane, cx).into_any_element(),
        None => Alert::warning(
            "info-pane-preview-unsupported",
            t!("info_pane.preview.unsupported").to_string(),
        )
        .into_any_element(),
    }
}

fn audio_preview_panel(
    path: &Path,
    pane: &InfoPane,
    _window: &mut Window,
    cx: &mut Context<InfoPane>,
) -> impl IntoElement {
    let play_label = pane.audio_play_button_label(path);
    let play_error = pane
        .audio_preview
        .as_ref()
        .filter(|preview| preview.path == path)
        .and_then(|preview| preview.play_error.clone());
    let metadata = pane
        .audio_preview
        .as_ref()
        .filter(|preview| preview.path == path)
        .and_then(AudioPreview::metadata);
    let status = pane.audio_status.clone();
    let is_active = pane.audio_player.is_active_path(path);
    let is_paused = is_active && pane.audio_player.is_paused();
    let can_seek = is_active && pane.audio_total_duration(path).is_some();
    let time_line = pane.audio_time_line(path);
    let metadata_loading = pane.audio_preview.as_ref().is_some_and(|preview| {
        preview.path == path && preview.metadata == AudioMetadataState::Loading
    });
    let detail_lines = audio_metadata_lines(path, metadata);
    let metadata_title = metadata
        .and_then(|metadata| metadata.title.clone())
        .unwrap_or_else(|| preview_kind_title(PreviewKind::Audio));

    v_flex()
        .w_full()
        .gap_2()
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .child(icon_foreground(IconName::File, cx))
                .child(
                    Label::new(metadata_title)
                        .text_sm()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(cx.theme().foreground),
                ),
        )
        .child(
            Slider::new(&pane.audio_seek_slider)
                .disabled(!can_seek)
                .w_full(),
        )
        .child(
            Label::new(if metadata_loading && !is_active {
                t!("info_pane.folder.loading").to_string()
            } else {
                time_line
            })
            .text_xs()
            .text_color(cx.theme().muted_foreground),
        )
        .children(detail_lines.into_iter().map(|(label, value)| {
            h_flex()
                .w_full()
                .justify_between()
                .gap_3()
                .child(
                    Label::new(label)
                        .text_xs()
                        .text_color(cx.theme().muted_foreground),
                )
                .child(
                    Label::new(value)
                        .text_xs()
                        .text_color(cx.theme().foreground),
                )
        }))
        .child(
            h_flex()
                .gap_2()
                .flex_wrap()
                .child(
                    Button::new("info-pane-audio-play")
                        .label(play_label)
                        .on_click(cx.listener(|this, _, window, cx| {
                            cx.stop_propagation();
                            this.toggle_audio_playback(window, cx);
                        })),
                )
                .child(
                    Button::new("info-pane-audio-backward")
                        .label(t!("info_pane.audio.backward").to_string())
                        .when(!can_seek, |this| this.opacity(0.5))
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            if this.selected_audio_path().is_some() {
                                this.seek_audio_relative(-5.0, cx);
                            }
                        })),
                )
                .child(
                    Button::new("info-pane-audio-forward")
                        .label(t!("info_pane.audio.forward").to_string())
                        .when(!can_seek, |this| this.opacity(0.5))
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            if this.selected_audio_path().is_some() {
                                this.seek_audio_relative(5.0, cx);
                            }
                        })),
                )
                .when(is_active || is_paused || pane.audio_is_finished(path), |row| {
                    row.child(
                        Button::new("info-pane-audio-stop")
                            .label(t!("info_pane.audio.stop").to_string())
                            .on_click(cx.listener(|this, _, window, cx| {
                                cx.stop_propagation();
                                this.stop_audio_playback(window, cx);
                            })),
                    )
                }),
        )
        .when_some(status, |panel, message| {
            panel.child(
                Label::new(message)
                    .text_xs()
                    .text_color(cx.theme().muted_foreground),
            )
        })
        .when_some(play_error, |panel, error| {
            panel.child(Alert::error(
                "info-pane-audio-play-error",
                format!("{}: {error}", t!("info_pane.audio.play_error")),
            ))
        })
}

fn audio_metadata_lines(path: &Path, metadata: Option<&AudioFileMetadata>) -> Vec<(String, String)> {
    let mut lines = Vec::new();

    if let Some(metadata) = metadata {
        if let Some(artist) = metadata.artist.as_ref() {
            lines.push((t!("info_pane.audio.artist").to_string(), artist.clone()));
        }
        if let Some(album) = metadata.album.as_ref() {
            lines.push((t!("info_pane.audio.album").to_string(), album.clone()));
        }
        if let Some(codec) = metadata.codec.as_ref() {
            lines.push((t!("info_pane.audio.codec").to_string(), codec.clone()));
        }
        if let Some(sample_rate) = metadata.sample_rate {
            lines.push((
                t!("info_pane.audio.sample_rate").to_string(),
                format_audio_sample_rate(sample_rate),
            ));
        }
        if let Some(channels) = metadata.channels {
            lines.push((
                t!("info_pane.audio.channels").to_string(),
                format_audio_channels(channels),
            ));
        }
        if let Some(bitrate) = metadata.bitrate_kbps {
            lines.push((
                t!("info_pane.audio.bitrate").to_string(),
                format!("{bitrate} kbps"),
            ));
        }
        if let Some(file_size) = metadata.file_size {
            lines.push((
                t!("info_pane.audio.file_size").to_string(),
                format_size(file_size),
            ));
        }
    } else if let Ok(file_meta) = std::fs::metadata(path) {
        lines.push((
            t!("info_pane.audio.file_size").to_string(),
            format_size(file_meta.len()),
        ));
    }

    lines
}

fn video_preview_panel(
    path: &Path,
    pane: &InfoPane,
    cx: &mut Context<InfoPane>,
) -> impl IntoElement {
    let preview = pane
        .video_preview
        .as_ref()
        .filter(|preview| preview.path == path);
    let metadata = preview.and_then(video_preview_metadata);
    let metadata_loading = preview.is_some_and(|preview| preview.metadata == VideoMetadataState::Loading);
    let preview_error = preview.and_then(|preview| preview.preview_error.clone());
    let is_playing = preview.is_some_and(|preview| matches!(preview.playback, VideoPlaybackState::Starting | VideoPlaybackState::Playing));
    let is_paused = preview.is_some_and(|preview| matches!(preview.playback, VideoPlaybackState::Paused));
    #[cfg(windows)]
    let embed_active = pane.embedded_video_active_for_path(path);
    #[cfg(not(windows))]
    let embed_active = false;
    let can_seek = metadata.and_then(|metadata| metadata.duration).is_some();
    let playback_label = if is_playing {
        t!("info_pane.video.pause").to_string()
    } else if is_paused {
        t!("info_pane.video.resume").to_string()
    } else if preview.is_some_and(|preview| matches!(preview.playback, VideoPlaybackState::Finished)) {
        t!("info_pane.video.replay").to_string()
    } else {
        t!("info_pane.video.play").to_string()
    };
    let status = pane.video_status.clone().or_else(|| {
        preview.map(|preview| video_time_line(preview, metadata))
    });
    let detail_lines = video_metadata_lines(path, metadata);
    #[cfg(windows)]
    let entity = cx.entity().downgrade();

    v_flex()
        .w_full()
        .gap_2()
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .child(icon_foreground(IconName::File, cx))
                .child(
                    Label::new(preview_kind_title(PreviewKind::Video))
                        .text_sm()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(cx.theme().foreground),
                ),
        )
        .when(embed_active, |panel| {
            #[cfg(windows)]
            {
                panel.child(
                    div()
                        .w_full()
                        .h(px(220.))
                        .rounded(cx.theme().radius)
                        .border_1()
                        .border_color(cx.theme().primary.opacity(0.35))
                        .bg(gpui::rgba(0x0d121aff))
                        .on_prepaint(move |bounds, _, cx| {
                            let _ = entity.update(cx, |this, cx| {
                                let changed = this
                                    .video_host_bounds
                                    .map(|prev| {
                                        (prev.origin.x - bounds.origin.x).abs() > px(0.5)
                                            || (prev.origin.y - bounds.origin.y).abs() > px(0.5)
                                            || (prev.size.width - bounds.size.width).abs() > px(0.5)
                                            || (prev.size.height - bounds.size.height).abs() > px(0.5)
                                    })
                                    .unwrap_or(true);
                                if changed {
                                    this.video_host_bounds = Some(bounds);
                                    cx.notify();
                                }
                            });
                        }),
                )
            }
            #[cfg(not(windows))]
            {
                panel
            }
        })
        .when(!embed_active, |panel| {
            panel.child(
                div()
                    .w_full()
                    .h(px(220.))
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().muted)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Label::new(t!("info_pane.folder.loading").to_string())
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
        })
        .when(metadata_loading, |panel| {
            panel.child(
                Label::new(t!("info_pane.folder.loading").to_string())
                    .text_xs()
                    .text_color(cx.theme().muted_foreground),
            )
        })
        .child(
            v_flex()
                .w_full()
                .gap_1()
                .child(
                    Slider::new(&pane.video_seek_slider)
                        .disabled(!can_seek)
                        .w_full(),
                ),
        )
        .child(
            h_flex()
                .gap_2()
                .flex_wrap()
                .child(
                    Button::new("info-pane-video-play")
                        .label(playback_label)
                        .on_click(cx.listener(|this, _, window, cx| {
                            cx.stop_propagation();
                            this.toggle_video_playback(window, cx);
                        })),
                ),
        )
        .child(
            h_flex()
                .gap_2()
                .flex_wrap()
                .child(
                    Button::new("info-pane-video-backward")
                        .label(t!("info_pane.video.backward").to_string())
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.seek_video_relative(-5.0, cx);
                        })),
                )
                .child(
                    Button::new("info-pane-video-forward")
                        .label(t!("info_pane.video.forward").to_string())
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.seek_video_relative(5.0, cx);
                        })),
                )
                .when(is_playing || is_paused || preview.is_some_and(|preview| matches!(preview.playback, VideoPlaybackState::Finished)), |row| {
                    row.child(
                        Button::new("info-pane-video-stop")
                            .label(t!("info_pane.video.stop").to_string())
                            .on_click(cx.listener(|this, _, _, cx| {
                                cx.stop_propagation();
                                this.stop_video_playback(cx);
                            })),
                    )
                }),
        )
        .when_some(status, |panel, status| {
            panel.child(
                Label::new(status)
                    .text_xs()
                    .text_color(cx.theme().muted_foreground),
            )
        })
        .children(detail_lines.into_iter().map(|(label, value)| {
            h_flex()
                .w_full()
                .justify_between()
                .gap_3()
                .child(
                    Label::new(label)
                        .text_xs()
                        .text_color(cx.theme().muted_foreground),
                )
                .child(
                    Label::new(value)
                        .text_xs()
                        .text_color(cx.theme().foreground),
                )
        }))
        .when_some(preview_error, |panel, error| {
            panel.child(Alert::error(
                "info-pane-video-preview-error",
                format!("{}: {error}", t!("info_pane.preview.error")),
            ))
        })
}

fn video_preview_metadata(preview: &VideoPreview) -> Option<&VideoFileMetadata> {
    match &preview.metadata {
        VideoMetadataState::Ready(metadata) => Some(metadata),
        _ => None,
    }
}

fn video_time_line(preview: &VideoPreview, metadata: Option<&VideoFileMetadata>) -> String {
    let total = metadata.and_then(|metadata| metadata.duration);
    if matches!(preview.playback, VideoPlaybackState::Finished) {
        if let Some(total) = total {
            format!(
                "{} {} / {}",
                t!("info_pane.video.ended"),
                format_audio_duration(preview.current_position),
                format_audio_duration(total)
            )
        } else {
            t!("info_pane.video.ended").to_string()
        }
    } else if let Some(total) = total {
        format!(
            "{} / {}",
            format_audio_duration(preview.current_position),
            format_audio_duration(total)
        )
    } else if matches!(preview.playback, VideoPlaybackState::Starting) {
        t!("info_pane.video.starting").to_string()
    } else {
        format_audio_duration(preview.current_position)
    }
}

fn video_progress_fraction(preview: &VideoPreview, metadata: Option<&VideoFileMetadata>) -> f32 {
    let Some(total) = metadata.and_then(|metadata| metadata.duration) else {
        return 0.;
    };
    if total.is_zero() {
        return 0.;
    }
    (preview.current_position.as_secs_f32() / total.as_secs_f32()).clamp(0., 1.)
}

fn video_metadata_lines(path: &Path, metadata: Option<&VideoFileMetadata>) -> Vec<(String, String)> {
    let mut lines = Vec::new();

    if let Some(metadata) = metadata {
        lines.push((
            t!("info_pane.video.duration").to_string(),
            metadata
                .duration
                .map(format_audio_duration)
                .unwrap_or_else(|| t!("info_pane.video.duration_unknown").to_string()),
        ));
        if let (Some(width), Some(height)) = (metadata.width, metadata.height) {
            lines.push((
                t!("info_pane.video.resolution").to_string(),
                format!("{width} × {height}"),
            ));
        }
        if let Some(frame_rate_milli) = metadata.frame_rate_milli {
            lines.push((
                t!("info_pane.video.frame_rate").to_string(),
                format_video_frame_rate(frame_rate_milli),
            ));
        }
        if let Some(codec) = metadata.codec.as_ref() {
            lines.push((t!("info_pane.video.codec").to_string(), codec.clone()));
        }
        if let Some(bitrate) = metadata.bitrate_kbps {
            lines.push((
                t!("info_pane.video.bitrate").to_string(),
                format!("{bitrate} kbps"),
            ));
        }
        lines.push((
            t!("info_pane.video.audio").to_string(),
            if metadata.has_audio {
                t!("info_pane.video.audio_yes").to_string()
            } else {
                t!("info_pane.video.audio_no").to_string()
            },
        ));
        if let Some(file_size) = metadata.file_size {
            lines.push((
                t!("info_pane.video.file_size").to_string(),
                format_size(file_size),
            ));
        }
    } else if let Ok(file_size) = std::fs::metadata(path).map(|meta| meta.len()) {
        lines.push((
            t!("info_pane.video.file_size").to_string(),
            format_size(file_size),
        ));
    }

    lines
}

fn format_audio_duration(duration: Duration) -> String {
    let total = duration.as_secs();
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let seconds = total % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

fn format_video_frame_rate(frame_rate_milli: u32) -> String {
    let fps = frame_rate_milli as f32 / 1000.;
    if (fps - fps.round()).abs() < 0.01 {
        format!("{:.0} fps", fps)
    } else {
        format!("{fps:.2} fps")
    }
}

fn format_audio_sample_rate(sample_rate: u32) -> String {
    if sample_rate % 1000 == 0 {
        format!("{} kHz", sample_rate / 1000)
    } else {
        format!("{:.1} kHz", sample_rate as f32 / 1000.)
    }
}

fn format_audio_channels(channels: u16) -> String {
    match channels {
        1 => t!("info_pane.audio.channels_mono").to_string(),
        2 => t!("info_pane.audio.channels_stereo").to_string(),
        n => t!("info_pane.audio.channels_count", count = n).to_string(),
    }
}

fn folder_preview_panel(
    show_calc_size: bool,
    pane: &InfoPane,
    window: &mut Window,
    cx: &mut Context<InfoPane>,
) -> impl IntoElement {
    let lines: Vec<String> = pane
        .folder_contains_line()
        .into_iter()
        .map(|(_, value)| value)
        .chain(pane.folder_size_line().into_iter().map(|(_, value)| value))
        .collect();
    let lines_empty = lines.is_empty();

    v_flex()
        .w_full()
        .gap_2()
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .child(icon_foreground(IconName::FolderOpen, cx))
                .child(
                    Label::new(t!("info_pane.preview.folder_summary").to_string())
                        .text_sm()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(cx.theme().foreground),
                ),
        )
        .children(lines.iter().map(|line| {
            Label::new(line.clone())
                .text_sm()
                .text_color(cx.theme().foreground)
        }))
        .when(show_calc_size, |panel| panel.child(calculate_size_button(window, cx)))
        .when(lines_empty && !show_calc_size, |panel| {
            panel.child(Alert::info(
                "info-pane-preview-folder-loading",
                t!("info_pane.folder.loading").to_string(),
            ))
        })
}

fn preview_image_content(path: &std::path::Path) -> impl IntoElement {
    img(path.to_path_buf())
        .w_full()
        .max_h(px(360.))
        .object_fit(ObjectFit::Contain)
}

fn preview_text_content(
    path: &std::path::Path,
    kind: PreviewKind,
    cx: &mut Context<InfoPane>,
) -> AnyElement {
    match read_text_preview(path) {
        Ok(text) => {
            let is_code_like = matches!(
                kind,
                PreviewKind::Code | PreviewKind::Html | PreviewKind::Markdown
            );
            v_flex()
                .w_full()
                .gap_2()
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(icon_foreground(IconName::File, cx))
                        .child(
                            Label::new(preview_kind_title(kind))
                                .text_sm()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(cx.theme().foreground),
                        )
                        .child(
                            Label::new(
                                path.extension()
                                    .and_then(|ext| ext.to_str())
                                    .map(|ext| format!(".{ext}"))
                                    .unwrap_or_default(),
                            )
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                        ),
                )
                .child(
                    div()
                        .w_full()
                        .max_h(px(420.))
                        .overflow_y_scrollbar()
                        .p_2()
                        .rounded(cx.theme().radius)
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().muted)
                        .child(
                            div()
                                .w_full()
                                .text_xs()
                                .text_color(cx.theme().foreground)
                                .when(is_code_like, |this| {
                                    this.font_family(cx.theme().mono_font_family.clone())
                                })
                                .child(text),
                        ),
                )
                .into_any_element()
        }
        Err(error) => Alert::error(
            "info-pane-preview-error",
            format!("{}: {error}", t!("info_pane.preview.error")),
        )
        .into_any_element(),
    }
}

fn empty_preview() -> Alert {
    Alert::info(
        "info-pane-preview-empty",
        t!("info_pane.preview.empty").to_string(),
    )
}

fn preview_kind_title(kind: PreviewKind) -> String {
    match kind {
        PreviewKind::Image => t!("info_pane.preview.kind.image").to_string(),
        PreviewKind::Svg => t!("info_pane.preview.kind.svg").to_string(),
        PreviewKind::Markdown => t!("info_pane.preview.kind.markdown").to_string(),
        PreviewKind::Html => t!("info_pane.preview.kind.html").to_string(),
        PreviewKind::Code => t!("info_pane.preview.kind.code").to_string(),
        PreviewKind::Text => t!("info_pane.preview.kind.text").to_string(),
        PreviewKind::Pdf => t!("info_pane.preview.kind.pdf").to_string(),
        PreviewKind::Audio => t!("info_pane.preview.kind.audio").to_string(),
        PreviewKind::Video => t!("info_pane.preview.kind.video").to_string(),
    }
}

fn format_multi_type_summary(
    summary: &files_fs::MultiSelectSummary,
) -> String {
    let mut parts = Vec::new();
    if summary.files > 0 {
        parts.push(t!("info_pane.multi.files", count = summary.files).to_string());
    }
    if summary.folders > 0 {
        parts.push(t!("info_pane.multi.folders", count = summary.folders).to_string());
    }
    if summary.symlinks > 0 {
        parts.push(t!("info_pane.multi.symlinks", count = summary.symlinks).to_string());
    }
    if summary.other > 0 {
        parts.push(t!("info_pane.multi.other", count = summary.other).to_string());
    }
    parts.join(", ")
}

fn format_extension_count(ext: &str, count: usize) -> String {
    if ext.is_empty() {
        t!("info_pane.multi.no_extension", count = count).to_string()
    } else {
        t!("info_pane.multi.extension_item", ext = ext, count = count).to_string()
    }
}

fn format_attributes(item: &FileItem) -> String {
    let mut attrs = Vec::new();
    if item.kind == FileItemKind::Folder {
        attrs.push(t!("info_pane.attr.directory").to_string());
    }
    if item.is_readonly {
        attrs.push(t!("info_pane.attr.readonly").to_string());
    }
    if item.is_hidden {
        attrs.push(t!("info_pane.attr.hidden").to_string());
    }
    if item.is_system {
        attrs.push(t!("info_pane.attr.system").to_string());
    }
    attrs.join(", ")
}

fn item_type_label(item: &FileItem) -> String {
    match item.kind {
        FileItemKind::Folder => t!("files.type.folder").to_string(),
        FileItemKind::Symlink => t!("files.type.symlink").to_string(),
        FileItemKind::Other => t!("files.type.other").to_string(),
        FileItemKind::File => item
            .extension
            .as_ref()
            .map(|e| format!("{} file", e.to_uppercase()))
            .unwrap_or_else(|| t!("files.type.file").to_string()),
    }
}

fn format_size(size: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = size as f64;
    let mut unit = 0;
    while value >= 1024. && unit < UNITS.len() - 1 {
        value /= 1024.;
        unit += 1;
    }
    if unit == 0 {
        format!("{size} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn format_system_time(time: std::time::SystemTime) -> String {
    use chrono::{DateTime, Local};
    let local_time: DateTime<Local> = time.into();
    local_time.format("%Y-%m-%d %H:%M").to_string()
}
