use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TownMuteSettings {
    pub enabled: bool,
    /// The game's volume in town, in percent of its normal volume. 0 is silent.
    pub town_volume: u8,
    pub include_hideouts: bool,
    /// Overrides the automatically found `Client.txt`.
    pub client_log_path: Option<String>,
    /// More areas to treat as towns: area ids or names, as the log writes them.
    pub extra_areas: Vec<String>,
}

impl Default for TownMuteSettings {
    fn default() -> Self {
        TownMuteSettings {
            enabled: true,
            town_volume: 10,
            include_hideouts: false,
            client_log_path: None,
            extra_areas: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub town_mute: TownMuteSettings,
}

impl Settings {
    pub fn load(path: &Path) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| e.to_string())
    }
}
