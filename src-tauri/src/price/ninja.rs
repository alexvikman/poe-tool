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
    /// Price change over the last 7 days, in percent.
    pub change_7d: Option<f64>,
}

pub struct NinjaIndex {
    pub league: String,
    pub fetched: Instant,
    /// Chaos per divine.
    pub divine_chaos: f64,
    by_name: HashMap<String, Vec<NinjaPrice>>,
    /// Skill gems by their base gem (a transfigured gem's base is the gem it came from).
    gems_by_base: HashMap<String, Vec<GemPrice>>,
}

#[derive(Debug, Clone)]
struct GemPrice {
    name: String,
    transfigured: bool,
    variant: String,
    corrupted: bool,
    chaos: f64,
    listing_count: Option<u32>,
}

/// A base gem and the gems the Divine Font can turn it into, for choosing one in the
/// Labyrinth, where the game doesn't let you copy them.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GemFamily {
    pub base: String,
    /// Transfigured gems first, most valuable first; the base gem last.
    pub gems: Vec<GemOption>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GemOption {
    pub name: String,
    pub transfigured: bool,
    /// Uncorrupted versions, such as "1", "1/20", "20/20", lowest level first.
    pub prices: Vec<GemVariantPrice>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GemVariantPrice {
    pub variant: String,
    pub chaos: f64,
    pub listing_count: Option<u32>,
}

fn family(gems: &[GemPrice]) -> GemFamily {
    let mut names: Vec<(&str, bool)> = Vec::new();
    for g in gems {
        if !names.iter().any(|(n, _)| *n == g.name) {
            names.push((&g.name, g.transfigured));
        }
    }
    let base = gems
        .iter()
        .find(|g| !g.transfigured)
        .map(|g| g.name.clone())
        .unwrap_or_else(|| {
            // Only transfigured gems listed: "X of Y" → "X".
            gems[0].name.split(" of ").next().unwrap_or(&gems[0].name).to_string()
        });
    let mut options: Vec<GemOption> = names
        .into_iter()
        .map(|(name, transfigured)| {
            let mut prices: Vec<GemVariantPrice> = gems
                .iter()
                .filter(|g| g.name == name && !g.corrupted)
                .map(|g| GemVariantPrice {
                    variant: g.variant.clone(),
                    chaos: g.chaos,
                    listing_count: g.listing_count,
                })
                .collect();
            prices.sort_by_key(|p| variant_order(&p.variant));
            GemOption { name: name.to_string(), transfigured, prices }
        })
        .collect();
    let best = |o: &GemOption| o.prices.iter().map(|p| p.chaos).fold(0.0, f64::max);
    options.sort_by(|a, b| {
        a.transfigured
            .cmp(&b.transfigured)
            .reverse()
            .then(best(b).total_cmp(&best(a)))
    });
    GemFamily { base, gems: options }
}

/// "1" < "1/20" < "20" < "20/20" < "21/20": by level, then quality.
fn variant_order(v: &str) -> (u32, u32) {
    let mut parts = v.trim_end_matches('c').split('/');
    let level = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let quality = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    (level, quality)
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
    sparkline: Option<Sparkline>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Sparkline {
    total_change: Option<f64>,
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
    spark_line: Option<Sparkline>,
    /// For gems: `{"query": {"type": {"option": "<base gem>", "discriminator": …}}}`.
    trade_filter: Option<serde_json::Value>,
    metadata: Option<serde_json::Value>,
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
        let mut gems_by_base: HashMap<String, Vec<GemPrice>> = HashMap::new();
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
                    change_7d: line.sparkline.as_ref().and_then(|s| s.total_change),
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
                if l.gem_level.is_some() {
                    let base = l
                        .trade_filter
                        .as_ref()
                        .and_then(|f| f["query"]["type"]["option"].as_str())
                        .unwrap_or(&l.name)
                        .to_string();
                    let kind = l.metadata.as_ref().and_then(|m| m["gemKind"].as_str());
                    gems_by_base.entry(base.to_lowercase()).or_default().push(GemPrice {
                        name: l.name.clone(),
                        transfigured: matches!(kind, Some("transfigured" | "trarthus")),
                        variant: l.variant.clone().unwrap_or_default(),
                        corrupted: l.corrupted.unwrap_or(false),
                        chaos: l.chaos_value,
                        listing_count: l.listing_count,
                    });
                }
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
                    change_7d: l.spark_line.as_ref().and_then(|s| s.total_change),
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
                change_7d: None,
            }]
        });
        Ok(NinjaIndex {
            league: league.to_string(),
            fetched: Instant::now(),
            divine_chaos: divine_chaos.unwrap_or(0.0),
            by_name,
            gems_by_base,
        })
    }

    /// Gem families whose base gem or one of whose transfigured gems matches `query`
    /// (any part of the name, any case). Only bases that have transfigured gems.
    pub fn gem_families(&self, query: &str) -> Vec<GemFamily> {
        let q = query.trim().to_lowercase();
        if q.len() < 2 {
            return Vec::new();
        }
        let mut families: Vec<GemFamily> = self
            .gems_by_base
            .iter()
            .filter(|(base, gems)| {
                gems.iter().any(|g| g.transfigured)
                    && (base.contains(&q) || gems.iter().any(|g| g.name.to_lowercase().contains(&q)))
            })
            .map(|(_, gems)| family(gems))
            .collect();
        // Exact and prefix matches of the base first, then by name.
        families.sort_by_key(|f| {
            let b = f.base.to_lowercase();
            (b != q, !b.starts_with(&q), f.base.clone())
        });
        families.truncate(6);
        families
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

#[cfg(test)]
mod tests {
    use super::*;

    fn gem(name: &str, transfigured: bool, variant: &str, corrupted: bool, chaos: f64) -> GemPrice {
        GemPrice {
            name: name.into(),
            transfigured,
            variant: variant.into(),
            corrupted,
            chaos,
            listing_count: None,
        }
    }

    #[test]
    fn orders_variants_by_level_then_quality() {
        let mut v = vec!["20/20", "1/20", "21/20c", "1", "20"];
        v.sort_by_key(|v| variant_order(v));
        assert_eq!(v, vec!["1", "1/20", "20", "20/20", "21/20c"]);
    }

    #[test]
    fn family_lists_transfigured_gems_by_value_without_corrupted_prices() {
        let f = family(&[
            gem("Double Strike", false, "20/20", false, 20.0),
            gem("Double Strike of Impaling", true, "1/20", false, 5.4),
            gem("Double Strike of Momentum", true, "1", false, 1.0),
            gem("Double Strike of Momentum", true, "1/20", false, 15.0),
            gem("Double Strike of Momentum", true, "21/20c", true, 300.0),
        ]);
        assert_eq!(f.base, "Double Strike");
        let names: Vec<&str> = f.gems.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, vec!["Double Strike of Momentum", "Double Strike of Impaling", "Double Strike"]);
        let momentum: Vec<&str> = f.gems[0].prices.iter().map(|p| p.variant.as_str()).collect();
        assert_eq!(momentum, vec!["1", "1/20"]);
    }
}
