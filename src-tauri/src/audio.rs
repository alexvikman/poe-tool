//! Lowers the game's volume in the Windows volume mixer, the same as dragging the
//! game's slider in Volume Mixer by hand. The game itself is never touched.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// Remembers each game audio session's volume from before we lowered it, so it can be
/// put back. It is also kept on disk: Windows remembers an app's mixer volume, so if
/// this tool were to crash while the game is quiet, the next start puts it back.
pub struct Ducker {
    originals: HashMap<String, f32>,
    state_file: PathBuf,
}

impl Ducker {
    pub fn new(state_file: PathBuf) -> Self {
        let originals = std::fs::read(&state_file)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Ducker {
            originals,
            state_file,
        }
    }

    /// Whether some session is (or may still be) lowered by us.
    pub fn has_lowered(&self) -> bool {
        !self.originals.is_empty()
    }

    /// Sets every audio session of `pids` to `factor` of its original volume.
    /// Returns how many game sessions there are.
    pub fn lower(&mut self, pids: &HashSet<u32>, factor: f32) -> Result<usize, String> {
        let sessions = platform::sessions()?;
        let mut count = 0;
        let mut changed = false;
        for s in sessions.iter().filter(|s| pids.contains(&s.pid)) {
            count += 1;
            let current = s.volume()?;
            let original = *self.originals.entry(s.key.clone()).or_insert_with(|| {
                changed = true;
                current
            });
            let target = (original * factor).clamp(0.0, 1.0);
            if (current - target).abs() > 0.005 {
                s.set_volume(target)?;
            }
        }
        if changed {
            self.save();
        }
        Ok(count)
    }

    /// Puts back the original volume of every session we lowered. Sessions of a game
    /// that isn't running stay remembered until it runs again.
    pub fn restore(&mut self) -> Result<(), String> {
        if self.originals.is_empty() {
            return Ok(());
        }
        let sessions = platform::sessions()?;
        let mut restored = Vec::new();
        for s in &sessions {
            if let Some(&original) = self.originals.get(&s.key) {
                s.set_volume(original)?;
                restored.push(s.key.clone());
            }
        }
        if !restored.is_empty() {
            for key in restored {
                self.originals.remove(&key);
            }
            self.save();
        }
        Ok(())
    }

    fn save(&self) {
        if self.originals.is_empty() {
            let _ = std::fs::remove_file(&self.state_file);
        } else if let Ok(json) = serde_json::to_vec(&self.originals) {
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
        .map(|s| Ok((s.pid, s.volume()?)))
        .collect()
}

#[cfg(windows)]
mod platform {
    use std::ffi::c_void;
    use windows::Win32::Media::Audio::{
        eRender, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator,
        ISimpleAudioVolume, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED,
    };
    use windows::core::Interface;

    pub struct Session {
        /// Stable per device and app, so it survives a game restart.
        pub key: String,
        pub pid: u32,
        volume: ISimpleAudioVolume,
    }

    impl Session {
        pub fn volume(&self) -> Result<f32, String> {
            unsafe { self.volume.GetMasterVolume() }.map_err(|e| e.to_string())
        }

        pub fn set_volume(&self, level: f32) -> Result<(), String> {
            unsafe { self.volume.SetMasterVolume(level, std::ptr::null()) }
                .map_err(|e| e.to_string())
        }
    }

    /// Every audio session on every active output device.
    pub fn sessions() -> Result<Vec<Session>, String> {
        unsafe { sessions_inner() }.map_err(|e| format!("Windows audio: {e}"))
    }

    unsafe fn sessions_inner() -> windows::core::Result<Vec<Session>> {
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
                    out.push(Session { key, pid, volume });
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
    pub struct Session {
        pub key: String,
        pub pid: u32,
    }

    impl Session {
        pub fn volume(&self) -> Result<f32, String> {
            Ok(1.0)
        }
        pub fn set_volume(&self, _level: f32) -> Result<(), String> {
            Ok(())
        }
    }

    pub fn sessions() -> Result<Vec<Session>, String> {
        Ok(Vec::new())
    }
}
