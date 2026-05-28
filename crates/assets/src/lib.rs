//! Embedded icon assets for CyberFiles.
//!
//! Toolbar and navigation icons use [Google Material Symbols](https://fonts.google.com/icons?icon.style=Rounded)
//! (Rounded, default 24px) synced into `assets/icons/`. Window chrome, theme toggle, GitHub, and tab
//! close icons stay on gpui-component (Lucide) artwork.
//!
//! Run `python scripts/sync_material_icons.py` to refresh Material SVGs.
//!
//! UI color themes live in `themes/*.json` (see [`themes`] module).

pub mod themes;

use gpui::{AssetSource, Result, SharedString};
use gpui_component_assets::Assets as ComponentAssets;
use std::borrow::Cow;
use std::sync::OnceLock;

/// Zed editor gutter/icons expect paths under `icons/file_icons/` (see `theme::default_icon_theme`).
const ZED_ICON_PATH_ALIASES: &[(&str, &str)] = &[
    (
        "icons/file_icons/chevron_right.svg",
        "icons/chevron-right.svg",
    ),
    (
        "icons/file_icons/chevron_down.svg",
        "icons/chevron-down.svg",
    ),
    ("icons/chevron_right.svg", "icons/chevron-right.svg"),
    ("icons/chevron_down.svg", "icons/chevron-down.svg"),
];

/// GPUI icon paths that must use bundled Lucide SVGs, not Material replacements.
const LUCIDE_ICON_PATHS: &[&str] = &[
    "icons/window-close.svg",
    "icons/window-minimize.svg",
    "icons/window-maximize.svg",
    "icons/window-restore.svg",
    "icons/github.svg",
    "icons/moon.svg",
    "icons/sun.svg",
    "icons/close.svg",
];

fn component_assets() -> &'static ComponentAssets {
    static ASSETS: OnceLock<ComponentAssets> = OnceLock::new();
    ASSETS.get_or_init(|| ComponentAssets::new(""))
}

fn use_lucide_icon(path: &str) -> bool {
    LUCIDE_ICON_PATHS.contains(&path)
}

#[derive(rust_embed::RustEmbed)]
#[folder = "assets"]
#[include = "icons/**"]
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }

        let path = ZED_ICON_PATH_ALIASES
            .iter()
            .find_map(|(zed_path, local_path)| (*zed_path == path).then_some(*local_path))
            .unwrap_or(path);

        if use_lucide_icon(path) {
            return component_assets().load(path);
        }

        if let Some(file) = Self::get(path) {
            return Ok(Some(file.data));
        }

        component_assets().load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut names: Vec<SharedString> = Self::iter()
            .filter_map(|p| p.starts_with(path).then(|| p.into()))
            .collect();
        let mut from_component = component_assets().list(path)?;
        names.append(&mut from_component);
        names.sort();
        names.dedup();
        Ok(names)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zed_gutter_chevron_icons_load() {
        let assets = Assets;
        for path in [
            "icons/chevron_right.svg",
            "icons/chevron_down.svg",
            "icons/file_icons/chevron_right.svg",
        ] {
            let data = assets.load(path).expect("load");
            assert!(data.is_some(), "missing {path}");
        }
    }

    #[test]
    fn lucide_window_and_chrome_icons_load() {
        let assets = Assets;
        for path in LUCIDE_ICON_PATHS {
            let data = assets.load(path).expect("load");
            assert!(data.is_some(), "missing {path}");
        }
    }

    #[test]
    fn custom_colored_icons_load() {
        let assets = Assets;
        for path in [
            "icons/ic_folder.svg",
            "icons/ic_home.svg",
            "icons/ic_copy.svg",
            "icons/ic_cut.svg",
            "icons/ic_paste.svg",
            "icons/ic_new_folder.svg",
            "icons/ic_new_file.svg",
        ] {
            let data = assets.load(path).expect("load");
            assert!(data.is_some(), "missing {path}");
            let bytes = data.unwrap();
            let text = String::from_utf8_lossy(&bytes);
            assert!(text.contains("<svg"), "{path} is not a valid SVG");
            assert!(
                text.contains("fill=") || text.contains("stroke="),
                "{path} does not appear to be colored"
            );
        }
    }
}
