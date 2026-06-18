use std::ffi::{c_char, c_double, c_int, c_void, CStr, CString};
use std::fs;
use std::path::Path;
use std::ptr;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use image::{codecs::png::PngEncoder, ColorType, ImageEncoder};
use libloading::{Library, Symbol};
use tracing::{info, warn};

const MPV_RENDER_PARAM_INVALID: c_int = 0;
const MPV_RENDER_PARAM_API_TYPE: c_int = 1;
const MPV_RENDER_PARAM_BLOCK_FOR_TARGET_TIME: c_int = 12;
const MPV_RENDER_PARAM_SW_SIZE: c_int = 17;
const MPV_RENDER_PARAM_SW_FORMAT: c_int = 18;
const MPV_RENDER_PARAM_SW_STRIDE: c_int = 19;
const MPV_RENDER_PARAM_SW_POINTER: c_int = 20;

const MPV_EVENT_NONE: c_int = 0;
const MPV_EVENT_END_FILE: c_int = 7;
const MPV_EVENT_FILE_LOADED: c_int = 8;
const MPV_EVENT_SHUTDOWN: c_int = 1;

const MPV_RENDER_UPDATE_FRAME: u64 = 1;
const MPV_FORMAT_INT64: c_int = 4;
const MPV_FORMAT_DOUBLE: c_int = 5;

const MPV_RENDER_API_TYPE_SW: &[u8] = b"sw\0";
const MPV_SW_FORMAT_RGB0: &[u8] = b"rgb0\0";

#[repr(C)]
struct mpv_handle {
    _private: [u8; 0],
}

#[repr(C)]
struct mpv_render_context {
    _private: [u8; 0],
}

#[repr(C)]
struct mpv_render_param {
    type_: c_int,
    data: *mut c_void,
}

#[repr(C)]
struct mpv_event {
    event_id: c_int,
    error: c_int,
    reply_userdata: u64,
    data: *mut c_void,
}

type MpvCreate = unsafe extern "C" fn() -> *mut mpv_handle;
type MpvInitialize = unsafe extern "C" fn(*mut mpv_handle) -> c_int;
type MpvTerminateDestroy = unsafe extern "C" fn(*mut mpv_handle);
type MpvSetOptionString =
    unsafe extern "C" fn(*mut mpv_handle, *const c_char, *const c_char) -> c_int;
type MpvCommand = unsafe extern "C" fn(*mut mpv_handle, *const *const c_char) -> c_int;
type MpvWaitEvent = unsafe extern "C" fn(*mut mpv_handle, c_double) -> *mut mpv_event;
type MpvErrorString = unsafe extern "C" fn(c_int) -> *const c_char;
type MpvGetProperty =
    unsafe extern "C" fn(*mut mpv_handle, *const c_char, c_int, *mut c_void) -> c_int;
type MpvGetPropertyString = unsafe extern "C" fn(*mut mpv_handle, *const c_char) -> *mut c_char;
type MpvFree = unsafe extern "C" fn(*mut c_void);
type MpvRenderContextCreate = unsafe extern "C" fn(
    *mut *mut mpv_render_context,
    *mut mpv_handle,
    *mut mpv_render_param,
) -> c_int;
type MpvRenderContextUpdate = unsafe extern "C" fn(*mut mpv_render_context) -> u64;
type MpvRenderContextRender =
    unsafe extern "C" fn(*mut mpv_render_context, *mut mpv_render_param) -> c_int;
type MpvRenderContextFree = unsafe extern "C" fn(*mut mpv_render_context);

struct MpvApi {
    _library: Library,
    mpv_create: MpvCreate,
    mpv_initialize: MpvInitialize,
    mpv_terminate_destroy: MpvTerminateDestroy,
    mpv_set_option_string: MpvSetOptionString,
    mpv_command: MpvCommand,
    mpv_wait_event: MpvWaitEvent,
    mpv_error_string: MpvErrorString,
    mpv_get_property: MpvGetProperty,
    mpv_get_property_string: MpvGetPropertyString,
    mpv_free: MpvFree,
    mpv_render_context_create: MpvRenderContextCreate,
    mpv_render_context_update: MpvRenderContextUpdate,
    mpv_render_context_render: MpvRenderContextRender,
    mpv_render_context_free: MpvRenderContextFree,
}

impl MpvApi {
    fn load() -> Result<Arc<Self>> {
        unsafe {
            let library = Library::new("libmpv-2.dll")
                .or_else(|_| Library::new("third_party/mpv-dev/libmpv-2.dll"))
                .context("load libmpv-2.dll")?;

            let api = Self {
                mpv_create: *load_symbol(&library, b"mpv_create\0")?,
                mpv_initialize: *load_symbol(&library, b"mpv_initialize\0")?,
                mpv_terminate_destroy: *load_symbol(&library, b"mpv_terminate_destroy\0")?,
                mpv_set_option_string: *load_symbol(&library, b"mpv_set_option_string\0")?,
                mpv_command: *load_symbol(&library, b"mpv_command\0")?,
                mpv_wait_event: *load_symbol(&library, b"mpv_wait_event\0")?,
                mpv_error_string: *load_symbol(&library, b"mpv_error_string\0")?,
                mpv_get_property: *load_symbol(&library, b"mpv_get_property\0")?,
                mpv_get_property_string: *load_symbol(&library, b"mpv_get_property_string\0")?,
                mpv_free: *load_symbol(&library, b"mpv_free\0")?,
                mpv_render_context_create: *load_symbol(&library, b"mpv_render_context_create\0")?,
                mpv_render_context_update: *load_symbol(&library, b"mpv_render_context_update\0")?,
                mpv_render_context_render: *load_symbol(&library, b"mpv_render_context_render\0")?,
                mpv_render_context_free: *load_symbol(&library, b"mpv_render_context_free\0")?,
                _library: library,
            };

            Ok(Arc::new(api))
        }
    }

    fn error_text(&self, status: c_int) -> String {
        unsafe {
            let text = (self.mpv_error_string)(status);
            if text.is_null() {
                format!("mpv error {status}")
            } else {
                CStr::from_ptr(text).to_string_lossy().into_owned()
            }
        }
    }

    fn status_to_result(&self, status: c_int, action: &str) -> Result<()> {
        if status >= 0 {
            Ok(())
        } else {
            Err(anyhow!("{action}: {}", self.error_text(status)))
        }
    }
}

unsafe fn load_symbol<'a, T: Copy>(library: &'a Library, name: &[u8]) -> Result<Symbol<'a, T>> {
    library
        .get::<T>(name)
        .with_context(|| format!("load symbol {}", String::from_utf8_lossy(name)))
}

#[derive(Debug, Clone)]
pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    pub rgb0_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Default)]
pub struct MpvMediaInfo {
    pub duration: Option<Duration>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub audio_codec: Option<String>,
    pub video_codec: Option<String>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub frame_rate_milli: Option<u32>,
    pub bitrate_kbps: Option<u32>,
    pub file_size: Option<u64>,
    pub has_audio: bool,
}

#[derive(Debug, Clone)]
pub struct SubtitleTrack {
    pub id: i64,
    pub title: Option<String>,
    pub lang: Option<String>,
    pub selected: bool,
    pub external: bool,
    pub external_filename: Option<String>,
}

pub struct MpvPlayer {
    api: Arc<MpvApi>,
    handle: *mut mpv_handle,
    render_context: *mut mpv_render_context,
    file_loaded: bool,
    ended: bool,
}

pub struct MpvEmbedPlayer {
    api: Arc<MpvApi>,
    handle: *mut mpv_handle,
    ended: bool,
}

pub struct MpvAudioPlayer {
    api: Arc<MpvApi>,
    handle: *mut mpv_handle,
    ended: bool,
}

impl MpvPlayer {
    pub fn new() -> Result<Self> {
        let api = MpvApi::load()?;
        let handle = unsafe { (api.mpv_create)() };
        if handle.is_null() {
            bail!("mpv_create returned null");
        }

        let mut player = Self {
            api,
            handle,
            render_context: ptr::null_mut(),
            file_loaded: false,
            ended: false,
        };

        player.set_option("terminal", "no")?;
        player.set_option("msg-level", "all=warn")?;
        player.set_option("vo", "libmpv")?;
        player.set_option("hwdec", "auto")?;
        player.set_option("hwdec-codecs", "all")?;
        player.set_option("keep-open", "yes")?;
        player.set_option("idle", "yes")?;
        player.set_option("audio-display", "no")?;
        player.set_option("osc", "no")?;
        player.set_option("input-default-bindings", "no")?;
        player.set_option("input-vo-keyboard", "no")?;

        let init_status = unsafe { (player.api.mpv_initialize)(player.handle) };
        player
            .api
            .status_to_result(init_status, "initialize libmpv")?;
        player.create_render_context()?;
        Ok(player)
    }

    pub fn load_file(&mut self, path: &Path) -> Result<()> {
        let path = path
            .to_str()
            .ok_or_else(|| anyhow!("path is not valid UTF-8: {}", path.display()))?;
        let status = self.command(&["loadfile", path, "replace"])?;
        self.api.status_to_result(status, "load file with libmpv")?;
        self.file_loaded = false;
        self.ended = false;
        self.drain_events();
        Ok(())
    }

    pub fn set_pause(&mut self, paused: bool) -> Result<()> {
        let value = if paused { "yes" } else { "no" };
        let status = self.command(&["set", "pause", value])?;
        self.api.status_to_result(status, "set pause")?;
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        let status = self.command(&["stop"])?;
        self.api.status_to_result(status, "stop playback")?;
        self.file_loaded = false;
        self.ended = true;
        Ok(())
    }

    pub fn ended(&self) -> bool {
        self.ended
    }

    pub fn time_pos(&self) -> Result<Option<Duration>> {
        get_property_double(&self.api, self.handle, "time-pos").map(|value| {
            value
                .filter(|secs| *secs >= 0.0)
                .map(Duration::from_secs_f64)
        })
    }

    pub fn render_frame(&mut self, width: u32, height: u32) -> Result<Option<VideoFrame>> {
        if width == 0 || height == 0 {
            return Ok(None);
        }

        self.drain_events();
        if self.ended || self.render_context.is_null() {
            return Ok(None);
        }

        let flags = unsafe { (self.api.mpv_render_context_update)(self.render_context) };
        if flags & MPV_RENDER_UPDATE_FRAME == 0 && !self.file_loaded {
            return Ok(None);
        }
        if flags & MPV_RENDER_UPDATE_FRAME == 0 && !self.has_pending_frame() {
            return Ok(None);
        }

        let stride = width as usize * 4;
        let mut bytes = vec![0_u8; stride * height as usize];
        let mut sw_size = [width as c_int, height as c_int];
        let mut block_for_target_time = 0_i32;
        let mut sw_stride = stride;

        let mut params = [
            mpv_render_param {
                type_: MPV_RENDER_PARAM_SW_SIZE,
                data: sw_size.as_mut_ptr().cast(),
            },
            mpv_render_param {
                type_: MPV_RENDER_PARAM_SW_FORMAT,
                data: MPV_SW_FORMAT_RGB0.as_ptr().cast_mut().cast(),
            },
            mpv_render_param {
                type_: MPV_RENDER_PARAM_SW_STRIDE,
                data: (&mut sw_stride as *mut usize).cast(),
            },
            mpv_render_param {
                type_: MPV_RENDER_PARAM_SW_POINTER,
                data: bytes.as_mut_ptr().cast(),
            },
            mpv_render_param {
                type_: MPV_RENDER_PARAM_BLOCK_FOR_TARGET_TIME,
                data: (&mut block_for_target_time as *mut i32).cast(),
            },
            mpv_render_param {
                type_: MPV_RENDER_PARAM_INVALID,
                data: ptr::null_mut(),
            },
        ];

        let status = unsafe {
            (self.api.mpv_render_context_render)(self.render_context, params.as_mut_ptr())
        };
        self.api.status_to_result(status, "render frame")?;
        Ok(Some(VideoFrame {
            width,
            height,
            rgb0_bytes: bytes,
        }))
    }

    pub fn wait_until_loaded(&mut self, timeout: Duration) -> Result<()> {
        let deadline = Instant::now() + timeout;
        while !self.file_loaded {
            if Instant::now() >= deadline {
                bail!("timed out waiting for mpv file load");
            }
            self.drain_events();
            thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    }

    fn has_pending_frame(&self) -> bool {
        self.file_loaded && !self.ended
    }

    fn set_option(&mut self, name: &str, value: &str) -> Result<()> {
        let name =
            CString::new(name).with_context(|| format!("build C string for option {name}"))?;
        let value = CString::new(value)
            .with_context(|| format!("build C string value for option {name:?}"))?;
        let status =
            unsafe { (self.api.mpv_set_option_string)(self.handle, name.as_ptr(), value.as_ptr()) };
        self.api.status_to_result(
            status,
            &format!("set libmpv option {}", name.to_string_lossy()),
        )
    }

    fn create_render_context(&mut self) -> Result<()> {
        let mut render_context = ptr::null_mut();
        let mut params = [
            mpv_render_param {
                type_: MPV_RENDER_PARAM_API_TYPE,
                data: MPV_RENDER_API_TYPE_SW.as_ptr().cast_mut().cast(),
            },
            mpv_render_param {
                type_: MPV_RENDER_PARAM_INVALID,
                data: ptr::null_mut(),
            },
        ];

        let status = unsafe {
            (self.api.mpv_render_context_create)(
                &mut render_context,
                self.handle,
                params.as_mut_ptr(),
            )
        };
        self.api
            .status_to_result(status, "create libmpv software render context")?;
        self.render_context = render_context;
        Ok(())
    }

    fn command(&self, items: &[&str]) -> Result<c_int> {
        let owned = items
            .iter()
            .map(|item| {
                CString::new(*item).with_context(|| format!("build command argument {item:?}"))
            })
            .collect::<Result<Vec<_>>>()?;
        let mut raw = owned.iter().map(|item| item.as_ptr()).collect::<Vec<_>>();
        raw.push(ptr::null());
        Ok(unsafe { (self.api.mpv_command)(self.handle, raw.as_ptr()) })
    }

    fn drain_events(&mut self) {
        loop {
            let event = unsafe { (self.api.mpv_wait_event)(self.handle, 0.0) };
            if event.is_null() {
                break;
            }
            let event = unsafe { &*event };
            match event.event_id {
                MPV_EVENT_NONE => break,
                MPV_EVENT_FILE_LOADED => {
                    self.file_loaded = true;
                    self.ended = false;
                }
                MPV_EVENT_END_FILE => {
                    self.ended = true;
                }
                _ => {}
            }
        }
    }
}

impl MpvEmbedPlayer {
    pub fn new(target_wid: isize) -> Result<Self> {
        let api = MpvApi::load()?;
        let handle = unsafe { (api.mpv_create)() };
        if handle.is_null() {
            bail!("mpv_create returned null");
        }

        let mut player = Self {
            api,
            handle,
            ended: false,
        };

        player.set_option("terminal", "no")?;
        player.set_option("msg-level", "all=warn")?;
        player.set_option("vo", "gpu")?;
        player.set_option("gpu-context", "d3d11")?;
        player.set_option("hwdec", "d3d11va")?;
        player.set_option("hwdec-codecs", "all")?;
        player.set_option("vd-lavc-dr", "yes")?;
        player.set_option("keep-open", "yes")?;
        player.set_option("idle", "yes")?;
        player.set_option("force-window", "immediate")?;
        player.set_option("osc", "no")?;
        player.set_option("input-default-bindings", "no")?;
        player.set_option("input-vo-keyboard", "no")?;
        player.set_option("wid", &target_wid.to_string())?;

        let init_status = unsafe { (player.api.mpv_initialize)(player.handle) };
        player
            .api
            .status_to_result(init_status, "initialize embedded libmpv")?;
        info!(target_wid, "initialized embedded libmpv player");
        Ok(player)
    }

    pub fn load_file(&mut self, path: &Path) -> Result<()> {
        let path = path
            .to_str()
            .ok_or_else(|| anyhow!("path is not valid UTF-8: {}", path.display()))?;
        let status = self.command(&["loadfile", path, "replace"])?;
        self.api
            .status_to_result(status, "load file with embedded libmpv")?;
        info!(path, "submitted embedded libmpv loadfile command");
        self.ended = false;
        self.drain_events();
        Ok(())
    }

    pub fn set_pause(&mut self, paused: bool) -> Result<()> {
        let value = if paused { "yes" } else { "no" };
        let status = self.command(&["set", "pause", value])?;
        self.api.status_to_result(status, "set pause")?;
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        let status = self.command(&["stop"])?;
        self.api.status_to_result(status, "stop playback")?;
        self.ended = true;
        Ok(())
    }

    pub fn ended(&self) -> bool {
        self.ended
    }

    pub fn poll_events(&mut self) {
        self.drain_events();
    }

    pub fn time_pos(&self) -> Result<Option<Duration>> {
        self.get_property_double("time-pos").map(|value| {
            value
                .filter(|secs| *secs >= 0.0)
                .map(Duration::from_secs_f64)
        })
    }

    pub fn seek_to(&mut self, position: Duration) -> Result<()> {
        let seconds = format!("{:.3}", position.as_secs_f64().max(0.0));
        let status = self.command(&["seek", &seconds, "absolute"])?;
        self.api.status_to_result(status, "seek absolute")?;
        Ok(())
    }

    pub fn seek_relative(&mut self, seconds: f64) -> Result<()> {
        let seconds = format!("{seconds:.3}");
        let status = self.command(&["seek", &seconds, "relative"])?;
        self.api.status_to_result(status, "seek relative")?;
        Ok(())
    }

    pub fn set_volume(&mut self, volume: f64) -> Result<()> {
        let value = format!("{:.1}", volume.max(0.0));
        let status = self.command(&["set", "volume", &value])?;
        self.api.status_to_result(status, "set volume")?;
        Ok(())
    }

    pub fn set_mute(&mut self, mute: bool) -> Result<()> {
        let value = if mute { "yes" } else { "no" };
        let status = self.command(&["set", "mute", value])?;
        self.api.status_to_result(status, "set mute")?;
        Ok(())
    }

    pub fn set_speed(&mut self, speed: f64) -> Result<()> {
        let value = format!("{:.2}", speed.max(0.1));
        let status = self.command(&["set", "speed", &value])?;
        self.api.status_to_result(status, "set speed")?;
        Ok(())
    }

    fn set_option(&mut self, name: &str, value: &str) -> Result<()> {
        let name =
            CString::new(name).with_context(|| format!("build C string for option {name}"))?;
        let value = CString::new(value)
            .with_context(|| format!("build C string value for option {name:?}"))?;
        let status =
            unsafe { (self.api.mpv_set_option_string)(self.handle, name.as_ptr(), value.as_ptr()) };
        self.api.status_to_result(
            status,
            &format!("set libmpv option {}", name.to_string_lossy()),
        )
    }

    fn command(&self, items: &[&str]) -> Result<c_int> {
        let owned = items
            .iter()
            .map(|item| {
                CString::new(*item).with_context(|| format!("build command argument {item:?}"))
            })
            .collect::<Result<Vec<_>>>()?;
        let mut raw = owned.iter().map(|item| item.as_ptr()).collect::<Vec<_>>();
        raw.push(ptr::null());
        Ok(unsafe { (self.api.mpv_command)(self.handle, raw.as_ptr()) })
    }

    fn get_property_double(&self, name: &str) -> Result<Option<f64>> {
        get_property_double(&self.api, self.handle, name)
    }

    fn drain_events(&mut self) {
        loop {
            let event = unsafe { (self.api.mpv_wait_event)(self.handle, 0.0) };
            if event.is_null() {
                break;
            }
            let event = unsafe { &*event };
            match event.event_id {
                MPV_EVENT_NONE => break,
                MPV_EVENT_FILE_LOADED => {
                    info!("embedded libmpv reported file loaded");
                    self.ended = false;
                }
                MPV_EVENT_END_FILE => {
                    warn!("embedded libmpv reported end file");
                    self.ended = true;
                }
                _ => {}
            }
        }
    }

    pub fn sub_add(&mut self, path: &Path, flag: &str) -> Result<()> {
        let path = path
            .to_str()
            .ok_or_else(|| anyhow!("subtitle path is not valid UTF-8: {}", path.display()))?;
        let status = self.command(&["sub-add", path, flag])?;
        self.api.status_to_result(status, "sub-add")?;
        Ok(())
    }

    pub fn set_sid(&mut self, id: i64) -> Result<()> {
        let value = if id < 0 { "no" } else { &id.to_string() };
        let status = self.command(&["set", "sid", value])?;
        self.api.status_to_result(status, "set sid")?;
        Ok(())
    }

    pub fn set_sub_visibility(&mut self, visible: bool) -> Result<()> {
        let value = if visible { "yes" } else { "no" };
        let status = self.command(&["set", "sub-visibility", value])?;
        self.api.status_to_result(status, "set sub-visibility")?;
        Ok(())
    }

    pub fn sub_visibility(&self) -> Result<bool> {
        match get_property_string(&self.api, self.handle, "sub-visibility")? {
            Some(v) => Ok(v == "yes"),
            None => Ok(true),
        }
    }

    pub fn current_sid(&self) -> Result<Option<i64>> {
        get_property_i64(&self.api, self.handle, "sid")
    }

    pub fn subtitle_tracks(&self) -> Result<Vec<SubtitleTrack>> {
        let count = match get_property_i64(&self.api, self.handle, "track-list/count")? {
            Some(c) if c > 0 => c as usize,
            _ => return Ok(Vec::new()),
        };

        let mut tracks = Vec::new();
        for i in 0..count {
            let track_type =
                get_property_string(&self.api, self.handle, &format!("track-list/{i}/type"))?;
            if track_type.as_deref() != Some("sub") {
                continue;
            }
            let id = get_property_i64(&self.api, self.handle, &format!("track-list/{i}/id"))?
                .unwrap_or(0);
            let title =
                get_property_string(&self.api, self.handle, &format!("track-list/{i}/title"))?;
            let lang =
                get_property_string(&self.api, self.handle, &format!("track-list/{i}/lang"))?;
            let selected =
                get_property_string(&self.api, self.handle, &format!("track-list/{i}/selected"))?
                    .map(|s| s == "yes")
                    .unwrap_or(false);
            let external =
                get_property_string(&self.api, self.handle, &format!("track-list/{i}/external"))?
                    .map(|s| s == "yes")
                    .unwrap_or(false);
            let external_filename = if external {
                get_property_string(
                    &self.api,
                    self.handle,
                    &format!("track-list/{i}/external-filename"),
                )?
            } else {
                None
            };
            tracks.push(SubtitleTrack {
                id,
                title,
                lang,
                selected,
                external,
                external_filename,
            });
        }
        Ok(tracks)
    }
}

impl MpvPlayer {
    pub fn seek_to(&mut self, position: Duration) -> Result<()> {
        let seconds = format!("{:.3}", position.as_secs_f64().max(0.0));
        let status = self.command(&["seek", &seconds, "absolute"])?;
        self.api.status_to_result(status, "seek absolute")?;
        Ok(())
    }
}

impl MpvAudioPlayer {
    pub fn new() -> Result<Self> {
        let api = MpvApi::load()?;
        let handle = unsafe { (api.mpv_create)() };
        if handle.is_null() {
            bail!("mpv_create returned null");
        }

        let mut player = Self {
            api,
            handle,
            ended: false,
        };

        player.set_option("terminal", "no")?;
        player.set_option("msg-level", "all=warn")?;
        player.set_option("vo", "null")?;
        player.set_option("keep-open", "yes")?;
        player.set_option("idle", "yes")?;
        player.set_option("osc", "no")?;
        player.set_option("input-default-bindings", "no")?;
        player.set_option("input-vo-keyboard", "no")?;

        let init_status = unsafe { (player.api.mpv_initialize)(player.handle) };
        player
            .api
            .status_to_result(init_status, "initialize audio libmpv")?;
        Ok(player)
    }

    pub fn load_file(&mut self, path: &Path) -> Result<()> {
        let path = path
            .to_str()
            .ok_or_else(|| anyhow!("path is not valid UTF-8: {}", path.display()))?;
        let status = self.command(&["loadfile", path, "replace"])?;
        self.api
            .status_to_result(status, "load file with audio libmpv")?;
        self.ended = false;
        self.drain_events();
        Ok(())
    }

    pub fn set_pause(&mut self, paused: bool) -> Result<()> {
        let value = if paused { "yes" } else { "no" };
        let status = self.command(&["set", "pause", value])?;
        self.api.status_to_result(status, "set pause")?;
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        let status = self.command(&["stop"])?;
        self.api.status_to_result(status, "stop playback")?;
        self.ended = true;
        Ok(())
    }

    pub fn ended(&self) -> bool {
        self.ended
    }

    pub fn poll_events(&mut self) {
        self.drain_events();
    }

    pub fn time_pos(&self) -> Result<Option<Duration>> {
        self.get_property_double("time-pos").map(|value| {
            value
                .filter(|secs| *secs >= 0.0)
                .map(Duration::from_secs_f64)
        })
    }

    pub fn duration(&self) -> Result<Option<Duration>> {
        self.get_property_double("duration").map(|value| {
            value
                .filter(|secs| *secs > 0.0)
                .map(Duration::from_secs_f64)
        })
    }

    pub fn seek_to(&mut self, position: Duration) -> Result<()> {
        let seconds = format!("{:.3}", position.as_secs_f64().max(0.0));
        let status = self.command(&["seek", &seconds, "absolute"])?;
        self.api.status_to_result(status, "seek absolute")?;
        Ok(())
    }

    pub fn seek_relative(&mut self, seconds: f64) -> Result<()> {
        let seconds = format!("{seconds:.3}");
        let status = self.command(&["seek", &seconds, "relative"])?;
        self.api.status_to_result(status, "seek relative")?;
        Ok(())
    }

    pub fn set_volume(&mut self, volume: f64) -> Result<()> {
        let value = format!("{:.1}", volume.max(0.0));
        let status = self.command(&["set", "volume", &value])?;
        self.api.status_to_result(status, "set volume")?;
        Ok(())
    }

    pub fn set_mute(&mut self, mute: bool) -> Result<()> {
        let value = if mute { "yes" } else { "no" };
        let status = self.command(&["set", "mute", value])?;
        self.api.status_to_result(status, "set mute")?;
        Ok(())
    }

    pub fn set_speed(&mut self, speed: f64) -> Result<()> {
        let value = format!("{:.2}", speed.max(0.1));
        let status = self.command(&["set", "speed", &value])?;
        self.api.status_to_result(status, "set speed")?;
        Ok(())
    }

    fn set_option(&mut self, name: &str, value: &str) -> Result<()> {
        let name =
            CString::new(name).with_context(|| format!("build C string for option {name}"))?;
        let value = CString::new(value)
            .with_context(|| format!("build C string value for option {name:?}"))?;
        let status =
            unsafe { (self.api.mpv_set_option_string)(self.handle, name.as_ptr(), value.as_ptr()) };
        self.api.status_to_result(
            status,
            &format!("set libmpv option {}", name.to_string_lossy()),
        )
    }

    fn command(&self, items: &[&str]) -> Result<c_int> {
        let owned = items
            .iter()
            .map(|item| {
                CString::new(*item).with_context(|| format!("build command argument {item:?}"))
            })
            .collect::<Result<Vec<_>>>()?;
        let mut raw = owned.iter().map(|item| item.as_ptr()).collect::<Vec<_>>();
        raw.push(ptr::null());
        Ok(unsafe { (self.api.mpv_command)(self.handle, raw.as_ptr()) })
    }

    fn get_property_double(&self, name: &str) -> Result<Option<f64>> {
        get_property_double(&self.api, self.handle, name)
    }

    fn drain_events(&mut self) {
        loop {
            let event = unsafe { (self.api.mpv_wait_event)(self.handle, 0.0) };
            if event.is_null() {
                break;
            }
            let event = unsafe { &*event };
            match event.event_id {
                MPV_EVENT_NONE => break,
                MPV_EVENT_FILE_LOADED => {
                    self.ended = false;
                }
                MPV_EVENT_END_FILE => {
                    self.ended = true;
                }
                _ => {}
            }
        }
    }
}

pub fn probe_media(path: &Path) -> Result<MpvMediaInfo> {
    let api = MpvApi::load()?;
    let handle = unsafe { (api.mpv_create)() };
    if handle.is_null() {
        bail!("mpv_create returned null");
    }

    let mut probe = MpvProbeHandle { api, handle };
    probe.set_option("terminal", "no")?;
    probe.set_option("msg-level", "all=warn")?;
    probe.set_option("vo", "null")?;
    probe.set_option("ao", "null")?;
    probe.set_option("idle", "yes")?;
    probe.set_option("pause", "yes")?;

    let init_status = unsafe { (probe.api.mpv_initialize)(probe.handle) };
    probe
        .api
        .status_to_result(init_status, "initialize mpv probe handle")?;

    let path_str = path
        .to_str()
        .ok_or_else(|| anyhow!("path is not valid UTF-8: {}", path.display()))?;
    let status = probe.command(&["loadfile", path_str, "replace"])?;
    probe
        .api
        .status_to_result(status, "load file for mpv probe")?;
    probe.wait_until_loaded()?;

    let duration = get_property_double(&probe.api, probe.handle, "duration")?
        .filter(|secs| *secs > 0.0)
        .map(Duration::from_secs_f64);
    let title = metadata_value(
        &probe.api,
        probe.handle,
        &["metadata/by-key/title", "metadata/by-key/TITLE"],
    )?;
    let artist = metadata_value(
        &probe.api,
        probe.handle,
        &[
            "metadata/by-key/artist",
            "metadata/by-key/ARTIST",
            "metadata/by-key/album_artist",
            "metadata/by-key/ALBUMARTIST",
        ],
    )?;
    let album = metadata_value(
        &probe.api,
        probe.handle,
        &["metadata/by-key/album", "metadata/by-key/ALBUM"],
    )?;
    let video_codec = get_property_string(&probe.api, probe.handle, "video-codec")?;
    let audio_codec = match get_property_string(&probe.api, probe.handle, "audio-codec-name")? {
        Some(codec) => Some(codec),
        None => get_property_string(&probe.api, probe.handle, "audio-codec")?,
    };
    let sample_rate = get_property_i64(&probe.api, probe.handle, "audio-params/samplerate")?
        .filter(|value| *value > 0)
        .map(|value| value as u32);
    let channels = get_property_i64(&probe.api, probe.handle, "audio-params/channel-count")?
        .filter(|value| *value > 0)
        .map(|value| value as u16);
    let width = get_property_i64(&probe.api, probe.handle, "width")?
        .filter(|value| *value > 0)
        .map(|value| value as u32);
    let height = get_property_i64(&probe.api, probe.handle, "height")?
        .filter(|value| *value > 0)
        .map(|value| value as u32);
    let frame_rate_milli = get_property_double(&probe.api, probe.handle, "estimated-vf-fps")?
        .or_else(|| {
            get_property_double(&probe.api, probe.handle, "container-fps")
                .ok()
                .flatten()
        })
        .filter(|fps| *fps > 0.0)
        .map(|fps| (fps * 1000.0).round() as u32);
    let bitrate_kbps = get_property_i64(&probe.api, probe.handle, "video-bitrate")?
        .or_else(|| {
            get_property_i64(&probe.api, probe.handle, "audio-bitrate")
                .ok()
                .flatten()
        })
        .or_else(|| {
            get_property_i64(&probe.api, probe.handle, "video-params/bitrate")
                .ok()
                .flatten()
        })
        .filter(|value| *value > 0)
        .map(|value| (value as u64 / 1000) as u32);
    let file_size = fs::metadata(path).ok().map(|meta| meta.len());

    Ok(MpvMediaInfo {
        duration,
        title,
        artist,
        album,
        audio_codec: audio_codec.clone(),
        video_codec,
        sample_rate,
        channels,
        width,
        height,
        frame_rate_milli,
        bitrate_kbps,
        file_size,
        has_audio: audio_codec.is_some(),
    })
}

pub fn extract_video_poster_png(path: &Path, max_edge: u32) -> Result<Vec<u8>> {
    let info = probe_media(path)?;
    let width = info.width.unwrap_or(640).max(1);
    let height = info.height.unwrap_or(360).max(1);
    let max_edge = max_edge.max(64);
    let scale = (max_edge as f32 / width as f32)
        .min(max_edge as f32 / height as f32)
        .min(1.0);
    let target_width = ((width as f32 * scale).round() as u32).max(1);
    let target_height = ((height as f32 * scale).round() as u32).max(1);

    let mut player = MpvPlayer::new()?;
    player.load_file(path)?;
    player.set_pause(false)?;
    let target = if let Some(duration) = info.duration {
        let target = if duration > Duration::from_secs(8) {
            Duration::from_secs(1)
        } else if duration > Duration::from_secs(2) {
            Duration::from_secs_f64((duration.as_secs_f64() * 0.15).max(0.25))
        } else {
            Duration::from_secs_f64((duration.as_secs_f64() * 0.25).max(0.1))
        };
        target.min(duration)
    } else {
        Duration::from_secs(1)
    };
    player.wait_until_loaded(Duration::from_secs(2))?;
    let _ = player.seek_to(target);

    let deadline = Instant::now() + Duration::from_secs(2);
    let frame = loop {
        if let Some(frame) = player.render_frame(target_width, target_height)? {
            let reached_target = player
                .time_pos()?
                .is_some_and(|position| position + Duration::from_millis(100) >= target);
            if reached_target {
                break frame;
            }
        }
        if Instant::now() >= deadline {
            bail!("timed out rendering video poster");
        }
        thread::sleep(Duration::from_millis(15));
    };

    let mut rgba_bytes = frame.rgb0_bytes;
    for pixel in rgba_bytes.chunks_exact_mut(4) {
        pixel[3] = 0xFF;
    }

    let mut png = Vec::new();
    PngEncoder::new(&mut png).write_image(
        &rgba_bytes,
        frame.width,
        frame.height,
        ColorType::Rgba8.into(),
    )?;
    Ok(png)
}

struct MpvProbeHandle {
    api: Arc<MpvApi>,
    handle: *mut mpv_handle,
}

impl MpvProbeHandle {
    fn set_option(&mut self, name: &str, value: &str) -> Result<()> {
        let name =
            CString::new(name).with_context(|| format!("build C string for option {name}"))?;
        let value = CString::new(value)
            .with_context(|| format!("build C string value for option {name:?}"))?;
        let status =
            unsafe { (self.api.mpv_set_option_string)(self.handle, name.as_ptr(), value.as_ptr()) };
        self.api.status_to_result(
            status,
            &format!("set mpv probe option {}", name.to_string_lossy()),
        )
    }

    fn command(&self, items: &[&str]) -> Result<c_int> {
        let owned = items
            .iter()
            .map(|item| {
                CString::new(*item).with_context(|| format!("build command argument {item:?}"))
            })
            .collect::<Result<Vec<_>>>()?;
        let mut raw = owned.iter().map(|item| item.as_ptr()).collect::<Vec<_>>();
        raw.push(ptr::null());
        Ok(unsafe { (self.api.mpv_command)(self.handle, raw.as_ptr()) })
    }

    fn wait_until_loaded(&mut self) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if Instant::now() >= deadline {
                bail!("timed out waiting for mpv to load media");
            }
            let event = unsafe { (self.api.mpv_wait_event)(self.handle, 0.25) };
            if event.is_null() {
                continue;
            }
            let event = unsafe { &*event };
            match event.event_id {
                MPV_EVENT_FILE_LOADED => return Ok(()),
                MPV_EVENT_END_FILE => return Ok(()),
                MPV_EVENT_SHUTDOWN => bail!("mpv probe shut down before file loaded"),
                MPV_EVENT_NONE => continue,
                _ => {}
            }
        }
    }
}

fn metadata_value(
    api: &Arc<MpvApi>,
    handle: *mut mpv_handle,
    names: &[&str],
) -> Result<Option<String>> {
    for name in names {
        if let Some(value) = get_property_string(api, handle, name)? {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

impl Drop for MpvProbeHandle {
    fn drop(&mut self) {
        unsafe {
            if !self.handle.is_null() {
                (self.api.mpv_terminate_destroy)(self.handle);
                self.handle = ptr::null_mut();
            }
        }
    }
}

fn get_property_i64(api: &Arc<MpvApi>, handle: *mut mpv_handle, name: &str) -> Result<Option<i64>> {
    let property = CString::new(name).with_context(|| format!("build property name {name}"))?;
    let mut value = 0_i64;
    let status = unsafe {
        (api.mpv_get_property)(
            handle,
            property.as_ptr(),
            MPV_FORMAT_INT64,
            (&mut value as *mut i64).cast(),
        )
    };
    if status >= 0 {
        Ok(Some(value))
    } else if api.error_text(status).contains("property unavailable") {
        Ok(None)
    } else {
        Err(anyhow!(
            "read mpv property {name}: {}",
            api.error_text(status)
        ))
    }
}

fn get_property_double(
    api: &Arc<MpvApi>,
    handle: *mut mpv_handle,
    name: &str,
) -> Result<Option<f64>> {
    let property = CString::new(name).with_context(|| format!("build property name {name}"))?;
    let mut value = 0_f64;
    let status = unsafe {
        (api.mpv_get_property)(
            handle,
            property.as_ptr(),
            MPV_FORMAT_DOUBLE,
            (&mut value as *mut f64).cast(),
        )
    };
    if status >= 0 {
        Ok(Some(value))
    } else if api.error_text(status).contains("property unavailable") {
        Ok(None)
    } else {
        Err(anyhow!(
            "read mpv property {name}: {}",
            api.error_text(status)
        ))
    }
}

fn get_property_string(
    api: &Arc<MpvApi>,
    handle: *mut mpv_handle,
    name: &str,
) -> Result<Option<String>> {
    let property = CString::new(name).with_context(|| format!("build property name {name}"))?;
    let value = unsafe { (api.mpv_get_property_string)(handle, property.as_ptr()) };
    if value.is_null() {
        return Ok(None);
    }
    let text = unsafe { CStr::from_ptr(value) }
        .to_string_lossy()
        .trim()
        .to_string();
    unsafe {
        (api.mpv_free)(value.cast());
    }
    if text.is_empty() || text == "null" {
        Ok(None)
    } else {
        Ok(Some(text))
    }
}

impl Drop for MpvPlayer {
    fn drop(&mut self) {
        unsafe {
            if !self.render_context.is_null() {
                (self.api.mpv_render_context_free)(self.render_context);
                self.render_context = ptr::null_mut();
            }
            if !self.handle.is_null() {
                (self.api.mpv_terminate_destroy)(self.handle);
                self.handle = ptr::null_mut();
            }
        }
    }
}

impl Drop for MpvEmbedPlayer {
    fn drop(&mut self) {
        unsafe {
            if !self.handle.is_null() {
                (self.api.mpv_terminate_destroy)(self.handle);
                self.handle = ptr::null_mut();
            }
        }
    }
}

impl Drop for MpvAudioPlayer {
    fn drop(&mut self) {
        unsafe {
            if !self.handle.is_null() {
                (self.api.mpv_terminate_destroy)(self.handle);
                self.handle = ptr::null_mut();
            }
        }
    }
}
