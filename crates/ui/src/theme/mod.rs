//! CyberFiles theme catalog — Zed built-in themes (One, Ayu, Gruvbox) via gpui-component `ThemeSet` JSON.

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, OnceLock};

use cyberfiles_assets::themes::{ANT, AYU, GRUVBOX, ONE};
use gpui::{App, SharedString};
use gpui_component::{Theme, ThemeConfig, ThemeMode, ThemeSet};

/// Persisted `theme_name` in `settings.json` (theme set id).
pub const DEFAULT_THEME_SET_ID: &str = "One";

static CATALOG: OnceLock<ThemeCatalog> = OnceLock::new();

/// One selectable theme set (paired light + dark, matching Zed defaults where applicable).
#[derive(Debug, Clone)]
pub struct ThemeSetEntry {
    pub id: SharedString,
    pub display_name: SharedString,
    pub light: Arc<ThemeConfig>,
    pub dark: Arc<ThemeConfig>,
}

#[derive(Debug)]
pub struct ThemeCatalog {
    pub default_id: SharedString,
    sets: HashMap<SharedString, ThemeSetEntry>,
}

impl ThemeCatalog {
    pub fn global() -> &'static Self {
        CATALOG.get_or_init(Self::load_embedded)
    }

    pub fn sets(&self) -> impl Iterator<Item = &ThemeSetEntry> {
        self.sets.values()
    }

    pub fn sorted_sets(&self) -> Vec<&ThemeSetEntry> {
        let mut sets: Vec<_> = self.sets().collect();
        sets.sort_by(|a, b| {
            a.display_name
                .to_lowercase()
                .cmp(&b.display_name.to_lowercase())
        });
        sets
    }

    pub fn get(&self, id: &str) -> Option<&ThemeSetEntry> {
        self.sets.get(id)
    }

    pub fn normalize_set_id(name: &str) -> SharedString {
        let base = name
            .strip_suffix(" Light")
            .or_else(|| name.strip_suffix(" Dark"))
            .unwrap_or(name);
        match base {
            "Default" | "Default Light" | "Default Dark" => DEFAULT_THEME_SET_ID.into(),
            "CyberFiles" | "CyberFiles Blue" | "CyberFiles Mint" | "CyberFiles Yellow" => {
                DEFAULT_THEME_SET_ID.into()
            }
            "One Dark" | "One Light" => "One".into(),
            "Ayu Dark" | "Ayu Light" => "Ayu".into(),
            "Ayu Mirage" => "Ayu Mirage".into(),
            "Gruvbox Dark" | "Gruvbox Light" => "Gruvbox".into(),
            "Gruvbox Dark Hard" | "Gruvbox Light Hard" => "Gruvbox Hard".into(),
            "Gruvbox Dark Soft" | "Gruvbox Light Soft" => "Gruvbox Soft".into(),
            other => other.into(),
        }
    }

    fn load_embedded() -> Self {
        let mut sets = HashMap::new();
        for json in [ANT, ONE, AYU, GRUVBOX] {
            if let Ok(set) = serde_json::from_str::<ThemeSet>(json) {
                for entry in ThemeSetEntry::entries_from_family(set) {
                    sets.insert(entry.id.clone(), entry);
                }
            }
        }
        let default_id = sets
            .keys()
            .find(|id| id.as_ref() == DEFAULT_THEME_SET_ID)
            .cloned()
            .unwrap_or_else(|| DEFAULT_THEME_SET_ID.into());
        Self { default_id, sets }
    }
}

impl ThemeSetEntry {
    fn entries_from_family(set: ThemeSet) -> Vec<Self> {
        let family = set.name.to_string();
        let mut by_name: HashMap<String, Arc<ThemeConfig>> = HashMap::new();
        for theme in set.themes {
            by_name.insert(theme.name.to_string(), Arc::new(theme));
        }

        let mut dark_names: Vec<String> = by_name
            .values()
            .filter(|t| t.mode.is_dark())
            .map(|t| t.name.to_string())
            .collect();
        dark_names.sort();

        let mut entries = Vec::new();
        for dark_name in dark_names {
            let Some(dark) = by_name.get(&dark_name).cloned() else {
                continue;
            };
            let light_name = light_partner_name(&family, &dark_name);
            let light = by_name
                .get(&light_name)
                .cloned()
                .or_else(|| by_name.values().find(|t| !t.mode.is_dark()).cloned());
            let Some(light) = light else {
                continue;
            };
            let id = set_id_for_pair(&family, &dark_name);
            entries.push(Self {
                id: id.clone().into(),
                display_name: id.into(),
                light,
                dark,
            });
        }
        entries
    }
}

fn light_partner_name(family: &str, dark_name: &str) -> String {
    if dark_name == "Ayu Mirage" {
        return "Ayu Light".into();
    }
    if let Some(base) = dark_name.strip_suffix(" Dark Soft") {
        return format!("{base} Light Soft");
    }
    if let Some(base) = dark_name.strip_suffix(" Dark Hard") {
        return format!("{base} Light Hard");
    }
    if let Some(base) = dark_name.strip_suffix(" Dark") {
        return format!("{base} Light");
    }
    format!("{family} Light")
}

fn set_id_for_pair(family: &str, dark_name: &str) -> String {
    if dark_name == "Ayu Mirage" {
        return "Ayu Mirage".into();
    }
    if dark_name.ends_with(" Dark Soft") {
        return format!("{family} Soft");
    }
    if dark_name.ends_with(" Dark Hard") {
        return format!("{family} Hard");
    }
    if dark_name.ends_with(" Dark") {
        return family.to_string();
    }
    dark_name.to_string()
}

/// Install Zed themes after `gpui_component::init` (replaces active light/dark configs).
pub fn install(cx: &mut App) {
    let catalog = ThemeCatalog::global();
    let default = catalog
        .get(catalog.default_id.as_ref())
        .expect("embedded Zed theme");

    if !cx.has_global::<Theme>() {
        Theme::change(ThemeMode::Light, None, cx);
    }

    let theme = Theme::global_mut(cx);
    theme.light_theme = Rc::new((*default.light).clone());
    theme.dark_theme = Rc::new((*default.dark).clone());
    let mode = theme.mode;
    Theme::change(mode, None, cx);
}

/// Apply a theme set for the given appearance mode.
pub fn apply_set(set_id: &str, mode: ThemeMode, cx: &mut App) {
    let catalog = ThemeCatalog::global();
    let set_id = ThemeCatalog::normalize_set_id(set_id).to_string();
    let Some(entry) = catalog.get(&set_id) else {
        apply_set(catalog.default_id.as_ref(), mode, cx);
        return;
    };

    let theme = Theme::global_mut(cx);
    theme.light_theme = Rc::new((*entry.light).clone());
    theme.dark_theme = Rc::new((*entry.dark).clone());
    Theme::change(mode, None, cx);
}

/// Dropdown options: theme set id → display label.
pub fn theme_set_options() -> Vec<(SharedString, SharedString)> {
    ThemeCatalog::global()
        .sorted_sets()
        .into_iter()
        .map(|entry| (entry.id.clone(), entry.display_name.clone()))
        .collect()
}

pub fn current_theme_set_id(cx: &App) -> SharedString {
    ThemeCatalog::normalize_set_id(Theme::global(cx).theme_name().as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_theme_sets_load() {
        let catalog = ThemeCatalog::load_embedded();
        assert_eq!(catalog.sets.len(), 7);
        for id in [
            "Ant",
            "One",
            "Ayu",
            "Ayu Mirage",
            "Gruvbox",
            "Gruvbox Hard",
            "Gruvbox Soft",
        ] {
            let entry = catalog.get(id).expect(id);
            assert!(entry.light.mode == ThemeMode::Light);
            assert!(entry.dark.mode == ThemeMode::Dark);
        }
        let one = catalog.get("One").unwrap();
        assert_eq!(one.light.name.as_ref(), "One Light");
        assert_eq!(one.dark.name.as_ref(), "One Dark");
    }
}
