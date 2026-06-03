use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MediaPlayerConfig {
    pub recent_paths: Vec<PathBuf>,
    pub volume: f64,
    pub muted: bool,
    pub remember_position: bool,
    pub last_positions: Vec<(PathBuf, f64)>,
}

impl MediaPlayerConfig {
    pub fn config_path() -> Option<PathBuf> {
        directories::ProjectDirs::from("com", "rg-software", "cyber_media_player")
            .map(|dirs| dirs.config_dir().join("config.json"))
    }

    pub fn load() -> Self {
        if let Some(path) = Self::config_path() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(config) = serde_json::from_str(&content) {
                    return config;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        if let Some(path) = Self::config_path() {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(content) = serde_json::to_string_pretty(self) {
                let _ = fs::write(&path, content);
            }
        }
    }

    pub fn record_position(&mut self, path: &PathBuf, seconds: f64) {
        self.last_positions.retain(|(p, _)| p != path);
        self.last_positions.push((path.clone(), seconds));
        if self.last_positions.len() > 100 {
            self.last_positions.remove(0);
        }
    }

    pub fn get_position(&self, path: &PathBuf) -> Option<f64> {
        self.last_positions
            .iter()
            .find(|(p, _)| p == path)
            .map(|(_, s)| *s)
    }

    pub fn add_recent(&mut self, path: PathBuf) {
        self.recent_paths.retain(|p| p != &path);
        self.recent_paths.insert(0, path);
        if self.recent_paths.len() > 50 {
            self.recent_paths.pop();
        }
    }
}
