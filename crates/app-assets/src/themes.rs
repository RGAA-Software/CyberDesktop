//! Embedded UI color themes (gpui-component `ThemeSet` JSON).

/// CyberFiles design tokens (light + dark).
pub const CYBERFILES: &str = include_str!("../themes/cyberfiles.json");

/// CyberEditor design tokens (light + dark), based on CyberFiles with blue accent.
pub const CYBEREDITOR: &str = include_str!("../themes/cybereditor.json");

/// CyberMediaPlayer design tokens (light + dark), based on CyberFiles with teal accent.
pub const CYBERMEDIAPLAYER: &str = include_str!("../themes/cybermediaplayer.json");
