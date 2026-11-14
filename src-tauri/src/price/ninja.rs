//! Prices from poe.ninja for items that have one market price: currency, cards,
//! fragments, uniques, gems and so on.

use super::item::{Item, Rarity};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

const BASE: &str = "https://poe.ninja/poe1/api/economy";
/// poe.ninja updates about every 15 minutes.
pub const MAX_AGE: Duration = Duration::from_secs(15 * 60);

/// Bulk items, priced by poe.ninja from the currency exchange.
const EXCHANGE_TYPES: &[&str] = &[
    "Currency", "Fragment", "DivinationCard", "Scarab", "Oil", "Essence", "Fossil",
    "Resonator", "DeliriumOrb", "Artifact", "Omen", "Tattoo", "AllflameEmber", "Runegraft",
];

/// Items priced from stash listings.
const ITEM_TYPES: &[&str] = &[
    "UniqueWeapon", "UniqueArmour", "UniqueAccessory", "UniqueFlask", "UniqueJewel",
    "UniqueMap", "UniqueRelic", "UniqueTincture", "SkillGem", "Map", "Invitation", "Beast",
    "Wombgift",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NinjaPrice {
    pub name: String,
    pub base_type: Option<String>,
    pub variant: Option<String>,
    pub links: Option<u32>,
    pub gem_level: Option<u32>,
    pub gem_quality: Option<u32>,
    pub corrupted: Option<bool>,
    /// Price of one, in chaos.
    pub chaos: f64,
    pub listing_count: Option<u32>,
}

pub struct NinjaIndex {
    pub league: String,
    pub fetched: Instant,
    /// Chaos per divine.
    pub divine_chaos: f64,
    by_name: HashMap<String, Vec<NinjaPrice>>,
}

#[derive(Deserialize)]
struct ExchangeOverview {
    core: ExchangeCore,
    #[serde(default)]
    lines: Vec<ExchangeLine>,
    #[serde(default)]
    items: Vec<ExchangeItem>,
}

#[derive(Deserialize)]
struct ExchangeCore {
    rates: HashMap<String, f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExchangeLine {
    id: String,
    primary_value: f64,
}

#[derive(Deserialize)]
struct ExchangeItem {
    id: String,
    name: String,
}

#[derive(Deserialize)]
struct ItemOverview {
    #[serde(default)]
    lines: Vec<ItemLine>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItemLine {
    name: String,
    base_type: Option<String>,
    variant: Option<String>,
    links: Option<u32>,
    gem_level: Option<u32>,
    gem_quality: Option<u32>,
    corrupted: Option<bool>,
    chaos_value: f64,
    listing_count: Option<u32>,
}

impl NinjaIndex {
    pub async fn fetch(http: &reqwest::Client, league: &str) -> Result<Self, String> {
        let exchange = futures::future::join_all(EXCHANGE_TYPES.iter().map(|t| {
            let url = format!("{BASE}/exchange/current/overview");
            http.get(url).query(&[("league", league), ("type", t)]).send()
        }));
        let items = futures::future::join_all(ITEM_TYPES.iter().map(|t| {
            let url = format!("{BASE}/stash/current/item/overview");
            http.get(url).query(&[("league", league), ("type", t)]).send()
        }));
        let (exchange, items) = futures::join!(exchange, items);

        let mut by_name: HashMap<String, Vec<NinjaPrice>> = HashMap::new();
        let mut divine_chaos = None;
        let mut errors = 0;
        for resp in exchange {
            let Ok(resp) = resp.and_then(|r| r.error_for_status()) else {
                errors += 1;
                continue;
            };
            let Ok(ov) = resp.json::<ExchangeOverview>().await else {
                errors += 1;
                continue;
            };
            if let Some(rate) = ov.core.rates.get("divine").filter(|r| **r > 0.0) {
                divine_chaos.get_or_insert(1.0 / rate);
            }
            let names: HashMap<&str, &str> =
                ov.items.iter().map(|i| (i.id.as_str(), i.name.as_str())).collect();
            for line in &ov.lines {
                let Some(name) = names.get(line.id.as_str()) else { continue };
                by_name.entry(name.to_lowercase()).or_default().push(NinjaPrice {
                    name: name.to_string(),
                    base_type: None,
                    variant: None,
                    links: None,
                    gem_level: None,
                    gem_quality: None,
                    corrupted: None,
                    chaos: line.primary_value,
                    listing_count: None,
                });
            }
        }
        for resp in items {
            let Ok(resp) = resp.and_then(|r| r.error_for_status()) else {
                errors += 1;
                continue;
            };
            let Ok(ov) = resp.json::<ItemOverview>().await else {
                errors += 1;
                continue;
            };
            for l in ov.lines {
                by_name.entry(l.name.to_lowercase()).or_default().push(NinjaPrice {
                    name: l.name,
                    base_type: l.base_type,
                    variant: l.variant,
                    links: l.links,
                    gem_level: l.gem_level,
                    gem_quality: l.gem_quality,
                    corrupted: l.corrupted,
                    chaos: l.chaos_value,
                    listing_count: l.listing_count,
                });
            }
        }
        if by_name.is_empty() {
            return Err(format!("poe.ninja: no prices for {league} ({errors} requests failed)"));
        }
        // Chaos itself is the unit and isn't listed.
        by_name.entry("chaos orb".into()).or_insert_with(|| {
            vec![NinjaPrice {
                name: "Chaos Orb".into(),
                base_type: None,
                variant: None,
                links: None,
                gem_level: None,
                gem_quality: None,
                corrupted: None,
                chaos: 1.0,
                listing_count: None,
            }]
        });
        Ok(NinjaIndex {
            league: league.to_string(),
            fetched: Instant::now(),
            divine_chaos: divine_chaos.unwrap_or(0.0),
            by_name,
        })
    }

    /// The prices that fit this item, best match first. Empty when poe.ninja doesn't
    /// price items like it (rares, for example).
    pub fn lookup(&self, item: &Item) -> Vec<NinjaPrice> {
        let key = match item.rarity {
            Rarity::Unique => item.name.clone().unwrap_or_default(),
            Rarity::Rare | Rarity::Magic => return Vec::new(),
            _ => item.type_line.clone(),
        };
        let Some(prices) = self.by_name.get(&key.to_lowercase()) else {
            return Vec::new();
        };
        let mut prices = prices.clone();
        match item.rarity {
            Rarity::Unique => {
                prices.retain(|p| {
                    p.base_type.as_deref().is_none_or(|b| item.type_line.ends_with(b))
                });
                // 5- and 6-link variants are priced separately.
                let wanted = (item.links >= 5).then_some(item.links);
                if prices.iter().any(|p| p.links == wanted) {
                    prices.retain(|p| p.links == wanted);
                }
            }
            Rarity::Gem => {
                let level = item.gem_level.unwrap_or(1) as i64;
                let quality = item.quality.unwrap_or(0) as i64;
                prices.sort_by_key(|p| {
                    let corrupted_mismatch = (p.corrupted.unwrap_or(false) != item.corrupted) as i64;
                    corrupted_mismatch * 1000
                        + (p.gem_level.unwrap_or(1) as i64 - level).abs() * 10
                        + (p.gem_quality.unwrap_or(0) as i64 - quality).abs()
                });
                prices.truncate(3);
            }
            _ => {}
        }
        prices
    }
}
