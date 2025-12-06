use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub picker_directories: HashMap<String, PathBuf>,
}

impl Config {
    fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("daw").join("config.toml"))
    }

    pub fn load() -> Self {
        let Some(path) = Self::config_path() else {
            return Self::default();
        };

        match fs::read_to_string(&path) {
            Ok(contents) => toml::from_str(&contents).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) {
        let Some(path) = Self::config_path() else {
            return;
        };

        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        if let Ok(contents) = toml::to_string_pretty(self) {
            let _ = fs::write(&path, contents);
        }
    }
}
