use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gpui::{
    div, px, size, App, AppContext, Bounds, Context, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Render, SharedString, Size, StatefulInteractiveElement, Styled,
    Subscription, Window, WindowBounds, WindowKind, WindowOptions,
};
use gpui::prelude::FluentBuilder;
use gpui_component::{
    button::Button, h_flex, slider::Slider, slider::SliderEvent, slider::SliderState, v_flex,
    ActiveTheme, ElementExt, Root,
};

use app_ui::title_bar::TitleBar;

use crate::audio_player::AudioPlayer;
use crate::audio_visualizer::{AudioVisualizer, SPECTRUM_BANDS};
use crate::media_player_config::MediaPlayerConfig;
use crate::playlist::Playlist;
use crate::video_surface::{window_hwnd, NativeVideoSurface};

#[cfg(windows)]
use app_mpv_ffi::MpvEmbedPlayer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MediaType {
    Audio,
    Video,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoopMode {
    None,
    Single,
    All,
}

fn is_subtitle_file(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    matches!(
        ext.as_str(),
        "srt" | "ass" | "ssa" | "vtt" | "sub" | "idx" | "smi"
    )
}

fn is_media_file(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    matches!(
        ext.as_str(),
        "mp4" | "mkv"
            | "avi"
            | "mov"
            | "wmv"
            | "flv"
            | "webm"
            | "mp3"
            | "wav"
            | "flac"
            | "aac"
            | "ogg"
            | "m4a"
            | "wma"
    )
}

fn pseudo_random(max: usize, seed: usize) -> usize {
    (seed.wrapping_mul(2654435761_usize) % max.max(1)) as usize
}

pub struct PlayerPage {
    focus_handle: FocusHandle,
    playlist: Playlist,
    media_type: Option<MediaType>,
    #[cfg(windows)]
    embedded_player: Option<MpvEmbedPlayer>,
    #[cfg(windows)]
    video_surface: Option<NativeVideoSurface>,
    #[cfg(windows)]
    video_host_bounds: Option<Bounds<gpui::Pixels>>,
    audio_player: Option<AudioPlayer>,
    current_position: Option<Duration>,
    total_duration: Option<Duration>,
    is_playing: bool,
    is_paused: bool,
    poll_generation: u64,
    load_attempted: bool,
    pending_next: bool,
    pending_load: bool,
    loop_mode: LoopMode,
    shuffle: bool,
    speed: f64,
    config: MediaPlayerConfig,
    seek_slider: gpui::Entity<SliderState>,
    volume_slider: gpui::Entity<SliderState>,
    seek_dragging: bool,
    volume: f64,
    muted: bool,
    sub_visible: bool,
    sub_tracks: Vec<app_mpv_ffi::SubtitleTrack>,
    current_sub_id: Option<i64>,
    visualizer: Option<AudioVisualizer>,
    visualizer_spectrum: Vec<f32>,
    visualizer_generation: u64,
    spectrum_max_height: f32,
    _seek_subscription: Subscription,
    _volume_subscription: Subscription,
}

impl PlayerPage {
    pub fn new(initial_paths: Vec<PathBuf>, cx: &mut Context<Self>) -> Self {
        let seek_slider =
            cx.new(|_| SliderState::new().min(0.0).max(1.0).step(0.001).default_value(0.0));
        let seek_subscription = cx.subscribe(&seek_slider, |this, _, event: &SliderEvent, cx| {
            match event {
                SliderEvent::Change(_) => {
                    this.seek_dragging = true;
                }
                SliderEvent::Release(value) => {
                    this.seek_dragging = false;
                    this.commit_seek(value.start(), cx);
                }
            }
        });

        let config = MediaPlayerConfig::load();
        let default_volume = (config.volume.clamp(0.0, 100.0) / 100.0) as f32;
        let volume_slider =
            cx.new(|_| SliderState::new().min(0.0).max(1.0).step(0.01).default_value(default_volume));
        let volume_subscription =
            cx.subscribe(&volume_slider, |this, _, event: &SliderEvent, cx| {
                match event {
                    SliderEvent::Change(value) | SliderEvent::Release(value) => {
                        this.volume = f64::from(value.start()) * 100.0;
                        this.apply_volume(cx);
                    }
                }
            });
        Self {
            focus_handle: cx.focus_handle(),
            playlist: Playlist::from_paths(initial_paths),
            media_type: None,
            #[cfg(windows)]
            embedded_player: None,
            #[cfg(windows)]
            video_surface: None,
            #[cfg(windows)]
            video_host_bounds: None,
            audio_player: None,
            current_position: None,
            total_duration: None,
            is_playing: false,
            is_paused: false,
            poll_generation: 0,
            load_attempted: false,
            pending_next: false,
            pending_load: false,
            loop_mode: LoopMode::None,
            shuffle: false,
            speed: 1.0,
            seek_slider,
            volume_slider,
            seek_dragging: false,
            volume: config.volume.clamp(0.0, 100.0),
            muted: config.muted,
            sub_visible: true,
            sub_tracks: Vec::new(),
            current_sub_id: None,
            visualizer: None,
            visualizer_spectrum: vec![0.0; SPECTRUM_BANDS],
            visualizer_generation: 0,
            spectrum_max_height: 180.0,
            config,
            _seek_subscription: seek_subscription,
            _volume_subscription: volume_subscription,
        }
    }

    pub fn view(
        paths: Vec<PathBuf>,
        _window: &mut Window,
        cx: &mut App,
    ) -> gpui::Entity<Self> {
        cx.new(|cx| Self::new(paths, cx))
    }

    fn load_media(
        &mut self,
        path: &Path,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        self.stop();
        let info = app_mpv_ffi::probe_media(path).ok();
        let is_video = info.as_ref().and_then(|i| i.video_codec.as_ref()).is_some();
        self.total_duration = info.as_ref().and_then(|i| i.duration);

        if is_video {
            self.visualizer = None;
            self.load_video(path, window, cx)?;
            self.media_type = Some(MediaType::Video);
        } else {
            self.visualizer = AudioVisualizer::start();
            self.start_visualizer_poll(cx);
            if self.audio_player.is_none() {
                self.audio_player = Some(AudioPlayer::start());
            }
            if let Some(player) = self.audio_player.as_ref() {
                player.play(path.to_path_buf());
                self.media_type = Some(MediaType::Audio);
                self.is_playing = true;
                self.is_paused = false;
            }
        }

        self.apply_volume(cx);
        self.apply_speed();

        if self.config.remember_position {
            if let Some(secs) = self.config.get_position(&path.to_path_buf()) {
                let target = Duration::from_secs_f64(secs);
                match self.media_type {
                    Some(MediaType::Video) => {
                        #[cfg(windows)]
                        if let Some(player) = self.embedded_player.as_mut() {
                            let _ = player.seek_to(target);
                        }
                    }
                    Some(MediaType::Audio) => {
                        if let Some(player) = self.audio_player.as_ref() {
                            player.seek_to(target);
                        }
                    }
                    None => {}
                }
                self.current_position = Some(target);
            }
        }

        self.start_poll(cx);
        Ok(())
    }

    #[cfg(windows)]
    fn ensure_video_surface(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<isize> {
        if let Some(surface) = self.video_surface.as_ref() {
            return Ok(surface.hwnd());
        }
        let parent_hwnd =
            window_hwnd(window).ok_or_else(|| anyhow::anyhow!("resolve top-level hwnd"))?;
        let surface = NativeVideoSurface::new(parent_hwnd)?;
        let hwnd = surface.hwnd();
        self.video_surface = Some(surface);
        cx.notify();
        Ok(hwnd)
    }

    #[cfg(windows)]
    fn update_video_surface_bounds(&self, window: &Window) {
        let Some(surface) = self.video_surface.as_ref() else {
            return;
        };
        let Some(bounds) = self.video_host_bounds else {
            surface.set_visible(false);
            return;
        };
        surface.set_bounds(window, bounds);
    }

    #[cfg(windows)]
    fn load_video(
        &mut self,
        path: &Path,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let target_wid = self.ensure_video_surface(window, cx)?;
        if self.embedded_player.is_none() {
            self.embedded_player = Some(MpvEmbedPlayer::new(target_wid)?);
        }
        let player = self.embedded_player.as_mut().unwrap();
        player.load_file(path)?;
        self.is_playing = true;
        self.is_paused = false;
        self.auto_load_subtitles(path);
        Ok(())
    }

    #[cfg(not(windows))]
    fn load_video(
        &mut self,
        _path: &Path,
        _window: &Window,
        _cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        anyhow::bail!("video playback is only supported on Windows")
    }

    fn start_poll(&mut self, cx: &mut Context<Self>) {
        self.poll_generation = self.poll_generation.wrapping_add(1);
        let generation = self.poll_generation;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_secs(1))
                    .await;

                let mut keep_polling = false;
                let update_ok = this.update(cx, |this, _cx| {
                    if this.poll_generation != generation {
                        return;
                    }
                    keep_polling = this.is_playing;
                    if !keep_polling {
                        return;
                    }

                    match this.media_type {
                        Some(MediaType::Video) => {
                            #[cfg(windows)]
                            if let Some(player) = this.embedded_player.as_mut() {
                                player.poll_events();
                                this.current_position = player.time_pos().unwrap_or(None);
                                if let (Some(pos), Some(path)) = (this.current_position, this.playlist.current().cloned()) {
                                    this.config.record_position(&path, pos.as_secs_f64());
                                }
                                if let Ok(tracks) = player.subtitle_tracks() {
                                    this.sub_tracks = tracks;
                                }
                                if let Ok(sid) = player.current_sid() {
                                    this.current_sub_id = sid;
                                    this.sub_visible = sid.is_some();
                                }
                                if player.ended() {
                                    if this.loop_mode == LoopMode::Single {
                                        let _ = player.seek_to(Duration::ZERO);
                                        this.current_position = Some(Duration::ZERO);
                                    } else {
                                        this.is_playing = false;
                                        this.is_paused = false;
                                        this.pending_next = true;
                                        keep_polling = false;
                                    }
                                }
                            }
                            _cx.notify();
                        }
                        Some(MediaType::Audio) => {
                            if let Some(player) = this.audio_player.as_ref() {
                                let state = player.snapshot();
                                this.current_position = Some(state.position);
                                this.total_duration = state.total;
                                if let (Some(pos), Some(path)) = (this.current_position, this.playlist.current().cloned()) {
                                    this.config.record_position(&path, pos.as_secs_f64());
                                }
                                if state.finished {
                                    if this.loop_mode == LoopMode::Single {
                                        if let Some(path) = this.playlist.current().cloned() {
                                            player.play(path);
                                        }
                                    } else {
                                        this.is_playing = false;
                                        this.is_paused = false;
                                        this.pending_next = true;
                                        keep_polling = false;
                                    }
                                }
                            }
                            _cx.notify();
                        }
                        None => {}
                    }
                });
                if update_ok.is_err() || !keep_polling {
                    break;
                }
            }
        })
        .detach();
    }

    fn play_item(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.playlist.select(index).is_some() {
            self.load_attempted = true;
            if let Some(path) = self.playlist.current().cloned() {
                self.config.add_recent(path.clone());
                self.save_config();
                let _ = self.load_media(&path, window, cx);
            }
        }
    }

    fn play_next(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let path = if self.loop_mode == LoopMode::Single {
            self.playlist.current().cloned()
        } else if self.shuffle && self.playlist.len() > 1 {
            let current = self.playlist.current_index().unwrap_or(0);
            let seed = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as usize;
            let next = pseudo_random(self.playlist.len(), seed.wrapping_add(current * 31));
            let next = if next == current {
                (next + 1) % self.playlist.len()
            } else {
                next
            };
            self.playlist.select(next).cloned()
        } else {
            let has_next = self.playlist.next().is_some();
            if !has_next && self.loop_mode == LoopMode::All {
                self.playlist.select(0).cloned()
            } else {
                self.playlist.current().cloned()
            }
        };

        if let Some(path) = path {
            self.load_attempted = true;
            self.config.add_recent(path.clone());
            self.save_config();
            let _ = self.load_media(&path, window, cx);
        }
    }

    fn play_prev(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let path = if self.shuffle && self.playlist.len() > 1 {
            let current = self.playlist.current_index().unwrap_or(0);
            let seed = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as usize;
            let prev = pseudo_random(self.playlist.len(), seed.wrapping_add(current * 17));
            let prev = if prev == current {
                (prev + 1) % self.playlist.len()
            } else {
                prev
            };
            self.playlist.select(prev).cloned()
        } else {
            let has_prev = self.playlist.prev().is_some();
            if !has_prev && self.loop_mode == LoopMode::All {
                let last = self.playlist.len().saturating_sub(1);
                self.playlist.select(last).cloned()
            } else {
                self.playlist.current().cloned()
            }
        };

        if let Some(path) = path {
            self.load_attempted = true;
            self.config.add_recent(path.clone());
            self.save_config();
            let _ = self.load_media(&path, window, cx);
        }
    }

    fn toggle_pause(&mut self) {
        match self.media_type {
            Some(MediaType::Video) => {
                #[cfg(windows)]
                if let Some(player) = self.embedded_player.as_mut() {
                    if self.is_paused {
                        let _ = player.set_pause(false);
                        self.is_paused = false;
                    } else {
                        let _ = player.set_pause(true);
                        self.is_paused = true;
                    }
                }
            }
            Some(MediaType::Audio) => {
                if let Some(player) = self.audio_player.as_ref() {
                    player.toggle_pause();
                    self.is_paused = !self.is_paused;
                }
            }
            None => {}
        }
    }

    fn stop(&mut self) {
        match self.media_type {
            Some(MediaType::Video) => {
                #[cfg(windows)]
                if let Some(player) = self.embedded_player.as_mut() {
                    let _ = player.stop();
                }
            }
            Some(MediaType::Audio) => {
                if let Some(player) = self.audio_player.as_ref() {
                    player.stop();
                }
            }
            None => {}
        }
        #[cfg(windows)]
        if let Some(surface) = self.video_surface.as_ref() {
            surface.set_visible(false);
        }
        self.video_host_bounds = None;
        self.is_playing = false;
        self.is_paused = false;
        self.current_position = None;
        self.visualizer = None;
    }

    fn start_visualizer_poll(&mut self, cx: &mut Context<Self>) {
        self.visualizer_generation = self.visualizer_generation.wrapping_add(1);
        let generation = self.visualizer_generation;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(33)).await;
                let ok = this.update(cx, |this, cx| {
                    if this.visualizer_generation != generation {
                        return false;
                    }
                    if let Some(viz) = this.visualizer.as_ref() {
                        this.visualizer_spectrum = viz.spectrum();
                        cx.notify();
                    }
                    true
                });
                if ok.is_err() || !ok.unwrap_or(false) {
                    break;
                }
            }
        }).detach();
    }

    fn commit_seek(&mut self, fraction: f32, cx: &mut Context<Self>) {
        let Some(total) = self.total_duration else { return };
        let target = Duration::from_secs_f64(
            total.as_secs_f64() * f64::from(fraction.clamp(0.0, 1.0)),
        );

        match self.media_type {
            Some(MediaType::Video) => {
                #[cfg(windows)]
                if let Some(player) = self.embedded_player.as_mut() {
                    let _ = player.seek_to(target);
                }
            }
            Some(MediaType::Audio) => {
                if let Some(player) = self.audio_player.as_ref() {
                    player.seek_to(target);
                }
            }
            None => {}
        }
        self.current_position = Some(target);
        cx.notify();
    }

    fn save_config(&mut self) {
        self.config.volume = self.volume;
        self.config.muted = self.muted;
        self.config.save();
    }

    fn apply_volume(&mut self, _cx: &mut Context<Self>) {
        let effective_volume = if self.muted { 0.0 } else { self.volume };
        match self.media_type {
            Some(MediaType::Video) => {
                #[cfg(windows)]
                if let Some(player) = self.embedded_player.as_mut() {
                    let _ = player.set_volume(effective_volume);
                }
            }
            Some(MediaType::Audio) => {
                if let Some(player) = self.audio_player.as_ref() {
                    player.set_volume(effective_volume);
                }
            }
            None => {}
        }
        self.save_config();
    }

    fn apply_speed(&mut self) {
        match self.media_type {
            Some(MediaType::Video) => {
                #[cfg(windows)]
                if let Some(player) = self.embedded_player.as_mut() {
                    let _ = player.set_speed(self.speed);
                }
            }
            Some(MediaType::Audio) => {
                if let Some(player) = self.audio_player.as_ref() {
                    player.set_speed(self.speed);
                }
            }
            None => {}
        }
    }

    fn toggle_mute(&mut self, cx: &mut Context<Self>) {
        self.muted = !self.muted;
        self.apply_volume(cx);
        self.save_config();
    }

    fn toggle_loop_mode(&mut self) {
        self.loop_mode = match self.loop_mode {
            LoopMode::None => LoopMode::All,
            LoopMode::All => LoopMode::Single,
            LoopMode::Single => LoopMode::None,
        };
    }

    fn toggle_shuffle(&mut self) {
        self.shuffle = !self.shuffle;
    }

    fn toggle_speed(&mut self) {
        self.speed = match (self.speed * 10.0).round() as i64 {
            10 => 1.5,
            15 => 2.0,
            _ => 1.0,
        };
        self.apply_speed();
    }

    fn toggle_sub_visibility(&mut self) {
        self.sub_visible = !self.sub_visible;
        #[cfg(windows)]
        if let Some(player) = self.embedded_player.as_mut() {
            let _ = player.set_sub_visibility(self.sub_visible);
        }
    }

    fn cycle_sub_track(&mut self) {
        #[cfg(windows)]
        if let Some(player) = self.embedded_player.as_mut() {
            if let Ok(tracks) = player.subtitle_tracks() {
                self.sub_tracks = tracks;
            }
            let ids: Vec<i64> = self.sub_tracks.iter().map(|t| t.id).collect();
            if ids.is_empty() {
                return;
            }
            let next_id = match self.current_sub_id {
                Some(id) => {
                    let pos = ids.iter().position(|&x| x == id);
                    match pos {
                        Some(p) if p + 1 < ids.len() => Some(ids[p + 1]),
                        _ => None,
                    }
                }
                None => Some(ids[0]),
            };
            let target = next_id.unwrap_or(-1);
            let _ = player.set_sid(target);
            self.current_sub_id = next_id;
            self.sub_visible = next_id.is_some();
            let _ = player.set_sub_visibility(self.sub_visible);
        }
    }

    fn auto_load_subtitles(&mut self, video_path: &Path) {
        let Some(dir) = video_path.parent() else { return };
        let Some(stem) = video_path.file_stem() else { return };
        let stem = stem.to_string_lossy();
        let Ok(entries) = std::fs::read_dir(dir) else { return };

        let mut found = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() || !is_subtitle_file(&path) {
                continue;
            }
            if let Some(name) = path.file_stem() {
                let name = name.to_string_lossy();
                if name.starts_with(&*stem) {
                    found.push(path);
                }
            }
        }
        found.sort();

        #[cfg(windows)]
        if let Some(player) = self.embedded_player.as_mut() {
            for path in found {
                let _ = player.sub_add(&path, "auto");
            }
        }
    }

    fn open_subtitle_dialog(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let paths = rfd::FileDialog::new()
                .set_title("Open Subtitle Files")
                .add_filter("Subtitles", &["srt", "ass", "ssa", "vtt", "sub", "idx", "smi"])
                .add_filter("All Files", &["*"])
                .pick_files();

            if let Some(paths) = paths {
                let _ = this.update(cx, |this, _cx| {
                    #[cfg(windows)]
                    if let Some(player) = this.embedded_player.as_mut() {
                        for path in paths {
                            let _ = player.sub_add(&path, "select");
                        }
                    }
                });
            }
        })
        .detach();
    }

    fn open_file_dialog(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let paths = rfd::FileDialog::new()
                .set_title("Open Media Files")
                .add_filter("Video", &["mp4", "mkv", "avi", "mov", "wmv", "flv", "webm"])
                .add_filter("Audio", &["mp3", "wav", "flac", "aac", "ogg", "m4a", "wma"])
                .add_filter(
                    "All Media",
                    &[
                        "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "mp3", "wav",
                        "flac", "aac", "ogg", "m4a", "wma",
                    ],
                )
                .pick_files();

            if let Some(paths) = paths {
                let _ = this.update(cx, |this, _cx| {
                    let start_index = this.playlist.len();
                    for path in paths {
                        this.playlist.add(path.clone());
                        this.config.add_recent(path);
                    }
                    if !this.playlist.is_empty() {
                        this.playlist.select(start_index);
                        this.pending_load = true;
                    }
                    this.save_config();
                });
            }
        })
        .detach();
    }

    fn open_folder_dialog(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let folder = rfd::FileDialog::new()
                .set_title("Open Media Folder")
                .pick_folder();

            if let Some(folder) = folder {
                let mut media_files = Vec::new();
                if let Ok(entries) = std::fs::read_dir(&folder) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file() && is_media_file(&path) {
                            media_files.push(path);
                        }
                    }
                }
                media_files.sort();

                let _ = this.update(cx, |this, _cx| {
                    let start_index = this.playlist.len();
                    for path in media_files {
                        this.playlist.add(path.clone());
                        this.config.add_recent(path);
                    }
                    if !this.playlist.is_empty() {
                        this.playlist.select(start_index);
                        this.pending_load = true;
                    }
                    this.save_config();
                });
            }
        })
        .detach();
    }

    fn seek_relative(&mut self, seconds: f64) {
        let Some(current) = self.current_position else { return };
        let target = current.saturating_add(Duration::from_secs_f64(seconds));
        let target = self.total_duration.map(|t| target.min(t)).unwrap_or(target);

        match self.media_type {
            Some(MediaType::Video) => {
                #[cfg(windows)]
                if let Some(player) = self.embedded_player.as_mut() {
                    let _ = player.seek_to(target);
                }
            }
            Some(MediaType::Audio) => {
                if let Some(player) = self.audio_player.as_ref() {
                    player.seek_to(target);
                }
            }
            None => {}
        }
        self.current_position = Some(target);
    }

    fn format_time(duration: Option<Duration>) -> String {
        let Some(d) = duration else {
            return "--:--".to_string();
        };
        let secs = d.as_secs();
        let mins = secs / 60;
        let secs = secs % 60;
        format!("{:02}:{:02}", mins, secs)
    }
}

impl Focusable for PlayerPage {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for PlayerPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Auto-load current playlist item on first render
        if !self.load_attempted {
            self.load_attempted = true;
            if let Some(path) = self.playlist.current().cloned() {
                let _ = self.load_media(&path, window, cx);
            }
        }

        // Handle pending load from file dialog
        if self.pending_load {
            self.pending_load = false;
            if let Some(path) = self.playlist.current().cloned() {
                let _ = self.load_media(&path, window, cx);
            }
        }

        // Auto-advance to next track when current finishes
        if self.pending_next {
            self.pending_next = false;
            self.play_next(window, cx);
        }

        #[cfg(windows)]
        {
            self.update_video_surface_bounds(window);
        }

        // Sync seek slider when not dragging
        if !self.seek_dragging {
            let fraction =
                if let (Some(pos), Some(total)) = (self.current_position, self.total_duration) {
                    if total.as_secs_f64() > 0.0 {
                        (pos.as_secs_f64() / total.as_secs_f64()).clamp(0.0, 1.0) as f32
                    } else {
                        0.0
                    }
                } else {
                    0.0
                };
            let current = self.seek_slider.read(cx).value().start();
            if (current - fraction).abs() > 0.001 {
                self.seek_slider.update(cx, |slider, cx| {
                    slider.set_value(fraction, window, cx);
                });
            }
        }

        let file_name = self
            .playlist
            .current()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| {
                if self.playlist.is_empty() {
                    "Cyber Media Player".to_string()
                } else {
                    format!("{} items", self.playlist.len())
                }
            });

        let time_str = format!(
            "{} / {}",
            Self::format_time(self.current_position),
            Self::format_time(self.total_duration)
        );

        let play_label = if self.is_paused {
            "Resume"
        } else if self.is_playing {
            "Pause"
        } else {
            "Play"
        };

        let mute_label = if self.muted { "Unmute" } else { "Mute" };

        let loop_label = match self.loop_mode {
            LoopMode::None => "Loop",
            LoopMode::All => "LoopAll",
            LoopMode::Single => "Loop1",
        };

        let shuffle_label = if self.shuffle { "Shuffle" } else { "Order" };
        let speed_label = format!("{:.1}x", self.speed);

        let _entity = cx.entity().clone();

        let video_area = match self.media_type {
            Some(MediaType::Video) => div()
                .id("video-host")
                .size_full()
                .bg(gpui::rgba(0x0d121aff))
                .on_prepaint({
                    let entity = _entity.clone();
                    move |bounds, _window, cx| {
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
                    }
                }),
            _ => {
                let audio_name = self
                    .playlist
                    .current()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "No file".to_string());
                let max_bar_height = self.spectrum_max_height;
                let bar_width = 3.0;
                let gap = 1.0;
                let spectrum_bars: Vec<_> = self
                    .visualizer_spectrum
                    .iter()
                    .enumerate()
                    .map(|(i, &energy)| {
                        let height = energy * max_bar_height;
                        div()
                            .id(("spectrum-bar", i))
                            .w(px(bar_width))
                            .h(px(height.max(4.0)))
                            .rounded(px(2.0))
                            .bg(gpui::hsla(0.0, 0.0, 1.0, 0.4f32 + energy * 0.6))
                            .into_any_element()
                    })
                    .collect();
                div()
                    .id("audio-host")
                    .size_full()
                    .bg(gpui::rgba(0x0d121aff))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_end()
                    .px(px(16.))
                    .pb(px(16.))
                    .on_prepaint({
                        let entity = _entity.clone();
                        move |bounds, _window, cx| {
                            let _ = entity.update(cx, |this, _cx| {
                                let height: f32 = bounds.size.height.into();
                                this.spectrum_max_height = (height * 0.85).max(100.0);
                            });
                        }
                    })
                    .child(
                        div()
                            .flex_1()
                            .w_full()
                            .flex()
                            .items_end()
                            .justify_center()
                            .overflow_hidden()
                            .child(
                                h_flex()
                                    .items_end()
                                    .gap(px(gap))
                                    .children(spectrum_bars),
                            ),
                    )
                    .child(
                        div()
                            .pt(px(8.))
                            .text_color(cx.theme().foreground)
                            .child(audio_name),
                    )
            }
        };

        let playlist_items: Vec<_> = self
            .playlist
            .items()
            .iter()
            .enumerate()
            .map(|(index, path)| {
                let item_name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let is_active = self.playlist.current_index() == Some(index);
                div()
                    .id(("playlist-item", index))
                    .px(px(12.))
                    .py(px(8.))
                    .cursor_pointer()
                    .when(is_active, |item| item.bg(cx.theme().primary.opacity(0.15)))
                    .hover(|item| item.bg(cx.theme().primary.opacity(0.1)))
                    .child(item_name)
                    .on_click(cx.listener(move |this, _: &gpui::ClickEvent, window, cx| {
                        this.play_item(index, window, cx);
                    }))
                    .into_any_element()
            })
            .collect();

        let playlist_sidebar = {
            let base = v_flex()
                .w(px(280.))
                .h_full()
                .border_r_1()
                .border_color(cx.theme().border)
                .child(
                    div()
                        .h(px(40.))
                        .px(px(12.))
                        .flex()
                        .items_center()
                        .child("Playlist"),
                );
            if self.playlist.is_empty() {
                base.child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .px(px(16.))
                        .child(
                            div()
                                .text_color(cx.theme().muted_foreground)
                                .child("No media files"),
                        )
                        .child(
                            div()
                                .text_color(cx.theme().muted_foreground)
                                .text_sm()
                                .child("Click Open File to add"),
                        ),
                )
            } else {
                base.children(playlist_items)
            }
        };

        v_flex()
            .id("player-page")
            .size_full()
            .on_drop(cx.listener(|this, paths: &gpui::ExternalPaths, window, cx| {
                let start_index = this.playlist.len();
                let mut added = false;
                for path in paths.paths() {
                    if is_media_file(path) {
                        this.playlist.add(path.to_path_buf());
                        this.config.add_recent(path.to_path_buf());
                        added = true;
                    }
                }
                if added {
                    this.save_config();
                    if let Some(path) = this.playlist.items().get(start_index).cloned() {
                        let _ = this.playlist.select(start_index);
                        let _ = this.load_media(&path, window, cx);
                    }
                }
            }))
            .child(
                TitleBar::new()
                    .child(
                        h_flex()
                            .id("title-bar-inner")
                            .h_full()
                            .w_full()
                            .items_center()
                            .px(px(16.))
                            .child(div().flex_1().child(file_name)),
                    )
                    .trailing_before_controls(
                        h_flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                Button::new("open-file-btn")
                                    .label("Open File")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_file_dialog(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("open-folder-btn")
                                    .label("Open Folder")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_folder_dialog(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("load-sub-btn")
                                    .label("Load Sub")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_subtitle_dialog(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("sub-track-btn")
                                    .label({
                                        let has_subs = !self.sub_tracks.is_empty();
                                        let active = self.current_sub_id.is_some() && self.sub_visible;
                                        if !has_subs {
                                            "CC".into()
                                        } else if active {
                                            let label = self.current_sub_id.and_then(|id| {
                                                self.sub_tracks.iter().find(|t| t.id == id)
                                                    .and_then(|t| t.lang.clone())
                                                    .or_else(|| Some(format!("{}", id)))
                                            });
                                            label.map(|l| format!("CC {}", l)).unwrap_or_else(|| "CC".into())
                                        } else {
                                            "CC Off".into()
                                        }
                                    })
                                    .on_click(cx.listener(|this, _, _window, _cx| {
                                        this.cycle_sub_track();
                                    })),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_h(px(200.))
                    .child(playlist_sidebar)
                    .child(div().flex_1().h_full().child(video_area)),
            )
            .child(
                v_flex()
                    .px(px(16.))
                    .py(px(8.))
                    .gap(px(8.))
                    .child(
                        h_flex()
                            .items_center()
                            .gap(px(8.))
                            .child(time_str)
                            .child(
                                Slider::new(&self.seek_slider)
                                    .disabled(self.total_duration.is_none())
                                    .w_full(),
                            ),
                    )
                    .child(
                        h_flex()
                            .h(px(48.))
                            .items_center()
                            .gap(px(8.))
                            .child(
                                Button::new("prev-btn")
                                    .label("Prev")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.play_prev(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("backward-btn")
                                    .label("-5s")
                                    .on_click(cx.listener(|this, _, _window, _cx| {
                                        this.seek_relative(-5.0);
                                    })),
                            )
                            .child(
                                Button::new("play-btn")
                                    .label(play_label)
                                    .on_click(cx.listener(|this, _, _window, _cx| {
                                        if this.is_playing {
                                            this.toggle_pause();
                                        }
                                    })),
                            )
                            .child(
                                Button::new("stop-btn")
                                    .label("Stop")
                                    .on_click(cx.listener(|this, _, _window, _cx| {
                                        this.stop();
                                    })),
                            )
                            .child(
                                Button::new("forward-btn")
                                    .label("+5s")
                                    .on_click(cx.listener(|this, _, _window, _cx| {
                                        this.seek_relative(5.0);
                                    })),
                            )
                            .child(
                                Button::new("next-btn")
                                    .label("Next")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.play_next(window, cx);
                                    })),
                            )
                            .child(div().w(px(8.)))
                            .child(
                                Button::new("loop-btn")
                                    .label(loop_label)
                                    .on_click(cx.listener(|this, _, _window, _cx| {
                                        this.toggle_loop_mode();
                                    })),
                            )
                            .child(
                                Button::new("shuffle-btn")
                                    .label(shuffle_label)
                                    .on_click(cx.listener(|this, _, _window, _cx| {
                                        this.toggle_shuffle();
                                    })),
                            )
                            .child(
                                Button::new("speed-btn")
                                    .label(speed_label)
                                    .on_click(cx.listener(|this, _, _window, _cx| {
                                        this.toggle_speed();
                                    })),
                            )
                            .child(
                                Button::new("fullscreen-btn")
                                    .label("Full")
                                    .on_click(cx.listener(|_this, _, window, _cx| {
                                        window.toggle_fullscreen();
                                    })),
                            )
                            .child(div().w(px(8.)))
                            .child(
                                Button::new("mute-btn")
                                    .label(mute_label)
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.toggle_mute(cx);
                                    })),
                            )
                            .child(
                                Slider::new(&self.volume_slider)
                                    .disabled(false)
                                    .w(px(100.)),
                            )
                            .child(div().flex_1()),
                    ),
            )
    }
}

pub fn open_main_window<F, E>(title: impl Into<SharedString>, crate_view_fn: F, cx: &mut App)
where
    E: Into<gpui::AnyView>,
    F: FnOnce(&mut Window, &mut App) -> E + Send + 'static,
{
    let window_size = size(px(1280.), px(720.));
    let window_bounds = Bounds::centered(None, window_size, cx);
    let title = title.into();

    cx.spawn(async move |cx| {
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(window_bounds)),
            titlebar: Some(TitleBar::title_bar_options()),
            window_min_size: Some(Size {
                width: px(480.),
                height: px(320.),
            }),
            kind: WindowKind::Normal,
            #[cfg(target_os = "linux")]
            window_background: gpui::WindowBackgroundAppearance::Transparent,
            #[cfg(target_os = "linux")]
            window_decorations: Some(gpui::WindowDecorations::Client),
            ..Default::default()
        };

        let window = cx
            .open_window(options, |window, cx| {
                let view = crate_view_fn(window, cx);
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("failed to open window");

        window.update(cx, |_, window, _| {
            window.activate_window();
            window.set_window_title(&title);
        })?;

        Ok::<_, anyhow::Error>(())
    })
    .detach();
}
