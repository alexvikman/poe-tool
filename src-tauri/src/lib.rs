mod areas;
pub mod audio;
mod client_log;
mod game;
mod hotkey;
mod live;
pub mod price;
mod searches;
mod session;
mod settings;
mod town_mute;

use audio::Ducker;
use price::item::Item;
use price::stats::StatFilter;
use price::trade::{SearchOptions, TradeResult};
use price::{GemSearch, League, PriceCheck, PriceService};
use serde::Serialize;
use settings::{HotkeyAction, Settings};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, RunEvent, State, WebviewWindow, WindowEvent,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use live::{Live, LiveHit, LiveStatus};
use searches::{SearchGroup, SearchStore};
use town_mute::{Status, TownMute};

const PRICE_WINDOW: &str = "price";

struct AppState {
    settings: Arc<Mutex<Settings>>,
    settings_path: PathBuf,
    status: Arc<Mutex<Status>>,
    ducker: Arc<Mutex<Ducker>>,
    price: Arc<PriceService>,
    searches: Arc<Mutex<SearchStore>>,
    live: Arc<Live>,
}

impl AppState {
    /// Watches exactly the searches marked live, with the stored session.
    fn sync_live(&self) {
        let watched = self.searches.lock().unwrap().live_searches();
        self.live.sync(watched, session::get());
    }
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

/// Each hotkey in use, parsed. Refuses keys that aren't keys and keys used twice.
fn parse_hotkeys(settings: &Settings) -> Result<Vec<(Shortcut, HotkeyAction)>, String> {
    let mut out: Vec<(Shortcut, HotkeyAction)> = Vec::new();
    for (text, action) in settings.hotkeys() {
        let shortcut: Shortcut = text
            .parse()
            .map_err(|_| format!("\"{text}\" isn't a key the tool can listen for."))?;
        if out.iter().any(|(s, _)| *s == shortcut) {
            return Err(format!("{text} is used twice."));
        }
        out.push((shortcut, action));
    }
    Ok(out)
}

#[tauri::command]
fn set_settings(state: State<AppState>, settings: Settings) -> Result<(), String> {
    parse_hotkeys(&settings)?;
    settings.save(&state.settings_path)?;
    *state.settings.lock().unwrap() = settings;
    Ok(())
}

/// Called right before installing an update, which closes the app without the usual
/// exit: put the game's volume back first.
#[tauri::command]
fn prepare_update(state: State<AppState>) {
    let _ = state.ducker.lock().unwrap().restore(true);
}

#[tauri::command]
fn get_status(state: State<AppState>) -> Status {
    state.status.lock().unwrap().clone()
}

#[tauri::command]
async fn get_leagues(state: State<'_, AppState>, refresh: Option<bool>) -> Result<Vec<League>, String> {
    state.price.leagues(refresh.unwrap_or(false)).await
}

async fn league(state: &AppState) -> String {
    let chosen = state.settings.lock().unwrap().price_check.league.clone();
    match chosen {
        Some(l) if !l.is_empty() => l,
        _ => state.price.default_league().await,
    }
}

#[tauri::command]
async fn price_check(state: State<'_, AppState>, text: String) -> Result<PriceCheck, String> {
    let league = league(&state).await;
    let instant_buyout = state.settings.lock().unwrap().price_check.instant_buyout;
    state.price.check(&text, &league, instant_buyout).await
}

#[tauri::command]
async fn gem_search(state: State<'_, AppState>, query: String) -> Result<GemSearch, String> {
    let league = league(&state).await;
    state.price.gem_search(&query, &league).await
}

#[tauri::command]
async fn price_search(
    state: State<'_, AppState>,
    item: Item,
    base_type: String,
    filters: Vec<StatFilter>,
    options: SearchOptions,
    league: String,
) -> Result<TradeResult, String> {
    state.price.search(&item, &base_type, &filters, &options, &league).await
}

#[tauri::command]
fn searches_list(state: State<AppState>) -> Vec<SearchGroup> {
    state.searches.lock().unwrap().groups.clone()
}

#[tauri::command]
fn searches_add_group(state: State<AppState>, name: String) -> Result<SearchGroup, String> {
    state.searches.lock().unwrap().add_group(&name)
}

#[tauri::command]
fn searches_rename_group(state: State<AppState>, id: String, name: String) -> Result<(), String> {
    let mut store = state.searches.lock().unwrap();
    store.group_mut(&id)?.name = name.trim().to_string();
    store.save()?;
    drop(store);
    state.sync_live();
    Ok(())
}

#[tauri::command]
fn searches_delete_group(state: State<AppState>, id: String) -> Result<(), String> {
    let mut store = state.searches.lock().unwrap();
    store.groups.retain(|g| g.id != id);
    store.save()?;
    drop(store);
    state.sync_live();
    Ok(())
}

#[tauri::command]
fn searches_add_link(state: State<AppState>, group_id: String, link: String, name: String) -> Result<(), String> {
    let search = searches::from_link(&link, &name)?;
    state.searches.lock().unwrap().add_search(&group_id, search)?;
    Ok(())
}

/// Saves the search a price check made, with the stats ticked as they are.
#[tauri::command]
async fn searches_save_check(
    state: State<'_, AppState>,
    group_id: String,
    name: String,
    item: Item,
    base_type: String,
    filters: Vec<StatFilter>,
    options: SearchOptions,
    league: String,
) -> Result<(), String> {
    if state.price.is_bulk(&item) {
        return Err("Currency and other bulk items can't be saved as searches yet.".into());
    }
    let query = state.price.item_query(&item, &base_type, &filters, &options, &league).await?;
    let search = searches::SavedSearch { id: searches::new_id(), name, league, body: query, live: false };
    state.searches.lock().unwrap().add_search(&group_id, search)?;
    Ok(())
}

#[tauri::command]
fn searches_update(state: State<AppState>, id: String, name: Option<String>, live: Option<bool>) -> Result<(), String> {
    let mut store = state.searches.lock().unwrap();
    let search = store.search_mut(&id)?;
    if let Some(name) = name {
        search.name = name.trim().to_string();
    }
    if let Some(live) = live {
        search.live = live;
    }
    store.save()?;
    drop(store);
    state.sync_live();
    Ok(())
}

#[tauri::command]
fn searches_delete(state: State<AppState>, id: String) -> Result<(), String> {
    let mut store = state.searches.lock().unwrap();
    for g in store.groups.iter_mut() {
        g.searches.retain(|s| s.id != id);
    }
    store.save()?;
    drop(store);
    state.sync_live();
    Ok(())
}

/// The trade site's page for a saved search.
#[tauri::command]
async fn searches_open(state: State<'_, AppState>, id: String) -> Result<String, String> {
    let (body, league) = {
        let store = state.searches.lock().unwrap();
        let (_, s) = store.find(&id).ok_or("That search is gone.")?;
        (s.body.clone(), s.league.clone())
    };
    state.price.site_url(&body, &league).await
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionStatus {
    set: bool,
    account: Option<String>,
    error: Option<String>,
}

#[tauri::command]
async fn session_status(state: State<'_, AppState>) -> Result<SessionStatus, String> {
    let Some(s) = session::get() else {
        return Ok(SessionStatus { set: false, account: None, error: None });
    };
    Ok(match state.price.account_name(&s).await {
        Ok(account) => SessionStatus { set: true, account: Some(account), error: None },
        Err(e) => SessionStatus { set: true, account: None, error: Some(e) },
    })
}

#[tauri::command]
async fn session_set(state: State<'_, AppState>, value: String) -> Result<SessionStatus, String> {
    session::set(&value)?;
    state.sync_live();
    session_status(state).await
}

#[tauri::command]
fn session_clear(state: State<AppState>) -> Result<(), String> {
    session::clear()?;
    state.sync_live();
    Ok(())
}

#[tauri::command]
fn live_state(state: State<AppState>) -> (std::collections::HashMap<String, LiveStatus>, Vec<LiveHit>) {
    (state.live.statuses(), state.live.hits())
}

/// Travel to the seller's hideout (instant buyout) or whisper them, as the trade site's
/// buttons do.
#[tauri::command]
async fn live_use_token(state: State<'_, AppState>, token: String, league: String) -> Result<(), String> {
    let s = session::get().ok_or("Add your POESESSID first.")?;
    state.price.use_listing_token(&token, &league, &s).await
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PriceRequest {
    text: Option<String>,
    error: Option<String>,
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Puts the overlay beside the cursor, inside the monitor the cursor is on.
fn place_beside_cursor(window: &WebviewWindow) {
    let Ok(cursor) = window.cursor_position() else { return };
    let Ok(size) = window.outer_size() else { return };
    let monitor = window
        .monitor_from_point(cursor.x, cursor.y)
        .ok()
        .flatten();
    let (mx, my, mw, mh) = match &monitor {
        Some(m) => {
            let a = m.work_area();
            (a.position.x as f64, a.position.y as f64, a.size.width as f64, a.size.height as f64)
        }
        None => (0.0, 0.0, f64::MAX, f64::MAX),
    };
    let (w, h) = (size.width as f64, size.height as f64);
    let gap = 24.0;
    // Right of the cursor when there is room, else left of it.
    let mut x = cursor.x + gap;
    if x + w > mx + mw {
        x = cursor.x - gap - w;
    }
    let x = x.clamp(mx, (mx + mw - w).max(mx));
    let y = (cursor.y - h / 3.0).clamp(my, (my + mh - h).max(my));
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

fn on_price_hotkey(app: AppHandle) {
    // Not on the hotkey thread: copying waits for the game.
    std::thread::spawn(move || {
        let request = match hotkey::copy_item() {
            Ok(text) => PriceRequest { text: Some(text), error: None },
            Err(e) => PriceRequest { text: None, error: Some(e) },
        };
        let Some(window) = app.get_webview_window(PRICE_WINDOW) else { return };
        place_beside_cursor(&window);
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.emit("price-request", request);
    });
}

fn on_chat_hotkey(text: String, send: bool) {
    // Not on the hotkey thread: typing waits a little between keys.
    std::thread::spawn(move || {
        if let Err(e) = hotkey::send_chat(&text, send) {
            eprintln!("chat command {text}: {e}");
        }
    });
}

/// Registers the hotkeys (price check, chat commands) while the game is in front, and
/// only then, so other apps keep their keys.
fn watch_hotkeys(app: AppHandle, settings: Arc<Mutex<Settings>>) {
    let mut registered: HashMap<Shortcut, HotkeyAction> = HashMap::new();
    loop {
        let wanted: HashMap<Shortcut, HotkeyAction> = if hotkey::game_is_focused() {
            parse_hotkeys(&settings.lock().unwrap())
                .unwrap_or_default()
                .into_iter()
                .collect()
        } else {
            HashMap::new()
        };
        if wanted != registered {
            let gs = app.global_shortcut();
            for (shortcut, action) in registered.clone() {
                if wanted.get(&shortcut) != Some(&action) {
                    let _ = gs.unregister(shortcut);
                    registered.remove(&shortcut);
                }
            }
            for (shortcut, action) in wanted {
                if registered.contains_key(&shortcut) {
                    continue;
                }
                let handle = app.clone();
                let on_press = action.clone();
                let ok = gs
                    .on_shortcut(shortcut, move |_app, _shortcut, event| {
                        if event.state != ShortcutState::Pressed {
                            return;
                        }
                        match &on_press {
                            HotkeyAction::PriceCheck => on_price_hotkey(handle.clone()),
                            HotkeyAction::Chat { text, send } => on_chat_hotkey(text.clone(), *send),
                        }
                    })
                    .is_ok();
                if ok {
                    registered.insert(shortcut, action);
                }
            }
        }
        std::thread::sleep(Duration::from_millis(150));
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let cache_dir = app.path().app_cache_dir()?;
            let settings_path = config_dir.join("settings.json");
            let settings = Arc::new(Mutex::new(Settings::load(&settings_path)));
            let ducker = Arc::new(Mutex::new(Ducker::new(config_dir.join("lowered-volumes.json"))));
            let status = Arc::new(Mutex::new(Status::default()));
            let price = Arc::new(PriceService::new(cache_dir));
            // First, before anything slow: the windows already exist and their pages
            // call the commands as soon as they load.
            let searches = Arc::new(Mutex::new(SearchStore::load(config_dir.join("searches.json"))));
            let live = Live::new(app.handle().clone(), price.clone());
            app.manage(AppState {
                settings: settings.clone(),
                settings_path,
                status: status.clone(),
                ducker: ducker.clone(),
                price: price.clone(),
                searches,
                live,
            });
            app.state::<AppState>().sync_live();

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

            let handle = app.handle().clone();
            let hotkey_settings = settings.clone();
            std::thread::Builder::new()
                .name("hotkeys".into())
                .spawn(move || watch_hotkeys(handle, hotkey_settings))?;

            // Warm the caches so the first price check is quick.
            let warm = price.clone();
            tauri::async_runtime::spawn(async move {
                let league = warm.default_league().await;
                let _ = warm.check("Item Class: Stackable Currency\nRarity: Currency\nChaos Orb", &league, true).await;
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
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { api, .. } => {
                let _ = window.hide();
                api.prevent_close();
            }
            // The overlay goes away when you click back into the game.
            WindowEvent::Focused(false) if window.label() == PRICE_WINDOW => {
                let _ = window.hide();
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            set_settings,
            get_status,
            prepare_update,
            get_leagues,
            price_check,
            price_search,
            gem_search,
            searches_list,
            searches_add_group,
            searches_rename_group,
            searches_delete_group,
            searches_add_link,
            searches_save_check,
            searches_update,
            searches_delete,
            searches_open,
            session_status,
            session_set,
            session_clear,
            live_state,
            live_use_token
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app, event| {
        if let RunEvent::Exit = event {
            // Never leave the game quiet after we quit.
            if let Some(state) = app.try_state::<AppState>() {
                let _ = state.ducker.lock().unwrap().restore(true);
            }
        }
    });
}
