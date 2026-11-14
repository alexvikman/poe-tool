//! Lowers the game's volume in the Windows volume mixer, the same as dragging the
//! game's slider in Volume Mixer by hand. The game itself is never touched.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// Volumes closer than this count as the same (the mixer stores floats).
const SAME: f32 = 0.005;

/// One audio session, as the ducker needs it.
pub trait Session {
    /// Stable per output device and app, so it survives a game restart.
    fn key(&self) -> &str;
    fn pid(&self) -> u32;
    fn volume(&self) -> Result<f32, String>;
    fn set_volume(&self, level: f32) -> Result<(), String>;
}

/// What we know about one of the game's sessions (by key).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct Entry {
    /// The volume before we lowered it: what we put back.
    original: f32,
    /// The volume we lowered it to. A session found at this level outside town
    /// is ours to put back, even one the game opened after we restored.
    lowered_to: f32,
    /// Lowered right now.
    active: bool,
}

/// Lowers the game's sessions in town and puts them back outside. Restoring is a
/// state, not an event: every call outside town puts back any game session still at
/// the level we lowered it to. The game can open a new session while loading a zone,
/// and Windows gives it the app's last volume, which may be our lowered one.
///
/// The entries are kept on disk too, so a crash while the game is quiet is put right
/// at the next start.
pub struct Ducker {
    entries: HashMap<String, Entry>,
    state_file: PathBuf,
}

impl Ducker {
    pub fn new(state_file: PathBuf) -> Self {
        let entries = std::fs::read(&state_file)
            .ok()
            .and_then(|b| {
                serde_json::from_slice::<HashMap<String, Entry>>(&b)
                    .ok()
                    // The older format: key → original volume, all lowered.
                    .or_else(|| {
                        serde_json::from_slice::<HashMap<String, f32>>(&b).ok().map(|old| {
                            old.into_iter()
                                .map(|(k, v)| (k, Entry { original: v, lowered_to: v, active: true }))
                                .collect()
                        })
                    })
            })
            .unwrap_or_default();
        Ducker { entries, state_file }
    }

    /// Whether some session is lowered by us right now.
    pub fn has_lowered(&self) -> bool {
        self.entries.values().any(|e| e.active)
    }

    /// Sets the game's sessions to `factor` of their original volume. Returns how many
    /// game sessions there are. `focused`: the game window is in front (see
    /// [`Ducker`] on why that matters).
    pub fn lower(&mut self, pids: &HashSet<u32>, factor: f32, focused: bool) -> Result<usize, String> {
        let sessions = platform::sessions()?;
        self.lower_in(&sessions, pids, factor, focused)
    }

    /// Puts back every session we lowered, and any game session still at our level.
    pub fn restore(&mut self, focused: bool) -> Result<(), String> {
        if self.entries.is_empty() {
            return Ok(());
        }
        let sessions = platform::sessions()?;
        self.restore_in(&sessions, focused)
    }

    /// The game changes its own session volume when it goes to the background and
    /// comes back (alt-tab), so while it is in the background we never take a level
    /// as the user's original and never raise the volume: that would fight the game,
    /// or make a muted background game audible. Whatever is left is set right within
    /// a second of the game coming back to the front.
    fn lower_in<S: Session>(
        &mut self,
        sessions: &[S],
        pids: &HashSet<u32>,
        factor: f32,
        focused: bool,
    ) -> Result<usize, String> {
        let before = self.entries.clone();
        let mut count = 0;
        for s in sessions.iter().filter(|s| pids.contains(&s.pid())) {
            count += 1;
            let current = s.volume()?;
            let entry = match self.entries.get_mut(s.key()) {
                Some(e) => e,
                // A first sight in the background may be the game's own background level.
                None if !focused => continue,
                None => self.entries.entry(s.key().to_string()).or_insert(Entry {
                    original: current,
                    lowered_to: current,
                    active: false,
                }),
            };
            if !entry.active {
                // A level other than ours, seen while the game is in front, is the
                // user's: the new original. Our own lowered level never is.
                if focused && (current - entry.lowered_to).abs() > SAME {
                    entry.original = current;
                }
                entry.active = true;
            }
            entry.lowered_to = (entry.original * factor).clamp(0.0, 1.0);
            let raise = entry.lowered_to > current;
            if (current - entry.lowered_to).abs() > SAME && (focused || !raise) {
                s.set_volume(entry.lowered_to)?;
            }
        }
        if self.entries != before {
            self.save();
        }
        Ok(count)
    }

    fn restore_in<S: Session>(&mut self, sessions: &[S], focused: bool) -> Result<(), String> {
        let before = self.entries.clone();
        for s in sessions {
            let Some(entry) = self.entries.get_mut(s.key()) else { continue };
            let current = s.volume()?;
            let ours = entry.active || (current - entry.lowered_to).abs() <= SAME;
            if ours && (current - entry.original).abs() > SAME {
                if !focused && entry.original > current {
                    // Wait until the game is in front again (it stays marked lowered).
                    continue;
                }
                s.set_volume(entry.original)?;
            }
            entry.active = false;
        }
        if self.entries != before {
            self.save();
        }
        Ok(())
    }

    fn save(&self) {
        if let Ok(json) = serde_json::to_vec(&self.entries) {
            if let Some(dir) = self.state_file.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(&self.state_file, json);
        }
    }
}

/// Lists audio sessions as `(pid, volume)` for diagnostics.
pub fn list_sessions() -> Result<Vec<(u32, f32)>, String> {
    platform::sessions()?
        .iter()
        .map(|s| Ok((s.pid(), s.volume()?)))
        .collect()
}

#[cfg(windows)]
mod platform {
    use std::ffi::c_void;
    use windows::Win32::Media::Audio::{
        eRender, AudioSessionStateExpired, IAudioSessionControl2, IAudioSessionManager2,
        IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED,
    };
    use windows::core::Interface;

    pub struct WinSession {
        key: String,
        pid: u32,
        volume: ISimpleAudioVolume,
    }

    impl super::Session for WinSession {
        fn key(&self) -> &str {
            &self.key
        }

        fn pid(&self) -> u32 {
            self.pid
        }

        fn volume(&self) -> Result<f32, String> {
            unsafe { self.volume.GetMasterVolume() }.map_err(|e| e.to_string())
        }

        fn set_volume(&self, level: f32) -> Result<(), String> {
            unsafe { self.volume.SetMasterVolume(level, std::ptr::null()) }
                .map_err(|e| e.to_string())
        }
    }

    /// Every live audio session on every active output device. Expired sessions (the
    /// app closed them) are left out: changing their volume does nothing.
    pub fn sessions() -> Result<Vec<WinSession>, String> {
        unsafe { sessions_inner() }.map_err(|e| format!("Windows audio: {e}"))
    }

    unsafe fn sessions_inner() -> windows::core::Result<Vec<WinSession>> {
        unsafe {
            // Fails harmlessly if this thread already uses COM in another mode.
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let devices = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;
            let mut out = Vec::new();
            for i in 0..devices.GetCount()? {
                let device = devices.Item(i)?;
                let manager: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None)?;
                let list = manager.GetSessionEnumerator()?;
                for j in 0..list.GetCount()? {
                    let control = list.GetSession(j)?;
                    if control.GetState().is_ok_and(|s| s == AudioSessionStateExpired) {
                        continue;
                    }
                    let Ok(control2) = control.cast::<IAudioSessionControl2>() else {
                        continue;
                    };
                    let Ok(pid) = control2.GetProcessId() else {
                        continue;
                    };
                    let Ok(id) = control2.GetSessionIdentifier() else {
                        continue;
                    };
                    let key = id.to_string().unwrap_or_default();
                    CoTaskMemFree(Some(id.0 as *const c_void));
                    let volume: ISimpleAudioVolume = control.cast()?;
                    out.push(WinSession { key, pid, volume });
                }
            }
            Ok(out)
        }
    }
}

#[cfg(not(windows))]
mod platform {
    //! Windows only. Elsewhere there are no sessions, so the rest of the app can be
    //! developed and tested on any OS.
    pub fn sessions() -> Result<Vec<super::tests_support::FakeSession>, String> {
        Ok(Vec::new())
    }
}

/// A session in memory, for tests and for builds on other systems.
#[cfg(any(test, not(windows)))]
mod tests_support {
    use std::cell::Cell;

    pub struct FakeSession {
        pub key: String,
        pub pid: u32,
        pub level: Cell<f32>,
    }

    impl super::Session for FakeSession {
        fn key(&self) -> &str {
            &self.key
        }
        fn pid(&self) -> u32 {
            self.pid
        }
        fn volume(&self) -> Result<f32, String> {
            Ok(self.level.get())
        }
        fn set_volume(&self, level: f32) -> Result<(), String> {
            self.level.set(level);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::tests_support::FakeSession;
    use super::*;
    use std::cell::Cell;

    const GAME: u32 = 100;

    fn session(level: f32) -> FakeSession {
        FakeSession { key: "game".into(), pid: GAME, level: Cell::new(level) }
    }

    fn ducker() -> Ducker {
        let file = std::env::temp_dir()
            .join(format!("poe-tool-ducker-{}-{:?}.json", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_file(&file);
        Ducker::new(file)
    }

    fn pids() -> HashSet<u32> {
        HashSet::from([GAME])
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.001
    }

    #[test]
    fn lowers_and_restores() {
        let mut d = ducker();
        let s = [session(0.8)];
        d.lower_in(&s, &pids(), 0.1, true).unwrap();
        assert!(close(s[0].level.get(), 0.08));
        assert!(d.has_lowered());
        d.restore_in(&s, true).unwrap();
        assert!(close(s[0].level.get(), 0.8));
        assert!(!d.has_lowered());
    }

    #[test]
    fn restores_a_session_the_game_opened_after_leaving_town() {
        // Town, then a zone: we restore the old session, but the game replaces it while
        // loading, and Windows gives the new one the app's lowered volume.
        let mut d = ducker();
        let old = [session(1.0)];
        d.lower_in(&old, &pids(), 0.1, true).unwrap();
        d.restore_in(&old, true).unwrap();
        let new = [session(0.1)];
        d.restore_in(&new, true).unwrap();
        assert!(close(new[0].level.get(), 1.0));
    }

    #[test]
    fn never_takes_its_own_lowered_level_as_the_original() {
        // A restore was missed and the game sits at our level; the next town must not
        // remember 10% as the original.
        let mut d = ducker();
        let s = [session(1.0)];
        d.lower_in(&s, &pids(), 0.1, true).unwrap();
        d.restore_in(&[] as &[FakeSession], true).unwrap();
        d.entries.get_mut("game").unwrap().active = false; // the missed restore
        d.lower_in(&s, &pids(), 0.1, true).unwrap();
        d.restore_in(&s, true).unwrap();
        assert!(close(s[0].level.get(), 1.0));
    }

    #[test]
    fn keeps_a_volume_the_user_set_outside_town() {
        let mut d = ducker();
        let s = [session(1.0)];
        d.lower_in(&s, &pids(), 0.1, true).unwrap();
        d.restore_in(&s, true).unwrap();
        s[0].level.set(0.5); // the user turns the game down in the mixer
        d.restore_in(&s, true).unwrap();
        assert!(close(s[0].level.get(), 0.5));
        d.lower_in(&s, &pids(), 0.1, true).unwrap();
        assert!(close(s[0].level.get(), 0.05));
        d.restore_in(&s, true).unwrap();
        assert!(close(s[0].level.get(), 0.5));
    }

    #[test]
    fn alt_tab_while_changing_zones() {
        // Town with the game in front, alt-tab, the game goes quieter in the background,
        // zone change while away, back to the game, which writes back the level it had
        // when it lost focus (ours).
        let mut d = ducker();
        let s = [session(1.0)];
        d.lower_in(&s, &pids(), 0.1, true).unwrap();
        s[0].level.set(0.0); // the game's own background level
        d.restore_in(&s, false).unwrap();
        assert!(close(s[0].level.get(), 0.0), "never raised in the background");
        assert!(d.has_lowered(), "still owed a restore");
        s[0].level.set(0.1); // the game comes back to the front with its saved level
        d.restore_in(&s, true).unwrap();
        assert!(close(s[0].level.get(), 1.0));
    }

    #[test]
    fn town_entered_in_the_background_keeps_the_real_original() {
        let mut d = ducker();
        let s = [session(0.8)];
        d.lower_in(&s, &pids(), 0.1, true).unwrap();
        d.restore_in(&s, true).unwrap();
        s[0].level.set(0.0); // alt-tab: the game mutes itself
        d.lower_in(&s, &pids(), 0.1, false).unwrap();
        assert!(close(s[0].level.get(), 0.0), "not raised to the town level in the background");
        s[0].level.set(0.8); // back in front, the game restores its level
        d.lower_in(&s, &pids(), 0.1, true).unwrap();
        assert!(close(s[0].level.get(), 0.08));
        d.restore_in(&s, true).unwrap();
        assert!(close(s[0].level.get(), 0.8), "0 was never taken as the original");
    }

    #[test]
    fn lowers_a_loud_background_game_in_town() {
        // Without the game's own background muting, town noise is still lowered.
        let mut d = ducker();
        let s = [session(1.0)];
        d.lower_in(&s, &pids(), 0.1, true).unwrap();
        d.restore_in(&s, true).unwrap();
        d.lower_in(&s, &pids(), 0.1, false).unwrap();
        assert!(close(s[0].level.get(), 0.1));
    }

    #[test]
    fn leaves_other_apps_alone() {
        let mut d = ducker();
        let s = [session(1.0), FakeSession { key: "music".into(), pid: 7, level: Cell::new(0.1) }];
        d.lower_in(&s, &pids(), 0.1, true).unwrap();
        d.restore_in(&s, true).unwrap();
        assert!(close(s[1].level.get(), 0.1));
    }
}
