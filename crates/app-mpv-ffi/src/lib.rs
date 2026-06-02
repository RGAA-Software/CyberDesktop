use std::ffi::{c_char, c_double, c_int, c_void, CStr, CString};
use std::path::Path;
use std::ptr;
use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};
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

const MPV_RENDER_UPDATE_FRAME: u64 = 1;

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

        let status = unsafe { (self.api.mpv_render_context_render)(self.render_context, params.as_mut_ptr()) };
        self.api.status_to_result(status, "render frame")?;
        Ok(Some(VideoFrame {
            width,
            height,
            rgb0_bytes: bytes,
        }))
    }

    fn has_pending_frame(&self) -> bool {
        self.file_loaded && !self.ended
    }

    fn set_option(&mut self, name: &str, value: &str) -> Result<()> {
        let name = CString::new(name).with_context(|| format!("build C string for option {name}"))?;
        let value = CString::new(value)
            .with_context(|| format!("build C string value for option {name:?}"))?;
        let status = unsafe { (self.api.mpv_set_option_string)(self.handle, name.as_ptr(), value.as_ptr()) };
        self.api
            .status_to_result(status, &format!("set libmpv option {}", name.to_string_lossy()))
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
            (self.api.mpv_render_context_create)(&mut render_context, self.handle, params.as_mut_ptr())
        };
        self.api
            .status_to_result(status, "create libmpv software render context")?;
        self.render_context = render_context;
        Ok(())
    }

    fn command(&self, items: &[&str]) -> Result<c_int> {
        let owned = items
            .iter()
            .map(|item| CString::new(*item).with_context(|| format!("build command argument {item:?}")))
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

    fn set_option(&mut self, name: &str, value: &str) -> Result<()> {
        let name = CString::new(name).with_context(|| format!("build C string for option {name}"))?;
        let value = CString::new(value)
            .with_context(|| format!("build C string value for option {name:?}"))?;
        let status = unsafe { (self.api.mpv_set_option_string)(self.handle, name.as_ptr(), value.as_ptr()) };
        self.api
            .status_to_result(status, &format!("set libmpv option {}", name.to_string_lossy()))
    }

    fn command(&self, items: &[&str]) -> Result<c_int> {
        let owned = items
            .iter()
            .map(|item| CString::new(*item).with_context(|| format!("build command argument {item:?}")))
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
