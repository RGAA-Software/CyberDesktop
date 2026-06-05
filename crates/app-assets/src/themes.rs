//! Embedded UI color themes (gpui-component `ThemeSet` JSON), ported from Zed built-in themes.

/// Ant Design family (Ant Light / Ant Dark).
pub const ANT: &str = include_str!("../themes/ant.json");

/// Atom One family (One Light / One Dark).
pub const ONE: &str = include_str!("../themes/one.json");

/// Ayu family (Ayu Light / Ayu Dark / Ayu Mirage).
pub const AYU: &str = include_str!("../themes/ayu.json");

/// Gruvbox family (standard, hard, and soft light/dark variants).
pub const GRUVBOX: &str = include_str!("../themes/gruvbox.json");

/// CyberFiles design tokens (light + dark).
pub const CYBERFILES: &str = include_str!("../themes/cyberfiles.json");
