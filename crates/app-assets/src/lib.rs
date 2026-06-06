//! Embedded icon assets for CyberFiles.
//!
//! CyberFiles toolbar, navigation, and list icons use [Tabler Icons](https://tabler.io/icons) (outline,
//! 24px) under `assets/icons/tabler/`. Run `python scripts/sync_tabler_icons.py` to refresh them.
//!
//! Editor gutter chevrons, CyberEditor toolbar icons, and a few shared paths (`plus.svg`, `settings-2.svg`)
//! remain as bundled SVGs in `assets/icons/`. Window chrome, theme toggle, GitHub, and tab close icons
//! load from gpui-component (Lucide) artwork.
//!
//! UI color themes live in `themes/*.json` (see [`themes`] module).

pub mod themes;

use anyhow::Context as _;
use gpui::{App, AssetSource, Result, SharedString};
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

/// GPUI icon paths that must use bundled Lucide SVGs, not Tabler replacements.
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
#[include = "fonts/**"]
#[include = "icons/**"]
#[include = "app/**"]
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

impl Assets {
    pub fn load_fonts(&self, cx: &App) -> anyhow::Result<()> {
        let font_paths = self.list("fonts")?;
        let mut embedded_fonts = Vec::new();
        for font_path in font_paths {
            if font_path.ends_with(".ttf") {
                let font_bytes = cx
                    .asset_source()
                    .load(&font_path)?
                    .with_context(|| format!("loading font asset at path {font_path:?}"))?;
                embedded_fonts.push(font_bytes);
            }
        }

        cx.text_system().add_fonts(embedded_fonts)
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
    fn editor_toolbar_icons_load() {
        let assets = Assets;
        for path in [
            "icons/editor_close.svg",
            "icons/editor_search.svg",
            "icons/editor_find_prev.svg",
            "icons/editor_find_next.svg",
            "icons/editor_match_case.svg",
            "icons/editor_match_word.svg",
            "icons/editor_regex.svg",
            "icons/editor_replace_all.svg",
        ] {
            let data = assets.load(path).expect("load");
            assert!(data.is_some(), "missing {path}");
        }
    }

    #[test]
    fn app_logo_loads() {
        let assets = Assets;
        let data = assets
            .load("app/ic_file_manager_logo.svg")
            .expect("load");
        assert!(data.is_some(), "missing app/ic_file_manager_logo.svg");
    }

    #[test]
    fn tabler_icons_load() {
        let assets = Assets;
        for path in [
            "icons/tabler/files.svg",
            "icons/tabler/folder.svg",
            "icons/tabler/home.svg",
            "icons/tabler/copy.svg",
            "icons/tabler/cut.svg",
            "icons/tabler/layout-columns.svg",
        ] {
            let data = assets.load(path).expect("load");
            assert!(data.is_some(), "missing {path}");
            let bytes = data.unwrap();
            let text = String::from_utf8_lossy(&bytes);
            assert!(text.contains("<svg"), "{path} is not a valid SVG");
        }
    }
}
