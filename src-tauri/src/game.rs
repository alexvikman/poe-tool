//! Finds the running Path of Exile process (by name only).

use std::collections::HashSet;
use std::path::PathBuf;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

#[derive(Debug, Default, Clone)]
pub struct GameProcess {
    pub pids: HashSet<u32>,
    pub exe: Option<PathBuf>,
}

impl GameProcess {
    pub fn running(&self) -> bool {
        !self.pids.is_empty()
    }
}

/// PathOfExile.exe, PathOfExile_x64.exe, PathOfExileSteam.exe, PathOfExile_x64_KG.exe, …
pub fn is_game_exe(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let stem = lower.strip_suffix(".exe").unwrap_or(&lower);
    stem.starts_with("pathofexile")
}

pub struct GameFinder {
    system: Option<System>,
}

impl GameFinder {
    pub fn new() -> Self {
        GameFinder {
            system: System::new().ok(),
        }
    }

    pub fn find(&mut self) -> GameProcess {
        let Some(system) = self.system.as_mut() else {
            return GameProcess::default();
        };
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
        );
        let mut game = GameProcess::default();
        for (pid, process) in system.processes() {
            if is_game_exe(&process.name().to_string_lossy()) {
                game.pids.insert(pid.as_u32());
                if game.exe.is_none() {
                    game.exe = process.exe().map(PathBuf::from);
                }
            }
        }
        game
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_all_client_variants() {
        for name in [
            "PathOfExile.exe",
            "PathOfExile_x64.exe",
            "PathOfExileSteam.exe",
            "PathOfExile_x64Steam.exe",
            "PathOfExile_x64_KG.exe",
            "PathOfExileEGS.exe",
        ] {
            assert!(is_game_exe(name), "{name}");
        }
        assert!(!is_game_exe("poe-tool.exe"));
        assert!(!is_game_exe("explorer.exe"));
    }
}
