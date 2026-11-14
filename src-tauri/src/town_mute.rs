//! Lowers the game's volume while the player is in town.
//!
//! Signal: `Client.txt` (read only). Action: the Windows volume mixer. Nothing else.

use crate::areas::{self, AreaKind};
use crate::audio::Ducker;
use crate::client_log::{self, LogEvent, LogTail};
use crate::game::{GameFinder, GameProcess};
use crate::settings::{Settings, TownMuteSettings};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const TICK: Duration = Duration::from_millis(250);
/// How often to look for the game process.
const SLOW_TICK: Duration = Duration::from_secs(2);
/// How often to check the game's volume, so a session the game opens while loading a
/// zone is set right within a second.
const AUDIO_TICK: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub log_path: Option<String>,
    pub log_error: Option<String>,
    pub game_running: bool,
    pub area_id: Option<String>,
    pub area_name: Option<String>,
    pub area_kind: AreaKind,
    pub lowered: bool,
    pub game_sessions: usize,
    pub audio_error: Option<String>,
}

impl Default for Status {
    fn default() -> Self {
        Status {
            log_path: None,
            log_error: None,
            game_running: false,
            area_id: None,
            area_name: None,
            area_kind: AreaKind::Unknown,
            lowered: false,
            game_sessions: 0,
            audio_error: None,
        }
    }
}

/// The area the player is in, from the log.
#[derive(Debug, Default, Clone)]
pub struct AreaTracker {
    pub area_id: Option<String>,
    pub area_name: Option<String>,
    name_is_town: bool,
}

impl AreaTracker {
    pub fn apply(&mut self, event: &LogEvent) {
        match event {
            LogEvent::AreaGenerated { area_id, .. } => {
                self.area_id = Some(area_id.clone());
                self.area_name = None;
                self.name_is_town = false;
            }
            LogEvent::AreaEntered { name } => {
                self.name_is_town = areas::is_town_name(name);
                self.area_name = Some(name.clone());
            }
        }
    }

    pub fn kind(&self, settings: &TownMuteSettings) -> AreaKind {
        let custom = settings.extra_areas.iter().any(|a| {
            let a = a.trim();
            !a.is_empty()
                && (self.area_id.as_deref().is_some_and(|id| id.eq_ignore_ascii_case(a))
                    || self.area_name.as_deref().is_some_and(|n| n.eq_ignore_ascii_case(a)))
        });
        if custom || self.name_is_town {
            return AreaKind::Town;
        }
        match &self.area_id {
            Some(id) => areas::classify_id(id),
            None => AreaKind::Unknown,
        }
    }

    pub fn should_lower(&self, settings: &TownMuteSettings) -> bool {
        settings.enabled
            && match self.kind(settings) {
                AreaKind::Town => true,
                AreaKind::Hideout => settings.include_hideouts,
                AreaKind::Other | AreaKind::Unknown => false,
            }
    }
}

pub struct TownMute {
    settings: Arc<Mutex<Settings>>,
    ducker: Arc<Mutex<Ducker>>,
    finder: GameFinder,
    game: GameProcess,
    tail: Option<LogTail>,
    tracker: AreaTracker,
    status: Status,
}

impl TownMute {
    pub fn new(settings: Arc<Mutex<Settings>>, ducker: Arc<Mutex<Ducker>>) -> Self {
        TownMute {
            settings,
            ducker,
            finder: GameFinder::new(),
            game: GameProcess::default(),
            tail: None,
            tracker: AreaTracker::default(),
            status: Status::default(),
        }
    }

    /// Runs forever on its own thread, calling `on_status` whenever the status changes.
    pub fn run(mut self, on_status: impl Fn(&Status)) {
        let mut last_slow = Instant::now() - SLOW_TICK;
        let mut last_audio = Instant::now() - AUDIO_TICK;
        let mut last_settings = None;
        let mut last_lower = false;
        let mut last_sent: Option<Status> = None;
        loop {
            let settings = self.settings.lock().unwrap().town_mute.clone();
            let slow = last_slow.elapsed() >= SLOW_TICK;
            if slow {
                last_slow = Instant::now();
                self.game = self.finder.find();
                self.status.game_running = self.game.running();
            }
            let settings_changed = last_settings.as_ref() != Some(&settings);

            if slow || settings_changed {
                self.ensure_log(&settings);
            }
            self.read_log();

            let lower = self.tracker.should_lower(&settings) && self.game.running();
            // Apply at once when something changed, and every second anyway, so new
            // audio sessions and a restarted game are covered too.
            if last_audio.elapsed() >= AUDIO_TICK || settings_changed || lower != last_lower {
                last_audio = Instant::now();
                self.apply_volume(lower, &settings);
            }
            last_lower = lower;
            last_settings = Some(settings.clone());

            self.status.area_id = self.tracker.area_id.clone();
            self.status.area_name = self.tracker.area_name.clone();
            self.status.area_kind = self.tracker.kind(&settings);
            if last_sent.as_ref() != Some(&self.status) {
                on_status(&self.status);
                last_sent = Some(self.status.clone());
            }
            std::thread::sleep(TICK);
        }
    }

    fn ensure_log(&mut self, settings: &TownMuteSettings) {
        let wanted: Option<PathBuf> = match settings.client_log_path.as_deref().map(str::trim) {
            Some(p) if !p.is_empty() => Some(PathBuf::from(p)),
            _ => client_log::candidate_paths(self.game.exe.as_deref())
                .into_iter()
                .find(|p| p.is_file()),
        };
        let Some(wanted) = wanted else {
            self.tail = None;
            self.status.log_path = None;
            self.status.log_error = Some("Client.txt not found".into());
            return;
        };
        if self.tail.as_ref().is_some_and(|t| t.path() == wanted) {
            return;
        }
        match LogTail::open(&wanted) {
            Ok((tail, events)) => {
                self.tracker = AreaTracker::default();
                for e in &events {
                    self.tracker.apply(e);
                }
                self.tail = Some(tail);
                self.status.log_path = Some(wanted.display().to_string());
                self.status.log_error = None;
            }
            Err(e) => {
                self.tail = None;
                self.status.log_path = Some(wanted.display().to_string());
                self.status.log_error = Some(e.to_string());
            }
        }
    }

    fn read_log(&mut self) {
        let Some(tail) = self.tail.as_mut() else { return };
        match tail.poll() {
            Ok(events) => {
                for e in &events {
                    self.tracker.apply(e);
                }
                self.status.log_error = None;
            }
            Err(e) => self.status.log_error = Some(e.to_string()),
        }
    }

    fn apply_volume(&mut self, lower: bool, settings: &TownMuteSettings) {
        let focused = crate::hotkey::game_is_focused();
        let mut ducker = self.ducker.lock().unwrap();
        let result = if lower {
            let factor = f32::from(settings.town_volume.min(100)) / 100.0;
            ducker.lower(&self.game.pids, factor, focused).map(|n| {
                self.status.game_sessions = n;
            })
        } else {
            ducker.restore(focused)
        };
        self.status.audio_error = result.err();
        self.status.lowered = ducker.has_lowered();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generated(id: &str) -> LogEvent {
        LogEvent::AreaGenerated {
            level: 1,
            area_id: id.into(),
        }
    }

    #[test]
    fn lowers_in_town_and_not_in_maps() {
        let s = TownMuteSettings::default();
        let mut t = AreaTracker::default();
        assert!(!t.should_lower(&s));
        t.apply(&generated("1_1_town"));
        assert!(t.should_lower(&s));
        t.apply(&generated("MapWorldsStrand"));
        assert!(!t.should_lower(&s));
    }

    #[test]
    fn hideouts_only_when_enabled() {
        let mut s = TownMuteSettings::default();
        let mut t = AreaTracker::default();
        t.apply(&generated("HideoutLuxurious"));
        assert!(!t.should_lower(&s));
        s.include_hideouts = true;
        assert!(t.should_lower(&s));
    }

    #[test]
    fn disabled_never_lowers() {
        let s = TownMuteSettings {
            enabled: false,
            ..Default::default()
        };
        let mut t = AreaTracker::default();
        t.apply(&generated("1_1_town"));
        assert!(!t.should_lower(&s));
    }

    #[test]
    fn extra_areas_match_id_or_name() {
        let s = TownMuteSettings {
            extra_areas: vec!["the menagerie".into()],
            ..Default::default()
        };
        let mut t = AreaTracker::default();
        t.apply(&generated("Menagerie_Hub"));
        assert!(!t.should_lower(&s));
        t.apply(&LogEvent::AreaEntered {
            name: "The Menagerie".into(),
        });
        assert!(t.should_lower(&s));
    }

    #[test]
    fn town_name_fallback_resets_on_next_area() {
        let s = TownMuteSettings::default();
        let mut t = AreaTracker::default();
        t.apply(&generated("SomeNewTownId"));
        t.apply(&LogEvent::AreaEntered {
            name: "Kingsmarch".into(),
        });
        assert!(t.should_lower(&s));
        t.apply(&generated("1_1_2"));
        assert!(!t.should_lower(&s));
    }
}
