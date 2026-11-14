//! Matches item mod lines to the trade site's stat ids, and finds base types.

use super::item::{Item, ItemMod, Rarity, Roll};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// `GET /api/trade/data/stats`
#[derive(Debug, Deserialize)]
pub struct StatsResponse {
    pub result: Vec<StatGroup>,
}

#[derive(Debug, Deserialize)]
pub struct StatGroup {
    pub id: String,
    pub entries: Vec<StatEntry>,
}

#[derive(Debug, Deserialize)]
pub struct StatEntry {
    pub id: String,
    pub text: String,
}

/// `GET /api/trade/data/items`
#[derive(Debug, Deserialize)]
pub struct ItemsResponse {
    pub result: Vec<ItemGroup>,
}

#[derive(Debug, Deserialize)]
pub struct ItemGroup {
    pub entries: Vec<ItemEntry>,
}

#[derive(Debug, Deserialize)]
pub struct ItemEntry {
    #[serde(rename = "type")]
    pub base: String,
    /// For a transfigured gem, its name ("Cold Snap of Power"); `base` is then the gem
    /// it comes from and `disc` tells the trade site which version.
    pub text: Option<String>,
    pub disc: Option<String>,
}

/// `GET /api/trade/data/static`: bulk items and their ids on the currency exchange.
#[derive(Debug, Deserialize)]
pub struct StaticResponse {
    pub result: Vec<StaticGroup>,
}

#[derive(Debug, Deserialize)]
pub struct StaticGroup {
    pub entries: Vec<StaticEntry>,
}

#[derive(Debug, Deserialize)]
pub struct StaticEntry {
    pub id: String,
    pub text: Option<String>,
}

/// One filter of a trade search, shown to the user as a row they can turn on or off.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatFilter {
    /// Empty when the trade site has no stat for this line (shown, but not searchable).
    pub id: String,
    /// What the user sees: the item's own line, or the pseudo stat's text.
    pub text: String,
    pub value: Option<f64>,
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    pub enabled: bool,
    pub kind: String,
    /// The range the mod can roll in, when the item was copied with Ctrl+Alt+C.
    #[serde(default)]
    pub roll: Option<Roll>,
    #[serde(default)]
    pub tier: Option<u32>,
    /// Kept out of sight unless the user asks (constant rolls, totals that repeat others).
    #[serde(default)]
    pub hidden: bool,
}

pub struct StatIndex {
    /// (trade group, text template) → stat id
    by_text: HashMap<(String, String), String>,
    bases: Vec<String>,
    /// Transfigured gem name (lower case) → (base gem, discriminator).
    transfigured: HashMap<String, (String, String)>,
    /// Bulk item name (lower case) → its id on the currency exchange ("divine").
    exchange_ids: HashMap<String, String>,
}

impl StatIndex {
    pub fn new(stats: &StatsResponse, items: &ItemsResponse) -> Self {
        let mut by_text = HashMap::new();
        for group in &stats.result {
            for e in &group.entries {
                by_text
                    .entry((group.id.clone(), e.text.clone()))
                    .or_insert_with(|| e.id.clone());
            }
        }
        let mut seen = HashSet::new();
        let mut bases: Vec<String> = items
            .result
            .iter()
            .flat_map(|g| &g.entries)
            .map(|e| e.base.clone())
            .filter(|b| seen.insert(b.clone()))
            .collect();
        // Longest first, so "Two-Toned Boots" wins over "Boots"-like shorter names.
        bases.sort_by_key(|b| std::cmp::Reverse(b.len()));
        let transfigured = items
            .result
            .iter()
            .flat_map(|g| &g.entries)
            .filter_map(|e| Some((e.text.as_ref()?.to_lowercase(), (e.base.clone(), e.disc.clone()?))))
            .collect();
        StatIndex { by_text, bases, transfigured, exchange_ids: HashMap::new() }
    }

    pub fn with_exchange(mut self, statics: &StaticResponse) -> Self {
        self.exchange_ids = statics
            .result
            .iter()
            .flat_map(|g| &g.entries)
            .filter_map(|e| Some((e.text.as_ref()?.to_lowercase(), e.id.clone())))
            .collect();
        self
    }

    /// The currency exchange's id for a bulk item, such as "divine" for Divine Orb.
    pub fn exchange_id(&self, name: &str) -> Option<String> {
        self.exchange_ids.get(&name.to_lowercase()).cloned()
    }

    /// For a transfigured gem: its base gem and the trade site's discriminator.
    pub fn transfigured(&self, name: &str) -> Option<(String, String)> {
        self.transfigured.get(&name.to_lowercase()).cloned()
    }

    /// The base type of an item: rares and uniques say it on their second line,
    /// normal and magic items have it inside their name.
    pub fn base_type(&self, item: &Item) -> String {
        let line = item
            .type_line
            .trim_start_matches("Synthesised ")
            .trim_start_matches("Superior ");
        if matches!(item.rarity, Rarity::Rare | Rarity::Unique) {
            return line.to_string();
        }
        self.bases
            .iter()
            .find(|b| line.contains(b.as_str()))
            .cloned()
            .unwrap_or_else(|| line.to_string())
    }

    /// Finds the stat id of a mod line, and its value as the trade site counts it.
    pub fn match_mod(&self, m: &ItemMod, local: bool) -> Option<(String, Option<f64>)> {
        let group = m.kind.trade_group().to_string();
        let (template, values) = template_of(&m.text);
        let value = average(&values);

        let mut tries: Vec<(String, f64)> = vec![(template.clone(), 1.0)];
        for (a, b) in [(" increased ", " reduced "), (" reduced ", " increased "), (" more ", " less "), (" less ", " more ")] {
            if template.contains(a) {
                tries.push((template.replacen(a, b, 1), -1.0));
            }
        }
        if let Some(rest) = template.strip_prefix('-') {
            tries.push((format!("+{rest}"), -1.0));
        }
        // Lines whose numbers are fixed text on the site, such as "within 2.5 metres".
        tries.push((m.text.clone(), 0.0));
        for (text, sign) in tries {
            let local_text = format!("{text} (Local)");
            let candidates: [&str; 2] = if local {
                [&local_text, &text]
            } else {
                [&text, &local_text]
            };
            for c in candidates {
                if let Some(id) = self.by_text.get(&(group.clone(), c.to_string())) {
                    let value = if sign == 0.0 { None } else { value.map(|v| v * sign) };
                    return Some((id.clone(), value));
                }
            }
        }
        None
    }

    /// The trade site's id for a pseudo stat, by its text ("+# total maximum Life").
    pub fn pseudo_id(&self, text: &str) -> Option<String> {
        self.by_text.get(&("pseudo".to_string(), text.to_string())).cloned()
    }
}

pub(crate) fn format_value(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{v}")
    } else {
        format!("{v:.2}").trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// `"+73 to maximum Life"` → (`"+# to maximum Life"`, `[73]`).
pub fn template_of(text: &str) -> (String, Vec<f64>) {
    let mut template = String::with_capacity(text.len());
    let mut values = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            let mut num = String::from(c);
            while let Some(&n) = chars.peek() {
                if n.is_ascii_digit() || n == '.' {
                    num.push(n);
                    chars.next();
                } else {
                    break;
                }
            }
            values.push(num.trim_end_matches('.').parse().unwrap_or(0.0));
            template.push('#');
        } else {
            template.push(c);
        }
    }
    (template, values)
}

pub(crate) fn average(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / values.len() as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::price::item::ModKind;

    fn index() -> StatIndex {
        let stats: StatsResponse = serde_json::from_str(
            r##"{"result":[
              {"id":"explicit","entries":[
                {"id":"explicit.stat_3299347043","text":"+# to maximum Life"},
                {"id":"explicit.stat_1671376347","text":"+#% to Lightning Resistance"},
                {"id":"explicit.stat_3639275092","text":"#% increased Attribute Requirements"},
                {"id":"explicit.stat_1940865751","text":"Adds # to # Physical Damage (Local)"},
                {"id":"explicit.stat_960081730","text":"Adds # to # Physical Damage to Attacks"},
                {"id":"explicit.stat_1310194496","text":"#% increased Global Physical Damage"},
                {"id":"explicit.stat_2901986750","text":"+#% to all Elemental Resistances"},
                {"id":"explicit.stat_2899095498","text":"Enemies within 2.5 metres are Intimidated"}
              ]},
              {"id":"implicit","entries":[
                {"id":"implicit.stat_2915988346","text":"+#% to Fire and Cold Resistances"}
              ]}
            ]}"##,
        )
        .unwrap();
        let items: ItemsResponse = serde_json::from_str(
            r#"{"result":[{"entries":[{"type":"Two-Toned Boots"},{"type":"Ruby Ring"},{"type":"Ring"},
              {"type":"Cold Snap"},{"type":"Cold Snap","text":"Cold Snap of Power","disc":"alt_x"}]}]}"#,
        )
        .unwrap();
        StatIndex::new(&stats, &items)
    }

    fn m(kind: ModKind, text: &str) -> ItemMod {
        ItemMod { kind, text: text.into(), roll: None, affix: None, tier: None }
    }

    #[test]
    fn templates() {
        assert_eq!(template_of("+73 to maximum Life"), ("+# to maximum Life".into(), vec![73.0]));
        assert_eq!(
            template_of("Adds 10 to 20.5 Physical Damage"),
            ("Adds # to # Physical Damage".into(), vec![10.0, 20.5])
        );
    }

    #[test]
    fn matches_plain_reduced_and_local_mods() {
        let idx = index();
        assert_eq!(
            idx.match_mod(&m(ModKind::Explicit, "+73 to maximum Life"), false),
            Some(("explicit.stat_3299347043".into(), Some(73.0)))
        );
        assert_eq!(
            idx.match_mod(&m(ModKind::Explicit, "32% reduced Attribute Requirements"), false),
            Some(("explicit.stat_3639275092".into(), Some(-32.0)))
        );
        assert_eq!(
            idx.match_mod(&m(ModKind::Explicit, "Adds 10 to 20 Physical Damage"), true),
            Some(("explicit.stat_1940865751".into(), Some(15.0)))
        );
        assert_eq!(
            idx.match_mod(&m(ModKind::Implicit, "+20% to Fire and Cold Resistances"), false),
            Some(("implicit.stat_2915988346".into(), Some(20.0)))
        );
        assert_eq!(idx.match_mod(&m(ModKind::Explicit, "Something unknown"), false), None);
    }

    #[test]
    fn knows_transfigured_gems() {
        let idx = index();
        assert_eq!(idx.transfigured("Cold Snap of Power"), Some(("Cold Snap".into(), "alt_x".into())));
        assert_eq!(idx.transfigured("Cold Snap"), None);
    }

    #[test]
    fn finds_base_type_of_magic_items() {
        let idx = index();
        let mut item = super::super::item::parse(
            "Item Class: Rings\nRarity: Magic\nCoral Ruby Ring of the Whelpling\n--------\nItem Level: 80",
        )
        .unwrap();
        assert_eq!(idx.base_type(&item), "Ruby Ring");
        item.rarity = Rarity::Rare;
        item.type_line = "Two-Toned Boots".into();
        assert_eq!(idx.base_type(&item), "Two-Toned Boots");
    }
}
