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

#[cfg(windows)]
mod keys {
    pub use windows::Win32::UI::Input::KeyboardAndMouse::{
        VK_A, VK_C, VK_CONTROL, VK_MENU, VK_RETURN, VK_SHIFT, VK_V,
    };
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, SendInput,
        VIRTUAL_KEY,
    };

    /// A key press (`up: false`) or release.
    pub fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
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

    pub fn send(inputs: &[INPUT]) -> Result<(), String> {
        let sent = unsafe { SendInput(inputs, std::mem::size_of::<INPUT>() as i32) };
        if sent as usize == inputs.len() {
            Ok(())
        } else {
            Err("Couldn't send keys to the game.".into())
        }
    }

    /// Releases modifiers still held from the hotkey, which would change what our keys do.
    pub fn release_modifiers() -> [INPUT; 3] {
        [key(VK_SHIFT, true), key(VK_CONTROL, true), key(VK_MENU, true)]
    }
}

/// Sends one Ctrl+Alt+C to the game and returns the clipboard text it put there.
#[cfg(windows)]
pub fn copy_item() -> Result<String, String> {
    use keys::*;
    use windows::Win32::System::DataExchange::GetClipboardSequenceNumber;

    let before = unsafe { GetClipboardSequenceNumber() };
    send(&release_modifiers())?;
    send(&[
        key(VK_CONTROL, false),
        key(VK_MENU, false),
        key(VK_C, false),
        key(VK_C, true),
        key(VK_MENU, true),
        key(VK_CONTROL, true),
    ])
    .map_err(|_| "Couldn't send Ctrl+Alt+C to the game.".to_string())?;
    wait_for_clipboard(|| unsafe { GetClipboardSequenceNumber() } != before)
}

/// Types a chat command in the game: Enter opens the chat box, Ctrl+A and Ctrl+V
/// replace whatever is in it with `text` (through the clipboard, which is put back
/// afterwards), and Enter sends it when `send` is set. One key press, one command.
#[cfg(windows)]
pub fn send_chat(text: &str, send_it: bool) -> Result<(), String> {
    use keys::*;
    use std::thread::sleep;

    let mut clipboard = arboard::Clipboard::new().map_err(|e| format!("Clipboard: {e}"))?;
    let previous = clipboard.get_text().ok();
    clipboard
        .set_text(text.to_string())
        .map_err(|e| format!("Clipboard: {e}"))?;

    let result = (|| {
        send(&release_modifiers())?;
        send(&[key(VK_RETURN, false), key(VK_RETURN, true)])?;
        // Give the chat box a moment to open.
        sleep(Duration::from_millis(40));
        send(&[
            key(VK_CONTROL, false),
            key(VK_A, false),
            key(VK_A, true),
            key(VK_V, false),
            key(VK_V, true),
            key(VK_CONTROL, true),
        ])?;
        if send_it {
            sleep(Duration::from_millis(40));
            send(&[key(VK_RETURN, false), key(VK_RETURN, true)])?;
        }
        Ok::<(), String>(())
    })();

    // Let the game read the clipboard before it is put back: key presses are handled
    // whenever the game gets to them, so this needs a good margin.
    sleep(Duration::from_millis(1000));
    if let Some(previous) = previous {
        let _ = clipboard.set_text(previous);
    }
    result
}

#[cfg(not(windows))]
pub fn send_chat(_text: &str, _send: bool) -> Result<(), String> {
    Err("Chat commands only work on Windows.".into())
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
