mod areas;
pub mod audio;
mod client_log;
mod game;
mod settings;
mod town_mute;

use audio::Ducker;
use settings::Settings;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WindowEvent};
use town_mute::{Status, TownMute};

struct AppState {
    settings: Arc<Mutex<Settings>>,
    settings_path: PathBuf,
    status: Arc<Mutex<Status>>,
    ducker: Arc<Mutex<Ducker>>,
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn set_settings(state: State<AppState>, settings: Settings) -> Result<(), String> {
    settings.save(&state.settings_path)?;
    *state.settings.lock().unwrap() = settings;
    Ok(())
}

#[tauri::command]
fn get_status(state: State<AppState>) -> Status {
    state.status.lock().unwrap().clone()
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let settings_path = config_dir.join("settings.json");
            let settings = Arc::new(Mutex::new(Settings::load(&settings_path)));
            let ducker = Arc::new(Mutex::new(Ducker::new(config_dir.join("lowered-volumes.json"))));
            let status = Arc::new(Mutex::new(Status::default()));

            let monitor = TownMute::new(settings.clone(), ducker.clone());
            let handle = app.handle().clone();
            let shared_status = status.clone();
            std::thread::Builder::new()
                .name("town-mute".into())
                .spawn(move || {
                    monitor.run(|s| {
                        *shared_status.lock().unwrap() = s.clone();
                        let _ = handle.emit("town-mute-status", s);
                    })
                })?;

            app.manage(AppState {
                settings,
                settings_path,
                status,
                ducker,
            });

            // Keep running in the tray so the mute keeps working with the window closed.
            let show = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("PoE Tool")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_main(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main(tray.app_handle());
                    }
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![get_settings, set_settings, get_status])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app, event| {
        if let RunEvent::Exit = event {
            // Never leave the game quiet after we quit.
            if let Some(state) = app.try_state::<AppState>() {
                let _ = state.ducker.lock().unwrap().restore();
            }
        }
    });
}
