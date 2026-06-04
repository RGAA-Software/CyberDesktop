//! Compress context-menu label helpers (kept out of `helpers.rs` for unit tests).

use std::path::{Path, PathBuf};

use files_fs::compress_zip_file_display_name;
use gpui::SharedString;
use rust_i18n::t;

/// Max characters for the zip file name inside «Compress (name.zip)».
const COMPRESS_MENU_ZIP_NAME_MAX_CHARS: usize = 28;

pub(super) fn truncate_middle_chars(text: &str, max_chars: usize) -> String {
    const ELLIPSIS: &str = "...";
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    if len <= max_chars {
        return text.to_string();
    }
    let ellipsis_len = ELLIPSIS.chars().count();
    if max_chars <= ellipsis_len {
        return ELLIPSIS.chars().take(max_chars).collect();
    }
    let budget = max_chars - ellipsis_len;
    let head_len = budget.div_ceil(2);
    let tail_len = budget / 2;
    let head: String = chars.iter().take(head_len).copied().collect();
    let tail: String = chars.iter().skip(len - tail_len).copied().collect();
    format!("{head}{ELLIPSIS}{tail}")
}

pub(super) fn compress_context_menu_label(paths: &[PathBuf], destination: &Path) -> SharedString {
    let zip_name = compress_zip_file_display_name(paths, destination);
    let zip_name = truncate_middle_chars(&zip_name, COMPRESS_MENU_ZIP_NAME_MAX_CHARS);
    t!("files.menu.compress_with_name", name = zip_name).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_middle_preserves_ends() {
        let truncated = truncate_middle_chars("积极的的大师傅士大夫大夫士大夫.zip", 18);
        assert!(truncated.contains("..."));
        assert!(truncated.starts_with("积极"));
        assert!(truncated.ends_with(".zip"));
        assert!(truncated.chars().count() <= 18);
    }

    #[test]
    fn truncate_middle_short_string_unchanged() {
        assert_eq!(truncate_middle_chars("a.zip", 28), "a.zip");
    }
}
