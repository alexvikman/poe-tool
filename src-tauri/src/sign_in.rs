//! Signing in on pathofexile.com in a window of the app, so the session doesn't have
//! to be copied out of a browser by hand. The window shows the site's own login page
//! and has no access to the app (no capability names it). The app never sees what's
//! typed there: it only reads the site's POESESSID cookie, and keeps it once the site
//! says that session is signed in.

use crate::price::PriceService;
use crate::session;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, Url, WebviewUrl, WebviewWindowBuilder, WindowEvent};

const LABEL: &str = "sign-in";
const SITE: &str = "https://www.pathofexile.com";

/// Opens the sign-in window and waits until it signs in (the account's name) or is
/// closed.
pub async fn sign_in(app: AppHandle, price: Arc<PriceService>) -> Result<String, String> {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.set_focus();
        return Err("The sign-in window is already open.".into());
    }
    let site: Url = SITE.parse().unwrap();
    // Asking a window that's closing for anything can hang, so its events say what
    // page it's on and when it goes.
    let page = Arc::new(Mutex::new(String::new()));
    let closed = Arc::new(AtomicBool::new(false));
    let window = {
        let page = page.clone();
        WebviewWindowBuilder::new(&app, LABEL, WebviewUrl::External(site.join("/login").unwrap()))
            .title("Sign in to pathofexile.com")
            .inner_size(560.0, 780.0)
            .center()
            .on_navigation(move |url| {
                *page.lock().unwrap() = url.to_string();
                true
            })
            .build()
            .map_err(|e| e.to_string())?
    };
    {
        let closed = closed.clone();
        window.on_window_event(move |e| {
            if matches!(e, WindowEvent::CloseRequested { .. } | WindowEvent::Destroyed) {
                closed.store(true, Ordering::SeqCst);
            }
        });
    }

    // Visitors get a POESESSID too, so a cookie alone says nothing: ask the site each
    // time the cookie or the page changes (signing in ends on a new page).
    let mut checked: Option<(String, String)> = None;
    let deadline = Instant::now() + Duration::from_secs(20 * 60);
    while Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(1500)).await;
        if closed.load(Ordering::SeqCst) {
            return Err("Sign-in was closed.".into());
        }
        let Some(value) = site_session(&window, &site).await else { continue };
        let now = (value.clone(), page.lock().unwrap().clone());
        if checked.as_ref() == Some(&now) {
            continue;
        }
        checked = Some(now);
        if let Ok(account) = price.account_name(&value).await {
            session::set(&value)?;
            let _ = window.close();
            return Ok(account);
        }
    }
    let _ = window.close();
    Err("Sign-in took too long. Try again.".into())
}

/// Forgets the site's cookies in the app's browser storage, so the next sign-in
/// starts signed out (and can be another account).
pub async fn forget(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else { return };
    let site: Url = SITE.parse().unwrap();
    // WebView2 deadlocks if asked for cookies on the main thread.
    let _ = tauri::async_runtime::spawn_blocking(move || {
        for cookie in window.cookies_for_url(site).unwrap_or_default() {
            let _ = window.delete_cookie(cookie);
        }
    })
    .await;
}

async fn site_session(window: &tauri::WebviewWindow, site: &Url) -> Option<String> {
    let (window, site) = (window.clone(), site.clone());
    // A closing window can leave the request unanswered: don't wait for it forever.
    let read = tauri::async_runtime::spawn_blocking(move || window.cookies_for_url(site));
    let cookies = tokio::time::timeout(Duration::from_secs(5), read).await;
    let cookies = cookies.ok()?.ok()?.ok()?;
    cookies
        .iter()
        .find(|c| c.name() == "POESESSID")
        .map(|c| c.value().to_string())
        .filter(|v| !v.is_empty())
}
