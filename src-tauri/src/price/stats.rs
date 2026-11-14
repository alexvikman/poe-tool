//! Matches item mod lines to the trade site's stat ids, and finds base types.

use super::item::{Item, ItemMod, ModKind, Rarity};
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
}

/// One filter of a trade search, shown to the user as a row they can turn on or off.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatFilter {
    pub id: String,
    /// What the user sees: the item's own line, or the pseudo stat's text.
    pub text: String,
    pub value: Option<f64>,
    pub min: Option<f64>,
    pub enabled: bool,
    pub kind: String,
}

pub struct StatIndex {
    /// (trade group, text template) → stat id
    by_text: HashMap<(String, String), String>,
    bases: Vec<String>,
}

const WEAPON_OR_ARMOUR: &[&str] = &[
    "Body Armours", "Helmets", "Gloves", "Boots", "Shields", "Claws", "Daggers", "Wands",
    "One Hand Swords", "Thrusting One Hand Swords", "One Hand Axes", "One Hand Maces",
    "Sceptres", "Rune Daggers", "Bows", "Staves", "Warstaves", "Two Hand Swords",
    "Two Hand Axes", "Two Hand Maces", "Fishing Rods",
];

/// Mods that rarely make an item worth more; their filters start turned off.
const FILLER_MODS: &[&str] = &["Attribute Requirements", "Light Radius", "Stun and Block Recovery"];

/// How far below the item's own value a filter's minimum starts.
const DEFAULT_SPREAD: f64 = 0.15;

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
        StatIndex { by_text, bases }
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
        for (text, sign) in tries {
            let local_text = format!("{text} (Local)");
            let candidates: [&str; 2] = if local {
                [&local_text, &text]
            } else {
                [&text, &local_text]
            };
            for c in candidates {
                if let Some(id) = self.by_text.get(&(group.clone(), c.to_string())) {
                    return Some((id.clone(), value.map(|v| v * sign)));
                }
            }
        }
        None
    }

    /// The filters for a trade search of a rare or magic item: pseudo totals for life
    /// and resistances, then each mod. Mods the totals cover start turned off.
    pub fn filters(&self, item: &Item) -> Vec<StatFilter> {
        let local = WEAPON_OR_ARMOUR.contains(&item.item_class.as_str());
        let pseudo = pseudo_totals(&item.mods);
        let mut out: Vec<StatFilter> = pseudo
            .iter()
            .map(|(id, text, value)| StatFilter {
                id: id.to_string(),
                text: text.replace('#', &format_value(*value)),
                value: Some(*value),
                min: min_for(*value),
                enabled: true,
                kind: "pseudo".into(),
            })
            .collect();

        for m in &item.mods {
            if matches!(m.kind, ModKind::Enchant | ModKind::Scourge | ModKind::Crucible) {
                continue;
            }
            let Some((id, value)) = self.match_mod(m, local) else {
                continue;
            };
            let covered = !pseudo.is_empty() && is_covered_by_pseudo(&m.text);
            let filler = FILLER_MODS.iter().any(|f| m.text.contains(f));
            out.push(StatFilter {
                id,
                text: m.text.clone(),
                value,
                min: value.and_then(min_for),
                enabled: !covered && !filler && m.kind != ModKind::Crafted,
                kind: m.kind.trade_group().into(),
            });
        }
        out
    }
}

/// A minimum a bit below the item's own value. None for negative values ("reduced"),
/// where a minimum would rule out better items; the filter then only asks for the mod.
fn min_for(value: f64) -> Option<f64> {
    (value > 0.0).then(|| (value * (1.0 - DEFAULT_SPREAD)).floor())
}

fn format_value(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{v}")
    } else {
        format!("{v:.1}")
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

fn average(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / values.len() as f64)
    }
}

const PSEUDO_LIFE: &str = "pseudo.pseudo_total_life";
const PSEUDO_ELE_RES: &str = "pseudo.pseudo_total_elemental_resistance";
const PSEUDO_CHAOS_RES: &str = "pseudo.pseudo_total_chaos_resistance";

/// Totals the trade site also computes. Ours leave out some sources (such as life from
/// Strength), so they are never higher than the site's and never exclude the item itself.
fn pseudo_totals(mods: &[ItemMod]) -> Vec<(&'static str, &'static str, f64)> {
    let mut life = 0.0;
    let mut ele = 0.0;
    let mut chaos = 0.0;
    for m in mods {
        if matches!(m.kind, ModKind::Enchant) {
            continue;
        }
        let (t, v) = template_of(&m.text);
        let Some(&first) = v.first() else { continue };
        match t.as_str() {
            "+# to maximum Life" => life += first,
            "+#% to Fire Resistance" | "+#% to Cold Resistance" | "+#% to Lightning Resistance" => ele += first,
            "+#% to Fire and Cold Resistances"
            | "+#% to Fire and Lightning Resistances"
            | "+#% to Cold and Lightning Resistances" => ele += 2.0 * first,
            "+#% to all Elemental Resistances" => ele += 3.0 * first,
            "+#% to Chaos Resistance" => chaos += first,
            "+#% to Fire and Chaos Resistances" | "+#% to Cold and Chaos Resistances" | "+#% to Lightning and Chaos Resistances" => {
                ele += first;
                chaos += first;
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    if life > 0.0 {
        out.push((PSEUDO_LIFE, "+# total maximum Life", life));
    }
    if ele > 0.0 {
        out.push((PSEUDO_ELE_RES, "+#% total Elemental Resistance", ele));
    }
    if chaos > 0.0 {
        out.push((PSEUDO_CHAOS_RES, "+#% total to Chaos Resistance", chaos));
    }
    out
}

fn is_covered_by_pseudo(text: &str) -> bool {
    let (t, _) = template_of(text);
    t == "+# to maximum Life" || (t.starts_with("+#% to ") && t.contains("Resistance") && !t.contains("maximum"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index() -> StatIndex {
        let stats: StatsResponse = serde_json::from_str(
            r##"{"result":[
              {"id":"explicit","entries":[
                {"id":"explicit.stat_3299347043","text":"+# to maximum Life"},
                {"id":"explicit.stat_1671376347","text":"+#% to Lightning Resistance"},
                {"id":"explicit.stat_3639275092","text":"#% increased Attribute Requirements"},
                {"id":"explicit.stat_1940865751","text":"Adds # to # Physical Damage (Local)"},
                {"id":"explicit.stat_960081730","text":"Adds # to # Physical Damage to Attacks"}
              ]},
              {"id":"implicit","entries":[
                {"id":"implicit.stat_2915988346","text":"+#% to Fire and Cold Resistances"}
              ]}
            ]}"##,
        )
        .unwrap();
        let items: ItemsResponse = serde_json::from_str(
            r#"{"result":[{"entries":[{"type":"Two-Toned Boots"},{"type":"Ruby Ring"},{"type":"Ring"}]}]}"#,
        )
        .unwrap();
        StatIndex::new(&stats, &items)
    }

    fn m(kind: ModKind, text: &str) -> ItemMod {
        ItemMod { kind, text: text.into() }
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

    #[test]
    fn builds_pseudo_filters_and_turns_covered_mods_off() {
        let idx = index();
        let item = super::super::item::parse(
            "Item Class: Boots
Rarity: Rare
Grim League
Two-Toned Boots
--------
Item Level: 80
--------
+20% to Fire and Cold Resistances (implicit)
--------
32% reduced Attribute Requirements
+117 to maximum Life
+36% to Lightning Resistance",
        )
        .unwrap();
        let f = idx.filters(&item);
        let ids: Vec<(&str, bool, Option<f64>)> =
            f.iter().map(|f| (f.id.as_str(), f.enabled, f.min)).collect();
        assert_eq!(
            ids,
            vec![
                ("pseudo.pseudo_total_life", true, Some(99.0)),
                ("pseudo.pseudo_total_elemental_resistance", true, Some(64.0)),
                ("implicit.stat_2915988346", false, Some(17.0)),
                ("explicit.stat_3639275092", false, None),
                ("explicit.stat_3299347043", false, Some(99.0)),
                ("explicit.stat_1671376347", false, Some(30.0)),
            ]
        );
    }
}
