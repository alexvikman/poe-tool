//! Live searches: the trade site's websocket, which sends the ids of new listings for a
//! search as they appear. It needs the user's session (POESESSID).
//!
//! One task per watched search: it creates the search id signed in, connects, fetches
//! each new listing (signed in, so it carries the tokens to travel or whisper), and
//! reconnects with a growing pause when the connection drops.

use crate::price::trade::{urlencode_league, SITE};
use crate::price::PriceService;
use crate::searches::SavedSearch;
use futures::StreamExt;
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::{self, Message};

/// The most recent hits kept for the feed.
const KEEP_HITS: usize = 100;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveHit {
    pub search_id: String,
    pub search_name: String,
    pub group: String,
    pub league: String,
    /// When it showed up here (RFC 3339).
    pub seen: String,
    pub item_name: String,
    pub type_line: String,
    pub amount: Option<f64>,
    pub currency: Option<String>,
    pub account: String,
    /// Instant buyout listings: travel to the seller's hideout to buy.
    pub hideout_token: Option<String>,
    /// Other listings: whisper the seller from the game.
    pub whisper_token: Option<String>,
    pub whisper: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", tag = "state", content = "message")]
pub enum LiveStatus {
    Connecting,
    Live,
    /// Waiting to try again.
    Retrying(String),
    /// Stopped until the search or the session changes.
    Failed(String),
}

enum Stop {
    /// Try again after a pause.
    Retry(String),
    /// Don't: the session or the search is the problem.
    Fail(String),
}

struct Task {
    /// What it was started with; a change restarts it.
    fingerprint: String,
    handle: tauri::async_runtime::JoinHandle<()>,
}

pub struct Live {
    app: AppHandle,
    price: Arc<PriceService>,
    tasks: Mutex<HashMap<String, Task>>,
    status: Mutex<HashMap<String, LiveStatus>>,
    hits: Mutex<VecDeque<LiveHit>>,
}

impl Live {
    pub fn new(app: AppHandle, price: Arc<PriceService>) -> Arc<Self> {
        Arc::new(Live {
            app,
            price,
            tasks: Mutex::new(HashMap::new()),
            status: Mutex::new(HashMap::new()),
            hits: Mutex::new(VecDeque::new()),
        })
    }

    pub fn statuses(&self) -> HashMap<String, LiveStatus> {
        self.status.lock().unwrap().clone()
    }

    pub fn hits(&self) -> Vec<LiveHit> {
        self.hits.lock().unwrap().iter().cloned().collect()
    }

    /// Starts and stops watching so exactly these searches are watched.
    pub fn sync(self: &Arc<Self>, searches: Vec<(String, SavedSearch)>, session: Option<String>) {
        let mut tasks = self.tasks.lock().unwrap();
        let mut wanted: HashMap<String, (String, String, SavedSearch)> = HashMap::new();
        for (group, s) in searches {
            let fingerprint = format!("{}|{}|{}|{}", s.league, s.body, session.as_deref().unwrap_or(""), s.name);
            wanted.insert(s.id.clone(), (fingerprint, group, s));
        }
        tasks.retain(|id, task| {
            let keep = wanted.get(id).is_some_and(|(f, _, _)| *f == task.fingerprint);
            if !keep {
                task.handle.abort();
                self.status.lock().unwrap().remove(id);
            }
            keep
        });
        for (id, (fingerprint, group, search)) in wanted {
            if tasks.contains_key(&id) {
                continue;
            }
            let Some(session) = session.clone() else {
                self.set_status(&id, LiveStatus::Failed("Add your POESESSID to watch searches live.".into()));
                continue;
            };
            let me = self.clone();
            let handle = tauri::async_runtime::spawn(async move { me.watch(group, search, session).await });
            tasks.insert(id, Task { fingerprint, handle });
        }
        let _ = self.app.emit("live-status", self.statuses());
    }

    fn set_status(&self, id: &str, status: LiveStatus) {
        self.status.lock().unwrap().insert(id.to_string(), status);
        let _ = self.app.emit("live-status", self.statuses());
    }

    async fn watch(self: Arc<Self>, group: String, search: SavedSearch, session: String) {
        let mut pause = Duration::from_secs(5);
        loop {
            self.set_status(&search.id, LiveStatus::Connecting);
            match self.connect(&group, &search, &session).await {
                Ok(()) => pause = Duration::from_secs(5),
                Err(Stop::Fail(msg)) => {
                    self.set_status(&search.id, LiveStatus::Failed(msg));
                    return;
                }
                Err(Stop::Retry(msg)) => {
                    self.set_status(&search.id, LiveStatus::Retrying(msg));
                }
            }
            tokio::time::sleep(pause).await;
            pause = (pause * 2).min(Duration::from_secs(120));
        }
    }

    async fn connect(&self, group: &str, search: &SavedSearch, session: &str) -> Result<(), Stop> {
        let query_id = self
            .price
            .live_search_id(&search.body, &search.league, session)
            .await
            .map_err(|e| if e.contains("POESESSID") { Stop::Fail(e) } else { Stop::Retry(e) })?;

        let mut socket = open_socket(&search.league, &query_id, session).await?;
        self.set_status(&search.id, LiveStatus::Live);

        while let Some(msg) = socket.next().await {
            let text = match msg {
                Ok(Message::Text(t)) => t.to_string(),
                Ok(Message::Close(_)) => return Err(Stop::Retry("The trade site closed the connection.".into())),
                Ok(_) => continue,
                Err(e) => return Err(Stop::Retry(format!("Connection lost: {e}"))),
            };
            let Ok(event) = serde_json::from_str::<Value>(&text) else { continue };
            if event["auth"] == Value::Bool(false) {
                return Err(Stop::Fail("The trade site didn't accept the session. Check your POESESSID.".into()));
            }
            let ids = new_listings(&event);
            if ids.is_empty() {
                continue;
            }
            match self.price.fetch_listings(&ids, &query_id, session).await {
                Ok(listings) => {
                    // Still connected: clear a failed fetch's message.
                    if self.status.lock().unwrap().get(&search.id) != Some(&LiveStatus::Live) {
                        self.set_status(&search.id, LiveStatus::Live);
                    }
                    for l in listings {
                        self.hit(group, search, &l);
                    }
                }
                // The listing is lost, but the feed carries on.
                Err(e) => self.set_status(&search.id, LiveStatus::Retrying(format!("Missed a listing: {e}"))),
            }
        }
        Err(Stop::Retry("Connection lost.".into()))
    }

    fn hit(&self, group: &str, search: &SavedSearch, raw: &Value) {
        let listing = &raw["listing"];
        let item = &raw["item"];
        let s = |v: &Value| v.as_str().filter(|s| !s.is_empty()).map(str::to_string);
        let hit = LiveHit {
            search_id: search.id.clone(),
            search_name: search.name.clone(),
            group: group.to_string(),
            league: search.league.clone(),
            seen: now_rfc3339(),
            item_name: s(&item["name"]).unwrap_or_default(),
            type_line: s(&item["typeLine"]).unwrap_or_default(),
            amount: listing["price"]["amount"].as_f64(),
            currency: s(&listing["price"]["currency"]),
            account: s(&listing["account"]["name"]).unwrap_or_default(),
            hideout_token: s(&listing["hideout_token"]),
            whisper_token: s(&listing["whisper_token"]),
            whisper: s(&listing["whisper"]),
        };
        {
            let mut hits = self.hits.lock().unwrap();
            hits.push_front(hit.clone());
            hits.truncate(KEEP_HITS);
        }
        let _ = self.app.emit("live-hit", &hit);

        let title = if hit.item_name.is_empty() { hit.type_line.clone() } else { hit.item_name.clone() };
        let price = match (hit.amount, &hit.currency) {
            (Some(a), Some(c)) => format!("{a} {c}"),
            _ => "no price".into(),
        };
        let _ = self
            .app
            .notification()
            .builder()
            .title(format!("Live: {title}"))
            .body(format!("{price} · {} ({})", hit.search_name, hit.group))
            .show();
    }
}

type Socket = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Connects to a search's live feed, signed in.
async fn open_socket(league: &str, query_id: &str, session: &str) -> Result<Socket, Stop> {
    let url = format!("wss://www.pathofexile.com/api/trade/live/{}/{}", urlencode_league(league), query_id);
    let mut request = url.into_client_request().map_err(|e| Stop::Fail(e.to_string()))?;
    let headers = request.headers_mut();
    let header = |v: String| v.parse().map_err(|_| Stop::Fail("Bad session value.".into()));
    headers.insert("Cookie", header(format!("POESESSID={session}"))?);
    headers.insert("Origin", header(SITE.to_string())?);
    headers.insert("User-Agent", header(concat!("poe-tool/", env!("CARGO_PKG_VERSION")).to_string())?);

    let (socket, _) = tokio_tungstenite::connect_async(request).await.map_err(|e| match e {
        tungstenite::Error::Http(resp) if matches!(resp.status().as_u16(), 401 | 403) => {
            Stop::Fail("The trade site didn't accept the session. Check your POESESSID.".into())
        }
        e => Stop::Retry(format!("Couldn't connect: {e}")),
    })?;
    Ok(socket)
}

/// What a message says is new. The site sends one signed token per new listing
/// (`{"result": "<token>"}`), which the fetch endpoint takes in place of an id; older
/// messages carried a list of ids (`{"new": [...]}`).
fn new_listings(event: &Value) -> Vec<String> {
    if let Some(token) = event["result"].as_str().filter(|s| !s.is_empty()) {
        return vec![token.to_string()];
    }
    event["new"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

fn now_rfc3339() -> String {
    // Seconds since the epoch, formatted by hand to keep a date crate out.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

/// Days since 1970-01-01 → (year, month, day), Howard Hinnant's algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_771), (2026, 11, 14));
    }

    /// Against the real site: `POESESSID=… LEAGUE=… cargo test live_feed -- --ignored --nocapture`.
    /// Watches every new pair of boots and fetches the first one that shows up.
    #[tokio::test]
    #[ignore]
    async fn live_feed() {
        let session = std::env::var("POESESSID").expect("POESESSID");
        let league = std::env::var("LEAGUE").expect("LEAGUE");
        let price = PriceService::new(std::env::temp_dir().join("poe-tool-live-test"));
        println!("account: {:?}", price.account_name(&session).await);
        let body = serde_json::json!({"query": {"status": {"option": "online"},
            "filters": {"type_filters": {"filters": {"category": {"option": "armour.boots"}}}}}});
        let query_id = price.live_search_id(&body, &league, &session).await.unwrap();
        let mut socket = open_socket(&league, &query_id, &session).await.ok().expect("connect");
        let ids = tokio::time::timeout(Duration::from_secs(90), async {
            while let Some(Ok(msg)) = socket.next().await {
                if let Message::Text(t) = msg {
                    let event: Value = serde_json::from_str(&t).unwrap();
                    println!("message: {}", &t[..t.len().min(60)]);
                    let ids = new_listings(&event);
                    if !ids.is_empty() {
                        return ids;
                    }
                }
            }
            panic!("socket closed")
        })
        .await
        .expect("a new listing within 90 s");
        let listings = price.fetch_listings(&ids, &query_id, &session).await.unwrap();
        let l = &listings[0]["listing"];
        println!(
            "{} · {} {} · hideout token: {} · whisper token: {}",
            listings[0]["item"]["typeLine"], l["price"]["amount"], l["price"]["currency"],
            l["hideout_token"].is_string(), l["whisper_token"].is_string()
        );
        assert!(l["hideout_token"].is_string() || l["whisper_token"].is_string() || l["whisper"].is_string());
    }

    #[test]
    fn reads_new_listings() {
        let v = |s: &str| serde_json::from_str::<Value>(s).unwrap();
        assert_eq!(new_listings(&v(r#"{"result":"a.b.c"}"#)), vec!["a.b.c"]);
        assert_eq!(new_listings(&v(r#"{"new":["x","y"]}"#)), vec!["x", "y"]);
        assert!(new_listings(&v(r#"{"auth":true}"#)).is_empty());
    }
}
