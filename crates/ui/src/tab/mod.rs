//! CyberFiles-maintained fork of gpui-component `tab`.
//!
//! Source copied from gpui-component v0.5.x (`crates/ui/src/tab/`).
//! Modify here for local maintenance; tab-bar bottom rule is opt-in via [`TabBar::bottom_border`].
#![allow(dead_code)]

mod tab;
mod tab_bar;

pub use tab::*;
pub use tab_bar::*;
