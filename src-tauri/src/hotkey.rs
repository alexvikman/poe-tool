//! The price check hotkey: one key press copies the item under the cursor (one
//! Ctrl+Alt+C to the game, one in-game action) and opens the overlay. Ctrl+Alt+C is
//! the game's advanced copy, which adds each mod's roll range and tier.
//!
//! The hotkey is only registered while the game window is in front, so it never takes
//! the key from other apps.

use std::time::{Duration, Instant};

/// Whether Path of Exile's window is the foreground window.
#[cfg(windows)]
pub fn game_is_focused() -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    use windows::core::PWSTR;

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return false;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return false;
        }
        let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return false;
        };
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).is_ok();
        let _ = CloseHandle(process);
        if !ok {
            return false;
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        let name = path.rsplit('\\').next().unwrap_or(&path);
        crate::game::is_game_exe(name)
    }
}

#[cfg(not(windows))]
pub fn game_is_focused() -> bool {
    false
}

/// Sends one Ctrl+Alt+C to the game and returns the clipboard text it put there.
#[cfg(windows)]
pub fn copy_item() -> Result<String, String> {
    use windows::Win32::System::DataExchange::GetClipboardSequenceNumber;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, SendInput,
        VIRTUAL_KEY, VK_C, VK_CONTROL, VK_MENU, VK_SHIFT,
    };

    fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    let before = unsafe { GetClipboardSequenceNumber() };
    // Shift still held from the hotkey would change what the copy does.
    let inputs = [
        key(VK_SHIFT, true),
        key(VK_CONTROL, false),
        key(VK_MENU, false),
        key(VK_C, false),
        key(VK_C, true),
        key(VK_MENU, true),
        key(VK_CONTROL, true),
    ];
    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent as usize != inputs.len() {
        return Err("Couldn't send Ctrl+Alt+C to the game.".into());
    }
    wait_for_clipboard(|| unsafe { GetClipboardSequenceNumber() } != before)
}

#[cfg(not(windows))]
pub fn copy_item() -> Result<String, String> {
    // No game to copy from: use what is on the clipboard (for development).
    wait_for_clipboard(|| true)
}

fn wait_for_clipboard(changed: impl Fn() -> bool) -> Result<String, String> {
    let deadline = Instant::now() + Duration::from_millis(700);
    while !changed() {
        if Instant::now() > deadline {
            return Err("The game didn't copy an item. Hover over an item and try again.".into());
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    // The game may still be writing; a few tries.
    let mut last_err = String::new();
    for _ in 0..10 {
        match arboard::Clipboard::new().and_then(|mut c| c.get_text()) {
            Ok(text) => return Ok(text),
            Err(e) => last_err = e.to_string(),
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Err(format!("Couldn't read the clipboard: {last_err}"))
}
