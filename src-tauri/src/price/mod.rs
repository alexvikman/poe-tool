//! Price check: parse an item, then price it from poe.ninja and the trade site.

pub mod item;
pub mod ninja;
pub mod stats;
pub mod trade;

use item::{Item, Rarity};
use ninja::{NinjaIndex, NinjaPrice};
use serde::{Deserialize, Serialize};
use stats::{ItemsResponse, StatFilter, StatIndex, StatsResponse};
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

    pub async fn check(&self, text: &str, league: &str) -> Result<PriceCheck, String> {
        let item = item::parse(text).ok_or("That doesn't look like a Path of Exile item.")?;
        let stats = self.stat_index().await?;
        let base_type = stats.base_type(&item);

        let (ninja, ninja_error, divine_chaos) = match self.ninja_index(league).await {
            Ok(index) => (index.lookup(&item), None, index.divine_chaos),
            Err(e) => (Vec::new(), Some(e), 0.0),
        };

        let filters = match item.rarity {
            Rarity::Rare | Rarity::Magic | Rarity::Unique if item.identified => stats.filters(&item),
            _ => Vec::new(),
        };
        let options = SearchOptions::for_item(&item);

        // Bulk items have a good poe.ninja price; spend trade requests on the rest.
        let search = match item.rarity {
            Rarity::Rare | Rarity::Magic | Rarity::Normal | Rarity::Unique => true,
            Rarity::Gem => ninja.is_empty(),
            _ => ninja.is_empty(),
        };
        let (trade, trade_error) = if search {
            match self.search(&item, &base_type, &filters, &options, league).await {
                Ok(r) => (Some(r), None),
                Err(e) => (None, Some(e)),
            }
        } else {
            (None, None)
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

    pub async fn search(
        &self,
        item: &Item,
        base_type: &str,
        filters: &[StatFilter],
        options: &SearchOptions,
        league: &str,
    ) -> Result<TradeResult, String> {
        let query = trade::build_query(item, base_type, filters, options);
        self.trade.search(&self.http, league, query).await
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
        let index = Arc::new(StatIndex::new(&stats, &items));
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
