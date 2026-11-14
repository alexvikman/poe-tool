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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PriceCheckSettings {
    pub enabled: bool,
    /// Registered only while the game window is in front.
    pub hotkey: String,
    /// None: the current challenge league.
    pub league: Option<String>,
    /// Search only listings that can be bought at once from a merchant tab.
    pub instant_buyout: bool,
}

impl Default for PriceCheckSettings {
    fn default() -> Self {
        PriceCheckSettings {
            enabled: true,
            hotkey: "Ctrl+D".into(),
            league: None,
            instant_buyout: true,
        }
    }
}

/// A hotkey that types a chat command in the game: one key press, one command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ChatCommand {
    pub enabled: bool,
    pub hotkey: String,
    /// What to type, such as `/hideout`.
    pub text: String,
    /// Press Enter after typing; otherwise the text waits in the chat box to be edited.
    pub send: bool,
}

impl Default for ChatCommand {
    fn default() -> Self {
        ChatCommand {
            enabled: true,
            hotkey: String::new(),
            text: String::new(),
            send: true,
        }
    }
}

/// What a hotkey does.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HotkeyAction {
    PriceCheck,
    Chat { text: String, send: bool },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub town_mute: TownMuteSettings,
    pub price_check: PriceCheckSettings,
    pub chat_commands: Vec<ChatCommand>,
    pub pob_service: PobServiceSettings,
}

/// Another Path of Building service than PoE Tool's own (a local copy, for testing).
/// Empty is PoE Tool's own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PobServiceSettings {
    /// "http://localhost:8080", without a path.
    pub url: String,
    /// Its key, when it needs another than the one PoE Tool ships with.
    pub api_key: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            town_mute: TownMuteSettings::default(),
            price_check: PriceCheckSettings::default(),
            chat_commands: vec![ChatCommand {
                hotkey: "F5".into(),
                text: "/hideout".into(),
                ..ChatCommand::default()
            }],
            pob_service: PobServiceSettings::default(),
        }
    }
}

impl Settings {
    /// The hotkeys in use and what each does: price check and the chat commands that
    /// are on and have both a key and a text.
    pub fn hotkeys(&self) -> Vec<(String, HotkeyAction)> {
        let mut out = Vec::new();
        if self.price_check.enabled && !self.price_check.hotkey.trim().is_empty() {
            out.push((self.price_check.hotkey.clone(), HotkeyAction::PriceCheck));
        }
        for c in &self.chat_commands {
            if c.enabled && !c.hotkey.trim().is_empty() && !c.text.trim().is_empty() {
                out.push((
                    c.hotkey.clone(),
                    HotkeyAction::Chat { text: c.text.trim().to_string(), send: c.send },
                ));
            }
        }
        out
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_settings_files_get_the_hideout_command() {
        let s: Settings = serde_json::from_str(r#"{"townMute":{"enabled":false}}"#).unwrap();
        assert!(!s.town_mute.enabled);
        assert_eq!(s.chat_commands.len(), 1);
        assert_eq!(s.chat_commands[0].hotkey, "F5");
        assert_eq!(s.chat_commands[0].text, "/hideout");
    }

    #[test]
    fn hotkeys_skip_unfinished_and_disabled_commands() {
        let mut s = Settings::default();
        s.chat_commands.push(ChatCommand { hotkey: "F6".into(), ..ChatCommand::default() });
        s.chat_commands.push(ChatCommand {
            enabled: false,
            hotkey: "F7".into(),
            text: "/exit".into(),
            ..ChatCommand::default()
        });
        let keys: Vec<String> = s.hotkeys().into_iter().map(|(k, _)| k).collect();
        assert_eq!(keys, vec!["Ctrl+D", "F5"]);
    }
}
