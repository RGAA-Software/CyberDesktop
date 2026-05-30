//! Memory-mapped, streaming-decode file loading.
//!
//! The whole point of this module is fast opening of large files:
//! - the file is `mmap`ped (no eager copy into a `Vec<u8>`),
//! - the encoding is detected from a bounded prefix,
//! - bytes are decoded chunk-by-chunk straight into a [`ropey::RopeBuilder`],
//!   so there is never a single giant intermediate `String`.

use std::fs::File;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result};
use memmap2::Mmap;
use ropey::RopeBuilder;

use crate::buffer::TextBuffer;
use crate::encoding::{self, EncodingInfo, LineEnding};

/// A fully loaded document plus the metadata the UI needs for its status bar.
pub struct LoadedFile {
    pub buffer: TextBuffer,
    pub encoding: EncodingInfo,
    pub line_ending: LineEnding,
    /// True if a NUL byte was seen near the start (likely a binary file).
    pub looks_binary: bool,
    /// True if decoding hit malformed sequences (replaced with U+FFFD).
    pub had_decode_errors: bool,
}

/// Bytes fed to the decoder per iteration.
const DECODE_CHUNK: usize = 256 * 1024;

/// Live byte counter updated while [`load_file_with_progress`] decodes a file.
#[derive(Debug)]
pub struct LoadProgress {
    total_bytes: usize,
    bytes_done: AtomicUsize,
}

impl LoadProgress {
    pub fn new(total_bytes: usize) -> Self {
        Self {
            total_bytes,
            bytes_done: AtomicUsize::new(0),
        }
    }

    /// Fraction complete in `[0.0, 1.0]`.
    pub fn fraction(&self) -> f32 {
        if self.total_bytes == 0 {
            return 1.0;
        }
        (self.bytes_done.load(Ordering::Relaxed) as f32 / self.total_bytes as f32).clamp(0.0, 1.0)
    }
}

/// Loads `path` into a [`TextBuffer`], detecting encoding and line ending.
pub fn load_file(path: &Path) -> Result<LoadedFile> {
    load_file_with_progress(path, None)
}

/// Like [`load_file`], optionally updating `progress` after each decode chunk.
pub fn load_file_with_progress(
    path: &Path,
    progress: Option<&LoadProgress>,
) -> Result<LoadedFile> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let len = file
        .metadata()
        .with_context(|| format!("stat {}", path.display()))?
        .len() as usize;

    if let Some(progress) = progress {
        progress.bytes_done.store(0, Ordering::Relaxed);
    }

    if len == 0 {
        if let Some(progress) = progress {
            progress.bytes_done.store(0, Ordering::Relaxed);
        }
        return Ok(LoadedFile {
            buffer: TextBuffer::new(),
            encoding: EncodingInfo::default(),
            line_ending: LineEnding::default(),
            looks_binary: false,
            had_decode_errors: false,
        });
    }

    // SAFETY: we only ever read from the mapping. The standard mmap caveat
    // applies (external truncation while mapped could fault); acceptable for an
    // editor opening user-selected files.
    let mmap = unsafe { Mmap::map(&file) }.with_context(|| format!("mmap {}", path.display()))?;
    let bytes: &[u8] = &mmap;

    let info = encoding::detect(bytes);
    let looks_binary = encoding::looks_binary(bytes);

    let mut decoder = info.encoding.new_decoder();
    let mut builder = RopeBuilder::new();
    let mut scratch = String::new();
    let mut had_decode_errors = false;
    let mut line_ending: Option<LineEnding> = None;

    let mut offset = 0usize;
    while offset < bytes.len() {
        let end = (offset + DECODE_CHUNK).min(bytes.len());
        let last = end == bytes.len();
        let chunk = &bytes[offset..end];

        // `decode_to_string` writes only into existing spare capacity (it does
        // not grow the string), so reserve the worst-case UTF-8 length first.
        let needed = decoder
            .max_utf8_buffer_length(chunk.len())
            .unwrap_or(chunk.len().saturating_mul(4).max(4));
        scratch.clear();
        scratch.reserve(needed);
        let (_result, _read, errors) = decoder.decode_to_string(chunk, &mut scratch, last);
        had_decode_errors |= errors;

        if line_ending.is_none() && (scratch.contains('\n') || scratch.contains('\r') || last) {
            line_ending = Some(LineEnding::detect(&scratch));
        }

        builder.append(&scratch);
        offset = end;
        if let Some(progress) = progress {
            progress.bytes_done.store(offset, Ordering::Relaxed);
        }
    }

    Ok(LoadedFile {
        buffer: TextBuffer::from_rope(builder.finish()),
        encoding: info,
        line_ending: line_ending.unwrap_or_default(),
        looks_binary,
        had_decode_errors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp(bytes: &[u8]) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(bytes).unwrap();
        f.flush().unwrap();
        f
    }

    #[test]
    fn loads_utf8() {
        let f = write_temp("héllo\nworld\n".as_bytes());
        let loaded = load_file(f.path()).unwrap();
        assert_eq!(loaded.buffer.to_string(), "héllo\nworld\n");
        assert_eq!(loaded.encoding.encoding, encoding_rs::UTF_8);
        assert!(!loaded.had_decode_errors);
    }

    #[test]
    fn detects_line_ending_crlf() {
        let f = write_temp(b"a\r\nb\r\n");
        let loaded = load_file(f.path()).unwrap();
        assert_eq!(loaded.line_ending, LineEnding::Crlf);
    }

    #[test]
    fn decodes_gbk() {
        // "中文" in GBK.
        let gbk = [0xD6, 0xD0, 0xCE, 0xC4];
        let f = write_temp(&gbk);
        let loaded = load_file(f.path()).unwrap();
        assert_eq!(loaded.buffer.to_string(), "中文");
    }

    #[test]
    fn strips_utf8_bom() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice("hi".as_bytes());
        let f = write_temp(&bytes);
        let loaded = load_file(f.path()).unwrap();
        assert_eq!(loaded.buffer.to_string(), "hi");
        assert!(loaded.encoding.had_bom);
    }

    #[test]
    fn empty_file() {
        let f = write_temp(b"");
        let loaded = load_file(f.path()).unwrap();
        assert_eq!(loaded.buffer.len_chars(), 0);
        assert_eq!(loaded.buffer.line_count(), 1);
    }
}
