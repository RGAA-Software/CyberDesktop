use std::path::Path;

const MAX_TEXT_PREVIEW_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewKind {
    Image,
    Svg,
    Markdown,
    Html,
    Code,
    Text,
    Pdf,
    Audio,
    Video,
}

const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "wav", "flac", "ogg", "oga", "m4a", "aac", "wma", "opus", "aiff", "aif",
];

const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "avi", "mov", "wmv", "webm", "m4v", "mpg", "mpeg", "3gp", "flv",
];

const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "webp", "ico", "tif", "tiff", "svg",
];

pub fn is_image_path(path: &Path) -> bool {
    matches!(
        preview_kind(path),
        Some(PreviewKind::Image | PreviewKind::Svg)
    )
}

pub fn is_text_preview_path(path: &Path) -> bool {
    matches!(
        preview_kind(path),
        Some(
            PreviewKind::Markdown
                | PreviewKind::Html
                | PreviewKind::Code
                | PreviewKind::Text
        )
    )
}

pub fn preview_kind(path: &Path) -> Option<PreviewKind> {
    if !path.is_file() {
        return None;
    }

    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase());

    match ext.as_deref() {
        Some(ext) if IMAGE_EXTENSIONS.contains(&ext) => {
            if ext == "svg" {
                Some(PreviewKind::Svg)
            } else {
                Some(PreviewKind::Image)
            }
        }
        Some("md") => Some(PreviewKind::Markdown),
        Some("html" | "htm") => Some(PreviewKind::Html),
        Some(
            "json" | "xml" | "yaml" | "yml" | "toml" | "rs" | "css" | "js" | "ts" | "tsx"
            | "jsx" | "py" | "c" | "cpp" | "h" | "hpp" | "cs" | "java" | "gradle" | "go" | "sql"
            | "sh" | "bat" | "ps1",
        ) => Some(PreviewKind::Code),
        Some("txt" | "log" | "csv" | "ini" | "cfg") => Some(PreviewKind::Text),
        Some("pdf") => Some(PreviewKind::Pdf),
        Some(ext) if AUDIO_EXTENSIONS.contains(&ext) => Some(PreviewKind::Audio),
        Some(ext) if VIDEO_EXTENSIONS.contains(&ext) => Some(PreviewKind::Video),
        None if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case(".gitignore")) =>
        {
            Some(PreviewKind::Code)
        }
        _ => None,
    }
}

pub fn read_text_preview(path: &Path) -> anyhow::Result<String> {
    let data = std::fs::read(path)?;
    let truncated = data.len() > MAX_TEXT_PREVIEW_BYTES;
    let slice = &data[..data.len().min(MAX_TEXT_PREVIEW_BYTES)];
    let mut text = String::from_utf8_lossy(slice).into_owned();
    if truncated {
        text.push_str("\n…");
    }
    Ok(text)
}
