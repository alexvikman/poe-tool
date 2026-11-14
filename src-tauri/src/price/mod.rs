//! Price check: parse an item, then price it from poe.ninja and the trade site.

pub mod category;
pub mod filters;
pub mod item;
pub mod ninja;
pub mod stats;
pub mod trade;

use item::{Item, Rarity};
use ninja::{GemFamily, NinjaIndex, NinjaPrice};
use serde::{Deserialize, Serialize};
use stats::{ItemsResponse, StatFilter, StatIndex, StaticResponse, StatsResponse};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::Mutex;
use trade::{SearchOptions, TradeClient, TradeResult};

/// The trade site's stat and base lists change with patches; a day is fresh enough.
const TRADE_DATA_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);
/// Short, so a league that launches while the tool runs shows up within the hour.
const LEAGUES_MAX_AGE: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceCheck {
    pub item: Item,
    pub base_type: String,
    pub league: String,
    /// Chaos per divine, for showing prices in both.
    pub divine_chaos: f64,
    pub ninja: Vec<NinjaPrice>,
    pub ninja_error: Option<String>,
    pub filters: Vec<StatFilter>,
    pub options: SearchOptions,
    pub trade: Option<TradeResult>,
    pub trade_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GemSearch {
    pub league: String,
    pub divine_chaos: f64,
    pub families: Vec<GemFamily>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct League {
    pub id: String,
}

pub struct PriceService {
    http: reqwest::Client,
    cache_dir: PathBuf,
    stats: Mutex<Option<Arc<StatIndex>>>,
    ninja: Mutex<Option<Arc<NinjaIndex>>>,
    trade: TradeClient,
}

impl PriceService {
    pub fn new(cache_dir: PathBuf) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(concat!("poe-tool/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(15))
            .build()
            .expect("HTTP client");
        PriceService {
            http,
            cache_dir,
            stats: Mutex::new(None),
            ninja: Mutex::new(None),
            trade: TradeClient::new(),
        }
    }

    /// PC trade leagues from the trade site, so a new league shows up without an
    /// update: the challenge league, its Hardcore version, then Standard and Hardcore.
    /// Ruthless leagues are left out. `refresh` skips the hour-long cache.
    pub async fn leagues(&self, refresh: bool) -> Result<Vec<League>, String> {
        #[derive(Deserialize)]
        struct Resp {
            result: Vec<Entry>,
        }
        #[derive(Deserialize)]
        struct Entry {
            id: String,
            realm: String,
        }
        if refresh {
            let _ = std::fs::remove_file(self.cache_dir.join("leagues.json"));
        }
        let bytes = self
            .cached("leagues.json", "/api/trade/data/leagues", LEAGUES_MAX_AGE)
            .await?;
        let resp: Resp = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        let mut leagues: Vec<League> = resp
            .result
            .into_iter()
            .filter(|e| e.realm == "pc" && !e.id.contains("Ruthless"))
            .map(|e| League { id: e.id })
            .collect();
        leagues.sort_by_key(|l| {
            let permanent = matches!(l.id.as_str(), "Standard" | "Hardcore");
            let hardcore = l.id.starts_with("Hardcore") || l.id.starts_with("HC ");
            (permanent, hardcore)
        });
        Ok(leagues)
    }

    pub async fn default_league(&self) -> String {
        self.leagues(false)
            .await
            .ok()
            .and_then(|l| l.into_iter().next())
            .map(|l| l.id)
            .unwrap_or_else(|| "Standard".into())
    }

    /// `instant_buyout`: search only listings that can be bought at once (the default
    /// in settings; the user can change it per search).
    pub async fn check(&self, text: &str, league: &str, instant_buyout: bool) -> Result<PriceCheck, String> {
        let item = item::parse(text).ok_or("That doesn't look like a Path of Exile item.")?;
        let stats = self.stat_index().await?;
        let base_type = stats.base_type(&item);

        let (ninja, ninja_error, divine_chaos) = match self.ninja_index(league).await {
            Ok(index) => (index.lookup(&item), None, index.divine_chaos),
            Err(e) => (Vec::new(), Some(e), 0.0),
        };

        let filters = match item.rarity {
            Rarity::Rare | Rarity::Magic | Rarity::Unique if item.identified => filters::build(&stats, &item),
            _ => Vec::new(),
        };
        let options = SearchOptions::for_item(&item, instant_buyout);

        // Every item gets live listings: bulk items from the currency exchange, the rest
        // from the item search.
        let (trade, trade_error) = match self.search(&item, &base_type, &filters, &options, league).await {
            Ok(r) => (Some(r), None),
            Err(e) => (None, Some(e)),
        };

        Ok(PriceCheck {
            item,
            base_type,
            league: league.to_string(),
            divine_chaos,
            ninja,
            ninja_error,
            filters,
            options,
            trade,
            trade_error,
        })
    }

    /// Transfigured gems by base gem name, for the Divine Font in the Labyrinth.
    pub async fn gem_search(&self, query: &str, league: &str) -> Result<GemSearch, String> {
        let index = self.ninja_index(league).await?;
        Ok(GemSearch {
            league: league.to_string(),
            divine_chaos: index.divine_chaos,
            families: index.gem_families(query),
        })
    }

    pub async fn search(
        &self,
        item: &Item,
        base_type: &str,
        filters: &[StatFilter],
        options: &SearchOptions,
        league: &str,
    ) -> Result<TradeResult, String> {
        let stats = self.stat_index().await?;
        // Bulk items: the currency exchange, paid in chaos or divines.
        if let Some(want) = stats.exchange_id(&item.type_line).filter(|_| {
            !matches!(item.rarity, Rarity::Rare | Rarity::Magic | Rarity::Unique | Rarity::Gem | Rarity::Normal)
        }) {
            let have: Vec<&str> = ["chaos", "divine"].into_iter().filter(|h| *h != want).collect();
            let divine_chaos = self.ninja_index(league).await.map(|i| i.divine_chaos).unwrap_or(0.0);
            return self
                .trade
                .exchange(&self.http, league, &want, &have, divine_chaos, options.instant_buyout)
                .await;
        }
        let query = self.item_query(item, base_type, filters, options, league).await?;
        self.trade.search(&self.http, league, query).await
    }

    /// The item search for these choices, as the trade site takes it. Bulk items
    /// (searched on the currency exchange) have none.
    pub async fn item_query(
        &self,
        item: &Item,
        base_type: &str,
        filters: &[StatFilter],
        options: &SearchOptions,
        league: &str,
    ) -> Result<serde_json::Value, String> {
        let stats = self.stat_index().await?;
        let gem = if item.rarity == Rarity::Gem {
            stats.transfigured(&item.type_line)
        } else {
            None
        };
        Ok(match &gem {
            Some((base, disc)) => trade::build_query(item, base, Some(disc), filters, options, league),
            None => trade::build_query(item, base_type, None, filters, options, league),
        })
    }

    pub fn is_bulk(&self, item: &Item) -> bool {
        !matches!(item.rarity, Rarity::Rare | Rarity::Magic | Rarity::Unique | Rarity::Gem | Rarity::Normal)
    }

    /// The trade site's page for a saved search.
    pub async fn site_url(&self, body: &serde_json::Value, league: &str) -> Result<String, String> {
        let (id, _, _) = self.trade.search_ids(&self.http, league, body, None).await?;
        Ok(trade::site_search_url(league, &id))
    }

    /// A search id for the live search, created signed in.
    pub async fn live_search_id(&self, body: &serde_json::Value, league: &str, session: &str) -> Result<String, String> {
        let (id, _, _) = self.trade.search_ids(&self.http, league, body, Some(session)).await?;
        Ok(id)
    }

    pub async fn fetch_listings(&self, ids: &[String], query_id: &str, session: &str) -> Result<Vec<serde_json::Value>, String> {
        self.trade.fetch_raw(&self.http, ids, query_id, Some(session)).await
    }

    /// Travel to the seller's hideout, or whisper them, as the site's buttons do.
    pub async fn use_listing_token(&self, token: &str, league: &str, session: &str) -> Result<(), String> {
        self.trade.use_listing_token(&self.http, token, league, session).await
    }

    /// The account a session belongs to; an error when the site doesn't accept it.
    pub async fn account_name(&self, session: &str) -> Result<String, String> {
        let resp = self
            .http
            .get(format!("{}/character-window/get-account-name", trade::SITE))
            .header("Cookie", format!("POESESSID={session}"))
            .send()
            .await
            .map_err(|e| format!("Trade site: {e}"))?;
        let body: serde_json::Value = resp.json().await.map_err(|_| "The site didn't accept this POESESSID.".to_string())?;
        body["accountName"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| "The site didn't accept this POESESSID.".to_string())
    }

    async fn stat_index(&self) -> Result<Arc<StatIndex>, String> {
        let mut guard = self.stats.lock().await;
        if let Some(index) = guard.as_ref() {
            return Ok(index.clone());
        }
        let stats = self.cached("stats.json", "/api/trade/data/stats", TRADE_DATA_MAX_AGE).await?;
        let items = self.cached("items.json", "/api/trade/data/items", TRADE_DATA_MAX_AGE).await?;
        let stats: StatsResponse = serde_json::from_slice(&stats).map_err(|e| e.to_string())?;
        let items: ItemsResponse = serde_json::from_slice(&items).map_err(|e| e.to_string())?;
        let statics = self.cached("static.json", "/api/trade/data/static", TRADE_DATA_MAX_AGE).await?;
        let statics: StaticResponse = serde_json::from_slice(&statics).map_err(|e| e.to_string())?;
        let index = Arc::new(StatIndex::new(&stats, &items).with_exchange(&statics));
        *guard = Some(index.clone());
        Ok(index)
    }

    async fn ninja_index(&self, league: &str) -> Result<Arc<NinjaIndex>, String> {
        let mut guard = self.ninja.lock().await;
        if let Some(index) = guard.as_ref() {
            if index.league == league && index.fetched.elapsed() < ninja::MAX_AGE {
                return Ok(index.clone());
            }
        }
        match NinjaIndex::fetch(&self.http, league).await {
            Ok(index) => {
                let index = Arc::new(index);
                *guard = Some(index.clone());
                Ok(index)
            }
            // Old prices beat none.
            Err(e) => guard
                .as_ref()
                .filter(|i| i.league == league)
                .cloned()
                .ok_or(e),
        }
    }

    /// A trade-site data file, from the disk cache when fresh, else downloaded.
    /// Falls back to a stale copy when the site can't be reached.
    async fn cached(&self, file: &str, path: &str, max_age: Duration) -> Result<Vec<u8>, String> {
        let local = self.cache_dir.join(file);
        let fresh = std::fs::metadata(&local)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| SystemTime::now().duration_since(t).ok())
            .is_some_and(|age| age < max_age);
        if fresh {
            if let Ok(bytes) = std::fs::read(&local) {
                return Ok(bytes);
            }
        }
        let download = async {
            let resp = self
                .http
                .get(format!("{}{path}", trade::SITE))
                .send()
                .await
                .and_then(|r| r.error_for_status())
                .map_err(|e| format!("Trade site: {e}"))?;
            resp.bytes().await.map_err(|e| e.to_string())
        };
        match download.await {
            Ok(bytes) => {
                let _ = std::fs::create_dir_all(&self.cache_dir);
                let _ = std::fs::write(&local, &bytes);
                Ok(bytes.to_vec())
            }
            Err(e) => std::fs::read(&local).map_err(|_| e),
        }
    }
}
