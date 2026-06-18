//! Raw FFI to the in-process 7-Zip extract wrapper.

#![allow(non_camel_case_types, dead_code)]

use std::os::raw::c_void;

pub const SEVENZIP_FORMAT_AUTO: i32 = 0;
pub const SEVENZIP_FORMAT_ZIP: i32 = 0x01;
pub const SEVENZIP_FORMAT_BZIP2: i32 = 0x02;
pub const SEVENZIP_FORMAT_7Z: i32 = 0x07;
pub const SEVENZIP_FORMAT_XZ: i32 = 0x0C;
pub const SEVENZIP_FORMAT_TAR: i32 = 0xEE;
pub const SEVENZIP_FORMAT_GZIP: i32 = 0xEF;

pub type SevenZipProgressFn = Option<extern "C" fn(ctx: *mut c_void, completed: u32, total: u32)>;
pub type SevenZipCancelFn = Option<extern "C" fn(ctx: *mut c_void) -> i32>;

extern "C" {
    pub fn sevenzip_extract(
        dll_path: *const u16,
        archive_path: *const u16,
        dest_dir: *const u16,
        format_hint: i32,
        thread_count: u32,
        progress: SevenZipProgressFn,
        progress_ctx: *mut c_void,
        cancel: SevenZipCancelFn,
        cancel_ctx: *mut c_void,
        error_buf: *mut u16,
        error_buf_len: usize,
    ) -> i32;
}

pub mod sys {
    pub use super::*;
}
