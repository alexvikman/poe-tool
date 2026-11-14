//! Matches item mod lines to the trade site's stat ids, and finds base types.

use super::item::{Item, ItemMod, ModKind, Rarity, Roll};
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

    /// The filters for a trade search: for rares and magic items, pseudo totals for
    /// life and resistances, then each mod (those the totals cover start off); for
    /// uniques, each mod, with the ones that roll in a range turned on. Lines the trade
    /// site has no stat for are listed with an empty id.
    pub fn filters(&self, item: &Item) -> Vec<StatFilter> {
        let unique = item.rarity == Rarity::Unique;
        let local = WEAPON_OR_ARMOUR.contains(&item.item_class.as_str());
        let pseudo = if unique { Vec::new() } else { pseudo_totals(&item.mods) };
        let mut out: Vec<StatFilter> = pseudo
            .iter()
            .map(|(id, text, value)| StatFilter {
                id: id.to_string(),
                text: text.replace('#', &format_value(*value)),
                value: Some(*value),
                min: min_for(*value),
                max: None,
                enabled: true,
                kind: "pseudo".into(),
                roll: None,
                tier: None,
            })
            .collect();

        for m in &item.mods {
            if matches!(m.kind, ModKind::Scourge | ModKind::Crucible) {
                continue;
            }
            let matched = self.match_mod(m, local);
            if matched.is_none() && m.affix.is_none() && !m.text.chars().any(|c| c.is_ascii_digit()) {
                // Probably flavour text from the plain copy, not a mod.
                continue;
            }
            let (id, value) = matched.unwrap_or_default();
            let variable = m.roll.is_some_and(|r| r.max > r.min);
            let enabled = !id.is_empty()
                && if unique {
                    variable && m.kind == ModKind::Explicit
                } else {
                    let covered = !pseudo.is_empty() && is_covered_by_pseudo(&m.text);
                    let filler = FILLER_MODS.iter().any(|f| m.text.contains(f));
                    !covered && !filler && !matches!(m.kind, ModKind::Crafted | ModKind::Enchant)
                };
            let min = match (unique, value, m.roll) {
                (true, Some(v), Some(r)) if r.max > r.min => Some(min_in_roll(v, r)),
                (true, Some(v), _) => Some(v),
                (_, v, _) => v.and_then(min_for),
            };
            out.push(StatFilter {
                id,
                text: m.text.clone(),
                value,
                min,
                max: None,
                enabled,
                kind: m.kind.trade_group().into(),
                roll: m.roll,
                tier: m.tier,
            });
        }
        out
    }
}

/// Just below the item's roll: a tenth of the roll's range lower, never under the
/// range itself. `18 (15-25)` → 17.
fn min_in_roll(value: f64, roll: Roll) -> f64 {
    let (lo, hi) = if roll.min <= roll.max { (roll.min, roll.max) } else { (roll.max, roll.min) };
    (value - 0.1 * (hi - lo)).ceil().clamp(lo.min(value), value)
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
    fn unique_filters_turn_on_variable_mods_and_flag_unknown_lines() {
        let idx = index();
        let item = super::super::item::parse(
            "Item Class: Belts
Rarity: Unique
Belt of the Deceiver
Heavy Belt
--------
Item Level: 84
--------
{ Unique Modifier \u{2014} Damage }
18(15-25)% increased Global Physical Damage
{ Unique Modifier \u{2014} Life }
+37(30-40) to maximum Life
{ Unique Modifier }
+15(10-15)% to all Elemental Resistances
{ Unique Modifier }
Enemies within 2.5 metres are Intimidated \u{2014} Unscalable Value
{ Unique Modifier }
Something the site doesn't know",
        )
        .unwrap();
        let f = idx.filters(&item);
        let rows: Vec<(&str, bool, Option<f64>)> =
            f.iter().map(|f| (f.id.as_str(), f.enabled, f.min)).collect();
        assert_eq!(
            rows,
            vec![
                ("explicit.stat_1310194496", true, Some(17.0)),
                ("explicit.stat_3299347043", true, Some(36.0)),
                ("explicit.stat_2901986750", true, Some(15.0)),
                ("explicit.stat_2899095498", false, None),
                ("", false, None),
            ]
        );
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
