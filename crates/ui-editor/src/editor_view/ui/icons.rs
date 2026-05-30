//! Editor toolbar icons — same helpers as CyberFiles (`toolbar_icon` + `toolbar_icon_button`).

pub use cyberfiles_ui::{toolbar_icon, toolbar_icon_button};

pub(crate) mod paths {
    pub const CLOSE: &str = "icons/editor_close.svg";
    pub const FIND_PREV: &str = "icons/editor_find_prev.svg";
    pub const FIND_NEXT: &str = "icons/editor_find_next.svg";
    pub const MATCH_CASE: &str = "icons/editor_match_case.svg";
    pub const MATCH_WORD: &str = "icons/editor_match_word.svg";
    pub const REGEX: &str = "icons/editor_regex.svg";
    pub const SEARCH: &str = "icons/editor_search.svg";
    pub const REPLACE: &str = "icons/editor_replace.svg";
    pub const REPLACE_ALL: &str = "icons/editor_replace_all.svg";
    pub const COUNT: &str = "icons/editor_count.svg";
    pub const GOTO: &str = "icons/editor_goto.svg";
    pub const SETTINGS: &str = "icons/settings-2.svg";
    pub const FOLD_EXPANDED: &str = "icons/chevron-down.svg";
    pub const FOLD_COLLAPSED: &str = "icons/chevron-right.svg";
}
