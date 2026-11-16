//! The official trade site's search API (pathofexile.com/trade), with its rate limits.

use super::category;
use super::item::{Item, Rarity};
use super::stats::StatFilter;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const SITE: &str = "https://www.pathofexile.com";
/// Listings shown per search, fetched 10 at a time (the site's limit per request).
const SHOW_COUNT: usize = 20;
const FETCH_BATCH: usize = 10;

/// Search choices besides the stat filters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchOptions {
    /// None: either.
    pub corrupted: Option<bool>,
    /// Only listings that can be bought at once from a merchant tab.
    pub instant_buyout: bool,
    /// Search the item's category (Two-Handed Sword) instead of its base type.
    #[serde(default)]
    pub category: Option<Toggle<String>>,
    /// Minimum item level.
    #[serde(default)]
    pub item_level: Option<Toggle<u32>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Toggle<T> {
    pub value: T,
    /// What the user sees ("Two-Handed Sword").
    #[serde(default)]
    pub label: String,
    pub enabled: bool,
}

impl SearchOptions {
    pub fn for_item(item: &Item, instant_buyout: bool) -> Self {
        let gear = matches!(item.rarity, Rarity::Normal | Rarity::Magic | Rarity::Rare);
        let cluster = item.type_line.contains("Cluster Jewel");
        SearchOptions {
            // Gear and gems are priced by whether they are corrupted.
            corrupted: (gear || matches!(item.rarity, Rarity::Unique | Rarity::Gem)).then_some(item.corrupted),
            instant_buyout,
            // Rares sell by what they are, not by their exact base. Viscera can be
            // widened to every part of their kind, but each part does its own thing.
            category: if item.item_class == "Viscera" {
                let (id, label) = category::viscera_category(item.slot.as_deref());
                Some(Toggle { value: id.to_string(), label: label.to_string(), enabled: false })
            } else {
                category::trade_category(&item.item_class)
                    .filter(|_| gear && !cluster)
                    .map(|(id, label)| Toggle { value: id.to_string(), label: label.to_string(), enabled: true })
            },
            item_level: item
                .item_level
                .filter(|_| gear && item.item_class != "Jewels")
                .map(|ilvl| Toggle { value: ilvl.min(86), label: String::new(), enabled: false }),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradeResult {
    /// From the currency exchange (bulk items): prices are per item, with stock.
    pub exchange: bool,
    pub total: u32,
    pub url: String,
    pub listings: Vec<Listing>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    pub amount: f64,
    pub currency: String,
    pub indexed: String,
    pub account: String,
    pub item_level: Option<u32>,
    pub corrupted: bool,
    /// Exchange listings: how many the seller has.
    pub stock: Option<u32>,
}

/// Follows the limits the site sends in `X-Rate-Limit-Ip` (`max:period:penalty,…`),
/// keeping one request of headroom, and waits out a 429.
struct RateLimiter {
    rules: Vec<(usize, Duration)>,
    history: VecDeque<Instant>,
    blocked_until: Option<Instant>,
}

impl RateLimiter {
    fn new(default_rules: Vec<(usize, Duration)>) -> Self {
        RateLimiter {
            rules: default_rules,
            history: VecDeque::new(),
            blocked_until: None,
        }
    }

    /// How long until a request may go, or None if it may go now (and is counted).
    fn acquire(&mut self) -> Option<Duration> {
        let now = Instant::now();
        if let Some(until) = self.blocked_until {
            if until > now {
                return Some(until - now);
            }
            self.blocked_until = None;
        }
        let longest = self.rules.iter().map(|r| r.1).max().unwrap_or_default();
        while self.history.front().is_some_and(|t| now - *t > longest) {
            self.history.pop_front();
        }
        for &(max, period) in &self.rules {
            let in_window: Vec<&Instant> =
                self.history.iter().filter(|t| now - **t < period).collect();
            if in_window.len() + 1 >= max {
                let oldest = in_window.first().map(|t| **t).unwrap_or(now);
                return Some(period.saturating_sub(now - oldest));
            }
        }
        self.history.push_back(now);
        None
    }

    fn update(&mut self, headers: &reqwest::header::HeaderMap) {
        if let Some(rules) = headers
            .get("x-rate-limit-ip")
            .and_then(|v| v.to_str().ok())
            .map(parse_rules)
            .filter(|r| !r.is_empty())
        {
            self.rules = rules;
        }
        if let Some(secs) = headers
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
        {
            self.blocked_until = Some(Instant::now() + Duration::from_secs(secs));
        }
    }
}

fn parse_rules(header: &str) -> Vec<(usize, Duration)> {
    header
        .split(',')
        .filter_map(|rule| {
            let mut parts = rule.split(':');
            let max = parts.next()?.parse().ok()?;
            let period = parts.next()?.parse().ok()?;
            Some((max, Duration::from_secs(period)))
        })
        .collect()
}

pub struct TradeClient {
    search_limit: Mutex<RateLimiter>,
    fetch_limit: Mutex<RateLimiter>,
    exchange_limit: Mutex<RateLimiter>,
}

impl TradeClient {
    pub fn new() -> Self {
        TradeClient {
            search_limit: Mutex::new(RateLimiter::new(vec![(5, Duration::from_secs(10))])),
            fetch_limit: Mutex::new(RateLimiter::new(vec![(12, Duration::from_secs(4))])),
            exchange_limit: Mutex::new(RateLimiter::new(vec![(5, Duration::from_secs(15))])),
        }
    }

    /// Offers on the currency exchange for one bulk item (`want`, an exchange id such
    /// as "the-doctor"), paid in any of `have` ("chaos", "divine"), cheapest first.
    /// `divine_chaos` (chaos per divine) ranks offers in different currencies.
    pub async fn exchange(
        &self,
        http: &reqwest::Client,
        league: &str,
        want: &str,
        have: &[&str],
        divine_chaos: f64,
        instant_buyout: bool,
    ) -> Result<TradeResult, String> {
        if let Some(wait) = self.exchange_limit.lock().unwrap().acquire() {
            return Err(rate_limited(wait));
        }
        let status = if instant_buyout { "securable" } else { "online" };
        let query = json!({
            "query": { "status": { "option": status }, "have": have, "want": [want] },
            "sort": { "have": "asc" },
            "engine": "new",
        });
        let url = format!("{SITE}/api/trade/exchange/{}", urlencode(league));
        let resp = http.post(url).json(&query).send().await.map_err(net_error)?;
        self.exchange_limit.lock().unwrap().update(resp.headers());
        let resp = check_status(resp).await?;
        let found: ExchangeResponse = resp.json().await.map_err(net_error)?;
        // An object of listings by id, or an empty list when there are none.
        let entries: Vec<ExchangeResultEntry> = match found.result {
            Value::Object(map) => map
                .into_iter()
                .filter_map(|(_, v)| serde_json::from_value(v).ok())
                .collect(),
            _ => Vec::new(),
        };
        let mut listings: Vec<Listing> = entries
            .into_iter()
            .filter_map(|r| {
                let offer = r.listing.offers.into_iter().next()?;
                if offer.item.amount <= 0.0 {
                    return None;
                }
                Some(Listing {
                    amount: offer.exchange.amount / offer.item.amount,
                    currency: offer.exchange.currency,
                    indexed: r.listing.indexed,
                    account: r.listing.account.name,
                    item_level: None,
                    corrupted: false,
                    stock: offer.item.stock,
                })
            })
            .collect();
        let in_chaos = |l: &Listing| match l.currency.as_str() {
            "divine" if divine_chaos > 0.0 => l.amount * divine_chaos,
            _ => l.amount,
        };
        listings.sort_by(|a, b| in_chaos(a).total_cmp(&in_chaos(b)));
        listings.truncate(SHOW_COUNT);
        Ok(TradeResult {
            exchange: true,
            total: found.total,
            url: format!("{SITE}/trade/exchange/{}/{}", urlencode(league), found.id),
            listings,
        })
    }

    /// Runs a search as the trade site's own page does, signed in when `session` (a
    /// POESESSID) is given. Returns the search id, the total and the result ids.
    pub async fn search_ids(
        &self,
        http: &reqwest::Client,
        league: &str,
        body: &Value,
        session: Option<&str>,
    ) -> Result<(String, u32, Vec<String>), String> {
        if let Some(wait) = self.search_limit.lock().unwrap().acquire() {
            return Err(rate_limited(wait));
        }
        let url = format!("{SITE}/api/trade/search/{}", urlencode(league));
        let resp = signed_in(http.post(url), session).json(body).send().await.map_err(net_error)?;
        self.search_limit.lock().unwrap().update(resp.headers());
        let resp = check_status(resp).await?;
        let found: SearchResponse = resp.json().await.map_err(net_error)?;
        Ok((found.id, found.total, found.result))
    }

    /// Listings by id, as the site sends them (signed in, they carry the tokens for
    /// whispering and travelling to the seller's hideout).
    pub async fn fetch_raw(
        &self,
        http: &reqwest::Client,
        ids: &[String],
        query_id: &str,
        session: Option<&str>,
    ) -> Result<Vec<Value>, String> {
        let mut out = Vec::new();
        for batch in ids.chunks(FETCH_BATCH) {
            if let Some(wait) = self.fetch_limit.lock().unwrap().acquire() {
                return Err(rate_limited(wait));
            }
            let url = format!("{SITE}/api/trade/fetch/{}", batch.join(","));
            let resp = signed_in(http.get(url), session)
                .query(&[("query", query_id)])
                .send()
                .await
                .map_err(net_error)?;
            self.fetch_limit.lock().unwrap().update(resp.headers());
            let resp = check_status(resp).await?;
            let body: Value = resp.json().await.map_err(net_error)?;
            if let Some(results) = body["result"].as_array() {
                out.extend(results.iter().filter(|r| !r.is_null()).cloned());
            }
        }
        Ok(out)
    }

    /// What the site's "Travel to Hideout" and "Whisper" buttons do: hands the token
    /// to the game server, which acts in the game for the signed-in account.
    pub async fn use_listing_token(
        &self,
        http: &reqwest::Client,
        token: &str,
        league: &str,
        session: &str,
    ) -> Result<(), String> {
        let resp = signed_in(http.post(format!("{SITE}/api/trade/whisper")), Some(session))
            .header("Referer", format!("{SITE}/trade/search/{}", urlencode(league)))
            .header("X-Requested-With", "XMLHttpRequest")
            .header("Accept", "application/json")
            .json(&json!({ "token": token }))
            .send()
            .await
            .map_err(net_error)?;
        let resp = check_status(resp).await?;
        let body: Value = resp.json().await.unwrap_or(Value::Null);
        if body["error"].is_object() {
            return Err(body["error"]["message"].as_str().unwrap_or("The trade site said no.").to_string());
        }
        Ok(())
    }

    pub async fn search(
        &self,
        http: &reqwest::Client,
        league: &str,
        query: Value,
    ) -> Result<TradeResult, String> {
        if let Some(wait) = self.search_limit.lock().unwrap().acquire() {
            return Err(rate_limited(wait));
        }
        let url = format!("{SITE}/api/trade/search/{}", urlencode(league));
        let resp = http.post(url).json(&query).send().await.map_err(net_error)?;
        self.search_limit.lock().unwrap().update(resp.headers());
        let resp = check_status(resp).await?;
        let found: SearchResponse = resp.json().await.map_err(net_error)?;
        let site_url = format!("{SITE}/trade/search/{}/{}", urlencode(league), found.id);

        let mut listings = Vec::new();
        for batch in found.result.iter().take(SHOW_COUNT).collect::<Vec<_>>().chunks(FETCH_BATCH) {
            if let Some(wait) = self.fetch_limit.lock().unwrap().acquire() {
                if listings.is_empty() {
                    return Err(rate_limited(wait));
                }
                break; // Show what we have rather than nothing.
            }
            let ids: Vec<&str> = batch.iter().map(|s| s.as_str()).collect();
            let url = format!("{SITE}/api/trade/fetch/{}", ids.join(","));
            let resp = http
                .get(url)
                .query(&[("query", &found.id)])
                .send()
                .await
                .map_err(net_error)?;
            self.fetch_limit.lock().unwrap().update(resp.headers());
            let resp = check_status(resp).await?;
            let fetched: FetchResponse = resp.json().await.map_err(net_error)?;
            listings.extend(fetched.result.into_iter().flatten().filter_map(|r| {
                let price = r.listing.price?;
                Some(Listing {
                    amount: price.amount,
                    currency: price.currency,
                    indexed: r.listing.indexed,
                    account: r.listing.account.name,
                    item_level: r.item.ilvl,
                    corrupted: r.item.corrupted.unwrap_or(false),
                    stock: None,
                })
            }));
        }
        Ok(TradeResult {
            exchange: false,
            total: found.total,
            url: site_url,
            listings,
        })
    }
}

fn rate_limited(wait: Duration) -> String {
    format!(
        "The trade site's rate limit is close. Try again in {} s.",
        wait.as_secs().max(1)
    )
}

/// Adds the site's session cookie and origin, as its own pages send them.
fn signed_in(req: reqwest::RequestBuilder, session: Option<&str>) -> reqwest::RequestBuilder {
    match session {
        Some(s) => req.header("Cookie", format!("POESESSID={s}")).header("Origin", SITE),
        None => req,
    }
}

pub fn site_search_url(league: &str, id: &str) -> String {
    format!("{SITE}/trade/search/{}/{}", urlencode(league), id)
}

pub fn urlencode_league(league: &str) -> String {
    urlencode(league)
}

fn net_error(e: reqwest::Error) -> String {
    format!("Trade site: {e}")
}

async fn check_status(resp: reqwest::Response) -> Result<reqwest::Response, String> {
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    if status.as_u16() == 429 {
        return Err("The trade site says too many requests. Wait a minute.".into());
    }
    if status.as_u16() == 401 || status.as_u16() == 403 {
        return Err("The trade site didn't accept the session. Check your POESESSID under Searches.".into());
    }
    let body: Value = resp.json().await.unwrap_or(Value::Null);
    let message = body["error"]["message"].as_str().unwrap_or("unknown error");
    Err(format!("Trade site ({status}): {message}"))
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[derive(Deserialize)]
struct SearchResponse {
    id: String,
    total: u32,
    result: Vec<String>,
}

#[derive(Deserialize)]
struct ExchangeResponse {
    id: String,
    total: u32,
    #[serde(default)]
    result: Value,
}

#[derive(Deserialize)]
struct ExchangeResultEntry {
    listing: ExchangeListing,
}

#[derive(Deserialize)]
struct ExchangeListing {
    indexed: String,
    account: FetchAccount,
    offers: Vec<ExchangeOffer>,
}

#[derive(Deserialize)]
struct ExchangeOffer {
    exchange: ExchangeSide,
    item: ExchangeSide,
}

#[derive(Deserialize)]
struct ExchangeSide {
    currency: String,
    amount: f64,
    stock: Option<u32>,
}

#[derive(Deserialize)]
struct FetchResponse {
    result: Vec<Option<FetchResult>>,
}

#[derive(Deserialize)]
struct FetchResult {
    listing: FetchListing,
    item: FetchItem,
}

#[derive(Deserialize)]
struct FetchListing {
    indexed: String,
    account: FetchAccount,
    price: Option<FetchPrice>,
}

#[derive(Deserialize)]
struct FetchAccount {
    name: String,
}

#[derive(Deserialize)]
struct FetchPrice {
    amount: f64,
    currency: String,
}

#[derive(Deserialize)]
struct FetchItem {
    ilvl: Option<u32>,
    corrupted: Option<bool>,
}

/// The search for items like this one. `filters` are the stat rows the user left on.
/// `discriminator`: for a transfigured gem, which version of `base_type` (the trade
/// site lists "Cold Snap of Power" as Cold Snap, alt_x).
pub fn build_query(
    item: &Item,
    base_type: &str,
    discriminator: Option<&str>,
    filters: &[StatFilter],
    options: &SearchOptions,
    league: &str,
) -> Value {
    let status = if options.instant_buyout { "securable" } else { "available" };
    let mut query = json!({
        "status": { "option": status },
    });
    let mut misc = serde_json::Map::new();
    match item.rarity {
        Rarity::Unique => {
            // Foulborn (the site calls them mutated) and Vestigial uniques are their own
            // versions on the site.
            let name = item.name.as_deref().unwrap_or_default();
            let foulborn = name.strip_prefix("Foulborn ");
            let vestigial = base_type.strip_prefix("Vestigial ");
            query["name"] = json!(foulborn.unwrap_or(name));
            query["type"] = json!(vestigial.unwrap_or(base_type));
            misc.insert("mutated".into(), json!({ "option": foulborn.is_some() }));
            if vestigial.is_none() {
                misc.insert("vestigial".into(), json!({ "option": false }));
            }
        }
        Rarity::Gem => {
            query["type"] = match discriminator {
                Some(d) => json!({ "option": base_type, "discriminator": d }),
                None => json!(item.type_line),
            };
            if let Some(level) = item.gem_level {
                misc.insert("gem_level".into(), json!({ "min": level }));
            }
            if let Some(q) = item.quality {
                misc.insert("quality".into(), json!({ "min": q }));
            }
        }
        Rarity::Rare | Rarity::Magic | Rarity::Normal => {
            query["filters"]["type_filters"] = json!({ "filters": { "rarity": { "option": "nonunique" } } });
            match options.category.as_ref().filter(|c| c.enabled) {
                Some(c) => query["filters"]["type_filters"]["filters"]["category"] = json!({ "option": c.value }),
                None => query["type"] = json!(base_type),
            }
            if let Some(ilvl) = options.item_level.as_ref().filter(|l| l.enabled) {
                misc.insert("ilvl".into(), json!({ "min": ilvl.value }));
            }
            // An item that can still be crafted on is compared with others like it:
            // not fractured, mirrored or split (split only matters outside Standard).
            if !item.corrupted && !item.mirrored {
                if !item.fractured {
                    misc.insert("fractured_item".into(), json!({ "option": false }));
                }
                misc.insert("mirrored".into(), json!({ "option": false }));
                let plain = !item.synthesised && !item.fractured && item.influences.is_empty();
                if plain && league != "Standard" {
                    misc.insert("split".into(), json!({ "option": false }));
                }
            }
        }
        _ => match options.category.as_ref().filter(|c| c.enabled) {
            Some(c) => query["filters"]["type_filters"] = json!({ "filters": { "category": { "option": c.value } } }),
            None => query["type"] = json!(item.type_line),
        },
    }
    if item.links >= 5 {
        query["filters"]["socket_filters"] = json!({ "filters": { "links": { "min": item.links } } });
    }
    if let Some(c) = options.corrupted {
        misc.insert("corrupted".into(), json!({ "option": c }));
    }
    if !misc.is_empty() {
        query["filters"]["misc_filters"] = json!({ "filters": misc });
    }
    // Weapon and armour properties have their own filter groups; the rest are stats.
    for f in filters.iter().filter(|f| f.enabled) {
        let group = match f.id.split_once('.') {
            Some(("weapon", key)) => Some(("weapon_filters", key)),
            Some(("armour", key)) => Some(("armour_filters", key)),
            _ => None,
        };
        if let Some((group, key)) = group {
            let mut range = serde_json::Map::new();
            if let Some(min) = f.min {
                range.insert("min".into(), json!(min));
            }
            if let Some(max) = f.max {
                range.insert("max".into(), json!(max));
            }
            query["filters"][group]["filters"][key] = Value::Object(range);
        }
    }
    // Every stat goes along, the ones not ticked as disabled: the search ignores them,
    // and the trade site shows them to tick there. Hidden fixed rolls go without a
    // value; a unique's base implicit, the same on every copy, stays out.
    let stats: Vec<Value> = filters
        .iter()
        .filter(|f| !f.id.is_empty() && !f.id.starts_with("weapon.") && !f.id.starts_with("armour."))
        .filter(|f| !(item.rarity == Rarity::Unique && f.hidden && f.kind == "implicit"))
        .map(|f| {
            let mut value = serde_json::Map::new();
            if !f.hidden || f.enabled {
                if let Some(min) = f.min {
                    value.insert("min".into(), json!(min));
                }
                if let Some(max) = f.max {
                    value.insert("max".into(), json!(max));
                }
            }
            let mut stat = json!({ "id": f.id });
            if !value.is_empty() {
                stat["value"] = Value::Object(value);
            }
            if !f.enabled {
                stat["disabled"] = json!(true);
            }
            stat
        })
        .collect();
    query["stats"] = json!([{ "type": "and", "filters": stats }]);
    json!({ "query": query, "sort": { "price": "asc" } })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rate_limit_rules() {
        assert_eq!(
            parse_rules("5:10:60,15:60:300"),
            vec![(5, Duration::from_secs(10)), (15, Duration::from_secs(60))]
        );
    }

    #[test]
    fn limiter_keeps_headroom() {
        let mut l = RateLimiter::new(vec![(3, Duration::from_secs(10))]);
        assert!(l.acquire().is_none());
        assert!(l.acquire().is_none());
        assert!(l.acquire().is_some());
    }

    #[test]
    fn query_for_a_viscera_searches_its_type_or_its_part() {
        let item = super::super::item::parse(
            "Item Class: Viscera\nInquisitor's Head\n--------\nHead\n--------\nItem Level: 64",
        )
        .unwrap();
        let mut options = SearchOptions::for_item(&item, true);
        let q = build_query(&item, "Inquisitor's Head", None, &[], &options, "Covens");
        assert_eq!(q["query"]["type"], "Inquisitor's Head");
        assert!(q["query"]["filters"]["type_filters"].is_null());

        options.category.as_mut().unwrap().enabled = true;
        let q = build_query(&item, "Inquisitor's Head", None, &[], &options, "Covens");
        assert!(q["query"]["type"].is_null());
        assert_eq!(q["query"]["filters"]["type_filters"]["filters"]["category"]["option"], "viscera.head");
    }

    #[test]
    fn query_for_a_rare_has_base_and_enabled_stats() {
        let item = super::super::item::parse(
            "Item Class: Boots\nRarity: Rare\nGrim League\nTwo-Toned Boots\n--------\nItem Level: 80",
        )
        .unwrap();
        let filters = vec![
            StatFilter {
                id: "pseudo.pseudo_total_life".into(),
                text: String::new(),
                value: Some(100.0),
                min: Some(85.0),
                max: None,
                enabled: true,
                kind: "pseudo".into(),
                roll: None,
                tier: None,
                hidden: false,
            },
            StatFilter {
                id: "explicit.stat_1".into(),
                text: String::new(),
                value: None,
                min: None,
                max: Some(10.0),
                enabled: false,
                kind: "explicit".into(),
                roll: None,
                tier: None,
                hidden: false,
            },
            StatFilter {
                id: "weapon.dps".into(),
                text: String::new(),
                value: Some(258.0),
                min: Some(237.0),
                max: None,
                enabled: true,
                kind: "property".into(),
                roll: None,
                tier: None,
                hidden: false,
            },
        ];
        let q = build_query(&item, "Two-Toned Boots", None, &filters, &SearchOptions::for_item(&item, false), "Covens");
        assert_eq!(q["query"]["status"]["option"], "available");
        // Rares search their category, not their base, and not corrupted ones.
        assert!(q["query"]["type"].is_null());
        assert_eq!(q["query"]["filters"]["type_filters"]["filters"]["category"]["option"], "armour.boots");
        assert_eq!(q["query"]["filters"]["misc_filters"]["filters"]["corrupted"]["option"], false);
        assert!(q["query"]["filters"]["misc_filters"]["filters"]["ilvl"].is_null());
        assert_eq!(q["query"]["filters"]["misc_filters"]["filters"]["fractured_item"]["option"], false);
        assert_eq!(q["query"]["filters"]["misc_filters"]["filters"]["mirrored"]["option"], false);
        assert_eq!(q["query"]["filters"]["misc_filters"]["filters"]["split"]["option"], false);
        assert_eq!(q["query"]["filters"]["weapon_filters"]["filters"]["dps"]["min"], 237.0);
        assert_eq!(q["query"]["filters"]["type_filters"]["filters"]["rarity"]["option"], "nonunique");
        assert_eq!(
            q["query"]["stats"][0]["filters"],
            json!([
                { "id": "pseudo.pseudo_total_life", "value": { "min": 85.0 } },
                { "id": "explicit.stat_1", "value": { "max": 10.0 }, "disabled": true }
            ])
        );
    }

    #[test]
    fn query_for_a_unique_has_corrupted_and_buyout_options() {
        let item = super::super::item::parse(
            "Item Class: Belts\nRarity: Unique\nBelt of the Deceiver\nHeavy Belt\n--------\nItem Level: 84",
        )
        .unwrap();
        let mut options = SearchOptions::for_item(&item, false);
        assert_eq!(options.corrupted, Some(false));
        options.instant_buyout = true;
        let q = build_query(&item, "Heavy Belt", None, &[], &options, "Covens");
        assert_eq!(q["query"]["name"], "Belt of the Deceiver");
        assert_eq!(q["query"]["status"]["option"], "securable");
        assert_eq!(q["query"]["filters"]["misc_filters"]["filters"]["corrupted"]["option"], false);
        assert_eq!(q["query"]["filters"]["misc_filters"]["filters"]["mutated"]["option"], false);
    }

    #[test]
    fn query_for_a_foulborn_unique_uses_its_plain_name() {
        let item = super::super::item::parse(
            "Item Class: Belts\nRarity: Unique\nFoulborn Belt of the Deceiver\nHeavy Belt\n--------\nItem Level: 84",
        )
        .unwrap();
        let q = build_query(&item, "Heavy Belt", None, &[], &SearchOptions::for_item(&item, false), "Covens");
        assert_eq!(q["query"]["name"], "Belt of the Deceiver");
        assert_eq!(q["query"]["filters"]["misc_filters"]["filters"]["mutated"]["option"], true);
    }

    #[test]
    fn query_for_a_transfigured_gem_uses_the_discriminator() {
        let item = super::super::item::parse(
            "Item Class: Skill Gems\nRarity: Gem\nCold Snap of Power\n--------\nLevel: 1\nQuality: +18% (augmented)",
        )
        .unwrap();
        let q = build_query(&item, "Cold Snap", Some("alt_x"), &[], &SearchOptions::for_item(&item, false), "Covens");
        assert_eq!(q["query"]["type"], json!({ "option": "Cold Snap", "discriminator": "alt_x" }));
        assert_eq!(q["query"]["filters"]["misc_filters"]["filters"]["gem_level"]["min"], 1);
        assert_eq!(q["query"]["filters"]["misc_filters"]["filters"]["quality"]["min"], 18);
    }

    #[test]
    fn urlencodes_league_names() {
        assert_eq!(urlencode("Hardcore Covens"), "Hardcore%20Covens");
    }
}
