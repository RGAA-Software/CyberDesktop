//! Encoding detection, decoding and encoding.
//!
//! Detection order:
//! 1. A leading BOM (UTF-8 / UTF-16 LE/BE) wins unconditionally.
//! 2. Otherwise `chardetng` (the detector Firefox ships) guesses from a sample.
//!
//! Decoding is streaming-friendly: the loader feeds chunks into a
//! [`encoding_rs::Decoder`] so we never need the whole file as a single
//! intermediate `String`.

use encoding_rs::Encoding;

/// Detected/selected encoding for a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncodingInfo {
    /// The `encoding_rs` encoding used to decode/encode the bytes.
    pub encoding: &'static Encoding,
    /// Whether the source started with a byte-order mark.
    pub had_bom: bool,
}

impl EncodingInfo {
    /// Human-readable label for the status bar (e.g. `UTF-8`, `GBK`,
    /// `UTF-8 BOM`).
    pub fn label(&self) -> String {
        let name = self.encoding.name();
        if self.had_bom {
            format!("{name} BOM")
        } else {
            name.to_string()
        }
    }
}

impl Default for EncodingInfo {
    fn default() -> Self {
        Self {
            encoding: encoding_rs::UTF_8,
            had_bom: false,
        }
    }
}

/// Line-ending style of a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineEnding {
    /// `\n` (Unix).
    #[default]
    Lf,
    /// `\r\n` (Windows).
    Crlf,
    /// `\r` (classic Mac).
    Cr,
}

impl LineEnding {
    pub fn as_str(&self) -> &'static str {
        match self {
            LineEnding::Lf => "\n",
            LineEnding::Crlf => "\r\n",
            LineEnding::Cr => "\r",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            LineEnding::Lf => "LF",
            LineEnding::Crlf => "CRLF",
            LineEnding::Cr => "CR",
        }
    }

    /// Detects the dominant line ending by inspecting the first occurrence.
    pub fn detect(text: &str) -> LineEnding {
        let bytes = text.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'\r' => {
                    if bytes.get(i + 1) == Some(&b'\n') {
                        return LineEnding::Crlf;
                    }
                    return LineEnding::Cr;
                }
                b'\n' => return LineEnding::Lf,
                _ => {}
            }
            i += 1;
        }
        LineEnding::Lf
    }
}

/// How many bytes to sample for `chardetng` detection. The detector works on a
/// prefix; 64 KiB is plenty and keeps detection O(1) regardless of file size.
const DETECT_SAMPLE_BYTES: usize = 64 * 1024;

/// Detects the encoding of `bytes`.
///
/// `bytes` should be the start of the file (the loader hands us an mmap, so this
/// is the whole file, but we only sample a prefix). A detected BOM is reported
/// via [`EncodingInfo::had_bom`]; the loader is responsible for skipping the BOM
/// bytes before/while decoding (the `encoding_rs` decoder strips it itself when
/// fed from the start).
pub fn detect(bytes: &[u8]) -> EncodingInfo {
    if let Some((encoding, _bom_len)) = Encoding::for_bom(bytes) {
        return EncodingInfo {
            encoding,
            had_bom: true,
        };
    }

    let sample_len = bytes.len().min(DETECT_SAMPLE_BYTES);
    let mut detector = chardetng::EncodingDetector::new();
    let exhausted = sample_len == bytes.len();
    detector.feed(&bytes[..sample_len], exhausted);
    let encoding = detector.guess(None, true);
    EncodingInfo {
        encoding,
        had_bom: false,
    }
}

/// A heuristic "is this binary?" check: a NUL byte in the first 8 KiB.
pub fn looks_binary(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(8 * 1024)];
    head.contains(&0)
}

/// Encodes `text` into `info`'s encoding, re-adding a BOM if the source had one
/// and the encoding defines one.
pub fn encode(text: &str, info: EncodingInfo) -> Vec<u8> {
    let (cow, _, _) = info.encoding.encode(text);
    let mut out = Vec::new();
    if info.had_bom {
        match info.encoding.name() {
            "UTF-8" => out.extend_from_slice(&[0xEF, 0xBB, 0xBF]),
            "UTF-16LE" => out.extend_from_slice(&[0xFF, 0xFE]),
            "UTF-16BE" => out.extend_from_slice(&[0xFE, 0xFF]),
            _ => {}
        }
    }
    out.extend_from_slice(&cow);
    out
}
