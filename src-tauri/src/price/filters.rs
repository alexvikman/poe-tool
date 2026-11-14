//! Which stats a price check searches for, and which start turned on.
//!
//! The order matters, as each step takes the mods it covers out of the list:
//! 1. Equipment properties: weapon DPS (at 20% quality), attack speed and critical
//!    strike chance; armour, evasion, energy shield and ward. The mods they come from
//!    (added damage, increased physical damage, …) are covered by them.
//! 2. Pseudo totals the trade site computes: total resistances, life (with life from
//!    Strength), attributes, critical strike multiplier, …
//! 3. The mods left, each its own filter.
//!
//! Turned on at the start: total DPS (or the DPS that makes up most of it), total
//! elemental resistance, total life and total chaos resistance. Every mod starts off,
//! to be ticked by hand. Uniques show only the mods that roll in a range, and all of
//! them start on when no more than three are shown.

use super::category;
use super::item::{Item, ItemMod, ModKind, Rarity, Roll};
use super::stats::{average, format_value, template_of, StatFilter, StatIndex};
use std::collections::HashMap;

/// A minimum this far (percent) below the item's own value.
const SEARCH_RANGE: f64 = 10.0;

/// A mod with what the trade site knows about it.
#[derive(Clone)]
struct Mod<'a> {
    m: &'a ItemMod,
    template: String,
    /// Trade stat id, when the site has one.
    id: Option<String>,
    /// As the trade site counts it (averaged for "Adds # to #", negated for "reduced").
    value: Option<f64>,
    /// The mod's own number (averaged), negative for "reduced", for totals.
    amount: f64,
    /// Its roll range, signed like `amount`.
    roll: Option<Roll>,
    /// Its roll range as written, for a filter on the mod itself.
    written_roll: Option<Roll>,
}

/// value, lowest and highest it could be from the mods' roll ranges.
#[derive(Debug, Clone, Copy, Default)]
struct Range {
    value: f64,
    min: f64,
    max: f64,
}

impl Range {
    fn add(&mut self, other: Range, multiplier: f64) {
        self.value += other.value * multiplier;
        self.min += other.min * multiplier;
        self.max += other.max * multiplier;
    }
}

/// The advanced copy shows a range only for mods that can roll; one it shows without
/// is fixed at its value. The plain copy shows no ranges at all.
fn written_roll(m: &ItemMod, numbers: &[f64]) -> Option<Roll> {
    match m.roll {
        Some(r) => Some(r),
        None if m.affix.is_some() => average(numbers).map(|v| Roll { min: v, max: v }),
        None => None,
    }
}

pub fn build(index: &StatIndex, item: &Item) -> Vec<StatFilter> {
    let unique = item.rarity == Rarity::Unique;
    let local = category::is_weapon(&item.item_class) || category::is_armour(&item.item_class);

    let mut mods: Vec<Mod> = item
        .mods
        .iter()
        .filter(|m| !matches!(m.kind, ModKind::Scourge | ModKind::Crucible))
        .filter_map(|m| {
            let matched = index.match_mod(m, local);
            if matched.is_none() && m.affix.is_none() && !m.text.chars().any(|c| c.is_ascii_digit()) {
                return None; // flavour text from the plain copy, not a mod
            }
            let (template, numbers) = template_of(&m.text);
            let sign = if template.contains(" reduced ") || template.contains(" less ") { -1.0 } else { 1.0 };
            let (id, value) = match matched {
                Some((id, value)) => (Some(id), value),
                None => (None, None),
            };
            Some(Mod {
                m,
                template,
                id,
                value,
                amount: average(&numbers).unwrap_or(0.0) * sign,
                // The advanced copy shows a range only for mods that can roll; one it
                // shows without is fixed at its value.
                roll: written_roll(m, &numbers).map(|r| Roll { min: r.min * sign, max: r.max * sign }),
                written_roll: written_roll(m, &numbers),
            })
        })
        .collect();

    let mut filters = Vec::new();
    if category::is_weapon(&item.item_class) {
        weapon_props(item, &mut mods, &mut filters, index);
    } else if category::is_armour(&item.item_class) {
        armour_props(item, &mut mods, &mut filters);
    }
    pseudo(index, item, &mut mods, &mut filters);

    for md in &mods {
        let variable = md.roll.is_some_and(|r| r.min != r.max);
        let mut f = StatFilter {
            id: md.id.clone().unwrap_or_default(),
            text: md.m.text.clone(),
            value: md.value,
            min: None,
            max: None,
            enabled: false,
            kind: md.m.kind.trade_group().into(),
            roll: md.m.roll,
            tier: md.m.tier,
            hidden: false,
        };
        if let Some(v) = md.value {
            let bounds = md.written_roll.map(|r| (r.min.min(r.max), r.min.max(r.max)));
            f.min = default_min(v, bounds, unique, false);
        }
        if unique && md.id.is_some() {
            match md.m.kind {
                // The base's implicit, the same on every copy.
                ModKind::Implicit if !item.corrupted && item.item_class != "Jewels" => f.hidden = true,
                ModKind::Explicit if !variable => {
                    if item.corrupted && md.roll.is_some() {
                        f.enabled = true;
                    } else {
                        f.hidden = true;
                    }
                }
                _ => {}
            }
        }
        filters.push(f);
    }

    if unique {
        let visible = filters.iter().filter(|f| !f.hidden).count();
        if visible <= 3 {
            for f in filters.iter_mut().filter(|f| !f.hidden && !f.id.is_empty()) {
                f.enabled = true;
            }
        }
    }
    filters
}

/// A minimum a little below `value`: for uniques a tenth of the roll range below it
/// (none below a perfect roll), for other items 10% below; never under the lowest
/// the mods could roll (`bounds`), so a fixed value is searched as it is. None for
/// negative values, where a minimum rules out better.
fn default_min(value: f64, bounds: Option<(f64, f64)>, unique: bool, decimals: bool) -> Option<f64> {
    if value <= 0.0 {
        return None;
    }
    let variable = bounds.filter(|(lo, hi)| hi > lo);
    let raw = match (unique, variable) {
        (true, Some((_, hi))) if value >= hi => value,
        (true, Some((lo, hi))) => value - (hi - lo) * SEARCH_RANGE / 100.0,
        (true, None) => value,
        (false, _) => value - value.abs() * SEARCH_RANGE / 100.0,
    };
    let mut min = floor_to(raw, places(value, decimals));
    // Never under the lowest the mods could roll; a fixed value is searched as it is.
    if let Some((lo, _)) = bounds {
        min = min.max(floor_to(lo, places(value, decimals)));
    }
    Some(min)
}

fn places(value: f64, decimals: bool) -> i32 {
    if (!decimals && value.fract() == 0.0) || value.abs() >= 10.0 {
        0
    } else if value.abs() < 2.3 {
        2
    } else {
        1
    }
}

fn floor_to(value: f64, places: i32) -> f64 {
    let f = 10f64.powi(places);
    ((value + f64::EPSILON) * f).floor() / f
}

fn trunc_to(value: f64, places: i32) -> f64 {
    let f = 10f64.powi(places);
    (value * f).trunc() / f
}

/// Sums the mods with any of these templates, and takes them out of `mods`.
fn take(mods: &mut Vec<Mod>, templates: &[&str]) -> Range {
    let mut total = Range::default();
    mods.retain(|md| {
        if !templates.contains(&md.template.as_str()) {
            return true;
        }
        let roll = md.roll.unwrap_or(Roll { min: md.amount, max: md.amount });
        total.add(Range { value: md.amount, min: roll.min, max: roll.max }, 1.0);
        false
    });
    total
}

fn quality_for_trade(item: &Item) -> (f64, f64) {
    let ignores_quality = item.mods.iter().any(|m| {
        m.text == "Quality does not increase Physical Damage" || m.text == "Quality does not increase Defences"
    });
    if ignores_quality {
        (0.0, 0.0)
    } else {
        let q = item.quality.unwrap_or(0) as f64;
        (q, q.max(20.0))
    }
}

fn prop_filter(id: &str, label: &str, range: Range, decimals: bool, enabled: bool, hidden: bool) -> StatFilter {
    let p = places(range.value, decimals);
    let value = trunc_to(range.value, p);
    let bounds = Some((range.min.min(range.max), range.min.max(range.max)));
    StatFilter {
        id: id.into(),
        text: label.replace('#', &format_value(value)),
        value: Some(value),
        min: default_min(range.value, bounds, false, decimals),
        max: None,
        enabled,
        kind: "property".into(),
        roll: None,
        tier: None,
        hidden,
    }
}

/// Weapon DPS as the trade site computes it, at 20% quality (or the item's own if
/// higher): the base damage under the item's mods, with the mods' current rolls and
/// their lowest and highest, so a minimum never asks for more than the mods allow.
fn weapon_props(item: &Item, mods: &mut Vec<Mod>, out: &mut Vec<StatFilter>, _index: &StatIndex) {
    let w = &item.weapon;
    let (quality, trade_quality) = quality_for_trade(item);

    let flat_phys = take(mods, &["Adds # to # Physical Damage"]);
    let incr_phys = take(mods, &["#% increased Physical Damage"]);
    let incr_speed = take(mods, &["#% increased Attack Speed"]);
    let incr_crit = take(mods, &["#% increased Critical Strike Chance"]);
    let flat_ele = take(
        mods,
        &["Adds # to # Fire Damage", "Adds # to # Cold Damage", "Adds # to # Lightning Damage"],
    );

    let aps_now = w.aps.unwrap_or(0.0);
    let aps_base = aps_now / (1.0 + incr_speed.value / 100.0);
    let aps = Range {
        value: aps_now,
        min: aps_base * (1.0 + incr_speed.min / 100.0),
        max: aps_base * (1.0 + incr_speed.max / 100.0),
    };

    let phys_now = w.physical.map(|(a, b)| (a + b) / 2.0).unwrap_or(0.0);
    let phys_base = phys_now / (1.0 + quality / 100.0) / (1.0 + incr_phys.value / 100.0) - flat_phys.value;
    let at_q = |flat: f64, incr: f64| (phys_base + flat) * (1.0 + incr / 100.0) * (1.0 + trade_quality / 100.0);
    let pdps = Range {
        value: at_q(flat_phys.value, incr_phys.value) * aps.value,
        min: at_q(flat_phys.min, incr_phys.min) * aps.min,
        max: at_q(flat_phys.max, incr_phys.max) * aps.max,
    };

    let ele_now: f64 = w.elemental.iter().map(|(a, b)| (a + b) / 2.0).sum();
    let ele_base = ele_now - flat_ele.value;
    let edps = Range {
        value: ele_now * aps.value,
        min: (ele_base + flat_ele.min) * aps.min,
        max: (ele_base + flat_ele.max) * aps.max,
    };
    let mut dps = pdps;
    dps.add(edps, 1.0);

    let has_phys = phys_now > 0.0;
    let has_ele = ele_now > 0.0;
    let q_note = if (trade_quality - quality).abs() > f64::EPSILON {
        format!(" (at {trade_quality}% quality)")
    } else {
        String::new()
    };
    if has_ele {
        if has_phys {
            out.push(prop_filter("weapon.dps", &format!("Total DPS: #{q_note}"), dps, false, true, false));
        }
        let mostly = edps.value / dps.value >= 0.67;
        out.push(prop_filter("weapon.edps", "Elemental DPS: #", edps, false, mostly, !mostly));
    }
    if has_phys {
        let mostly = pdps.value / dps.value >= 0.67;
        let on = mostly && (category::physical_dps_matters(&item.item_class) || !has_ele);
        out.push(prop_filter("weapon.pdps", &format!("Physical DPS: #{q_note}"), pdps, false, on, !mostly));
    }
    if aps_now > 0.0 {
        out.push(prop_filter("weapon.aps", "Attacks per Second: #", aps, true, false, false));
    }
    if let Some(crit_now) = w.crit {
        let crit_base = crit_now / (1.0 + incr_crit.value / 100.0);
        let crit = Range {
            value: crit_now,
            min: crit_base * (1.0 + incr_crit.min / 100.0),
            max: crit_base * (1.0 + incr_crit.max / 100.0),
        };
        out.push(prop_filter("weapon.crit", "Critical Strike Chance: #%", crit, true, false, false));
    }
}

/// Armour, evasion, energy shield and ward at 20% quality, like DPS. Only a single-
/// defence item starts with its defence on; block starts off.
fn armour_props(item: &Item, mods: &mut Vec<Mod>, out: &mut Vec<StatFilter>) {
    let a = &item.armour;
    let (quality, trade_quality) = quality_for_trade(item);
    let single = [a.armour, a.evasion, a.energy_shield, a.ward].iter().filter(|v| v.is_some()).count() == 1;

    let ar_incr = &["#% increased Armour", "#% increased Armour and Evasion", "#% increased Armour and Energy Shield",
        "#% increased Armour, Evasion and Energy Shield"];
    let ev_incr = &["#% increased Evasion Rating", "#% increased Armour and Evasion", "#% increased Evasion and Energy Shield",
        "#% increased Armour, Evasion and Energy Shield"];
    let es_incr = &["#% increased Energy Shield", "#% increased Armour and Energy Shield", "#% increased Evasion and Energy Shield",
        "#% increased Armour, Evasion and Energy Shield"];
    let ward_incr = &["#% increased Ward"];

    // Summed before any are taken out: hybrid mods count for each defence they raise.
    let sum = |mods: &Vec<Mod>, templates: &[&str]| {
        let mut copy = mods.clone();
        take(&mut copy, templates)
    };
    let defences = [
        (a.armour, "+# to Armour", &ar_incr[..], "armour.ar", "Armour: #"),
        (a.evasion, "+# to Evasion Rating", &ev_incr[..], "armour.ev", "Evasion Rating: #"),
        (a.energy_shield, "+# to maximum Energy Shield", &es_incr[..], "armour.es", "Energy Shield: #"),
        (a.ward, "+# to Ward", &ward_incr[..], "armour.ward", "Ward: #"),
    ];
    let mut computed = Vec::new();
    for (now, flat_t, incr_t, id, label) in defences {
        let Some(now) = now else { continue };
        let flat = sum(mods, &[flat_t]);
        let incr = sum(mods, incr_t);
        let base = now / (1.0 + quality / 100.0) / (1.0 + incr.value / 100.0) - flat.value;
        let at = |f: f64, i: f64| (base + f) * (1.0 + i / 100.0) * (1.0 + trade_quality / 100.0);
        let range = Range { value: at(flat.value, incr.value), min: at(flat.min, incr.min), max: at(flat.max, incr.max) };
        computed.push(prop_filter(id, label, range, false, single, false));
    }
    if let Some(block) = a.block {
        let flat = sum(mods, &["+#% Chance to Block"]);
        let base = block - flat.value;
        let range = Range { value: block, min: base + flat.min, max: base + flat.max };
        computed.push(prop_filter("armour.block", "Chance to Block: #%", range, false, false, false));
    }
    if !computed.is_empty() {
        let mut all: Vec<&str> = vec!["+# to Armour", "+# to Evasion Rating", "+# to maximum Energy Shield", "+# to Ward",
            "+#% Chance to Block"];
        all.extend(ar_incr.iter().chain(ev_incr).chain(es_incr).chain(ward_incr));
        take(mods, &all);
        out.extend(computed);
    }
}

struct Source {
    template: &'static str,
    multiplier: f64,
    required: bool,
    /// Uniques keep this mod listed beside the total.
    keep: bool,
}

struct Rule {
    pseudo: &'static str,
    enabled: bool,
    group: Option<&'static str>,
    replaces: Option<&'static str>,
    sources: Vec<Source>,
    chaos: bool,
}

fn src(template: &'static str) -> Source {
    Source { template, multiplier: 1.0, required: false, keep: false }
}

fn rule(pseudo: &'static str, sources: Vec<Source>) -> Rule {
    Rule { pseudo, enabled: false, group: None, replaces: None, sources, chaos: false }
}

/// (template, elements it covers, covers chaos)
const RESISTANCES: &[(&str, &[&str], bool)] = &[
    ("+#% to All Resistances", &["fire", "cold", "lightning"], true),
    ("+#% to all Elemental Resistances", &["fire", "cold", "lightning"], false),
    ("+#% to Fire Resistance", &["fire"], false),
    ("+#% to Cold Resistance", &["cold"], false),
    ("+#% to Lightning Resistance", &["lightning"], false),
    ("+#% to Fire and Lightning Resistances", &["fire", "lightning"], false),
    ("+#% to Fire and Cold Resistances", &["fire", "cold"], false),
    ("+#% to Cold and Lightning Resistances", &["cold", "lightning"], false),
    ("+#% to Chaos Resistance", &[], true),
    ("+#% to Fire and Chaos Resistances", &["fire"], true),
    ("+#% to Cold and Chaos Resistances", &["cold"], true),
    ("+#% to Lightning and Chaos Resistances", &["lightning"], true),
];

const ATTRIBUTES: &[(&str, &[&str])] = &[
    ("+# to all Attributes", &["str", "dex", "int"]),
    ("+# to Strength", &["str"]),
    ("+# to Dexterity", &["dex"]),
    ("+# to Intelligence", &["int"]),
    ("+# to Strength and Intelligence", &["str", "int"]),
    ("+# to Strength and Dexterity", &["str", "dex"]),
    ("+# to Dexterity and Intelligence", &["dex", "int"]),
];

fn rules() -> Vec<Rule> {
    let res = |element: &str, group: &'static str, pseudo: &'static str| Rule {
        group: Some(group),
        ..rule(
            pseudo,
            RESISTANCES
                .iter()
                .filter(|(_, els, _)| els.contains(&element))
                .map(|(t, els, chaos)| Source { keep: els.len() > 1 || *chaos, ..src(t) })
                .collect(),
        )
    };
    let attr = |a: &str, pseudo: &'static str| Rule {
        group: Some("to_x_attr"),
        ..rule(
            pseudo,
            ATTRIBUTES
                .iter()
                .filter(|(_, attrs)| attrs.contains(&a))
                .map(|(t, attrs)| Source { keep: attrs.len() > 1, ..src(t) })
                .collect(),
        )
    };
    let with_attr = |required: &'static str, a: &str| {
        let mut s = vec![Source { required: true, ..src(required) }];
        s.extend(
            ATTRIBUTES
                .iter()
                .filter(|(_, attrs)| attrs.contains(&a))
                .map(|(t, _)| Source { multiplier: 0.5, keep: true, ..src(t) }),
        );
        s
    };
    let req = |t: &'static str| Source { required: true, ..src(t) };
    vec![
        Rule {
            enabled: true,
            ..rule(
                "+#% total Elemental Resistance",
                RESISTANCES
                    .iter()
                    .filter(|(_, els, _)| !els.is_empty())
                    .map(|(t, els, chaos)| Source { multiplier: els.len() as f64, keep: *chaos, ..src(t) })
                    .collect(),
            )
        },
        res("fire", "to_x_ele_res", "+#% total to Fire Resistance"),
        res("cold", "to_x_ele_res", "+#% total to Cold Resistance"),
        res("lightning", "to_x_ele_res", "+#% total to Lightning Resistance"),
        Rule {
            chaos: true,
            ..rule(
                "+#% total to Chaos Resistance",
                RESISTANCES
                    .iter()
                    .filter(|(_, _, chaos)| *chaos)
                    .map(|(t, els, _)| Source { keep: !els.is_empty(), ..src(t) })
                    .collect(),
            )
        },
        Rule { group: Some("to_all_attrs"), ..rule("+# total to all Attributes", vec![src("+# to all Attributes")]) },
        attr("str", "+# total to Strength"),
        attr("dex", "+# total to Dexterity"),
        attr("int", "+# total to Intelligence"),
        Rule { enabled: true, ..rule("+# total maximum Life", with_attr("+# to maximum Life", "str")) },
        rule("+# total maximum Mana", with_attr("+# to maximum Mana", "int")),
        rule("#% total increased maximum Energy Shield", vec![src("#% increased maximum Energy Shield")]),
        rule("+# total maximum Energy Shield", vec![src("+# to maximum Energy Shield")]),
        rule("+#% total Attack Speed", vec![src("#% increased Attack Speed")]),
        rule("+#% total Cast Speed", vec![src("#% increased Cast Speed")]),
        rule("#% increased Movement Speed", vec![src("#% increased Movement Speed")]),
        rule("#% total increased Physical Damage", vec![src("#% increased Global Physical Damage")]),
        Rule {
            group: Some("global_crit_chance"),
            ..rule("+#% Global Critical Strike Chance", vec![src("#% increased Global Critical Strike Chance")])
        },
        Rule {
            replaces: Some("global_crit_chance"),
            ..rule(
                "+#% total Critical Strike Chance for Spells",
                vec![req("#% increased Spell Critical Strike Chance"), src("#% increased Global Critical Strike Chance")],
            )
        },
        rule("+#% Global Critical Strike Multiplier", vec![src("+#% to Global Critical Strike Multiplier")]),
        Rule { group: Some("incr_ele_dmg"), ..rule("#% increased Elemental Damage", vec![src("#% increased Elemental Damage")]) },
        Rule {
            replaces: Some("incr_ele_dmg"),
            ..rule("#% increased Lightning Damage", vec![req("#% increased Lightning Damage"), src("#% increased Elemental Damage")])
        },
        Rule {
            replaces: Some("incr_ele_dmg"),
            ..rule("#% increased Cold Damage", vec![req("#% increased Cold Damage"), src("#% increased Elemental Damage")])
        },
        Rule {
            group: Some("incr_fire_dmg"),
            replaces: Some("incr_ele_dmg"),
            ..rule("#% increased Fire Damage", vec![req("#% increased Fire Damage"), src("#% increased Elemental Damage")])
        },
        Rule { group: Some("incr_spell_dmg"), ..rule("#% increased Spell Damage", vec![src("#% increased Spell Damage")]) },
        Rule {
            replaces: Some("incr_spell_dmg"),
            ..rule("#% increased Lightning Spell Damage", vec![req("#% increased Lightning Spell Damage"), src("#% increased Spell Damage")])
        },
        Rule {
            replaces: Some("incr_spell_dmg"),
            ..rule("#% increased Cold Spell Damage", vec![req("#% increased Cold Spell Damage"), src("#% increased Spell Damage")])
        },
        Rule {
            replaces: Some("incr_spell_dmg"),
            ..rule("#% increased Fire Spell Damage", vec![req("#% increased Fire Spell Damage"), src("#% increased Spell Damage")])
        },
        Rule {
            replaces: Some("incr_ele_dmg"),
            ..rule(
                "#% increased Elemental Damage with Attack Skills",
                vec![req("#% increased Elemental Damage with Attack Skills"), src("#% increased Elemental Damage")],
            )
        },
        Rule {
            replaces: Some("incr_fire_dmg"),
            ..rule(
                "#% increased Burning Damage",
                vec![req("#% increased Burning Damage"), src("#% increased Fire Damage"), src("#% increased Elemental Damage")],
            )
        },
        rule("# Life Regenerated per Second", vec![src("Regenerate # Life per second")]),
        rule("#% of Life Regenerated per Second", vec![src("Regenerate #% of Life per second")]),
        rule("#% of Physical Attack Damage Leeched as Life", vec![src("#% of Physical Attack Damage Leeched as Life")]),
        rule("#% of Physical Attack Damage Leeched as Mana", vec![src("#% of Physical Attack Damage Leeched as Mana")]),
        rule("#% increased Mana Regeneration Rate", vec![src("#% increased Mana Regeneration Rate")]),
    ]
}

const PSEUDO_KINDS: &[ModKind] = &[ModKind::Explicit, ModKind::Implicit, ModKind::Crafted, ModKind::Fractured];

fn pseudo(index: &StatIndex, item: &Item, mods: &mut Vec<Mod>, out: &mut Vec<StatFilter>) {
    let unique = item.rarity == Rarity::Unique;
    let rules = rules();
    let start = out.len();
    let mut groups: HashMap<&str, Vec<usize>> = HashMap::new(); // filter positions in `out`
    let mut removed: Vec<usize> = Vec::new();
    let mut consumed: Vec<(&str, bool)> = Vec::new(); // (template, keep)

    'rules: for r in &rules {
        let Some(id) = index.pseudo_id(r.pseudo) else { continue };
        let sources: Vec<(&Mod, &Source)> = mods
            .iter()
            .filter(|md| PSEUDO_KINDS.contains(&md.m.kind))
            .filter_map(|md| r.sources.iter().find(|s| s.template == md.template).map(|s| (md, s)))
            .collect();
        if sources.is_empty() {
            continue;
        }
        for s in r.sources.iter().filter(|s| s.required) {
            if !sources.iter().any(|(md, _)| md.template == s.template) {
                continue 'rules;
            }
        }
        if unique {
            let n = if item.corrupted {
                sources.len()
            } else {
                sources.iter().filter(|(md, _)| md.m.kind == ModKind::Explicit).count()
            };
            if n < 2 {
                continue;
            }
        }

        let mut total = Range::default();
        let mut all_rolled = true;
        for (md, s) in &sources {
            let roll = md.roll.unwrap_or_else(|| {
                all_rolled = false;
                Roll { min: md.amount, max: md.amount }
            });
            total.add(Range { value: md.amount, min: roll.min, max: roll.max }, s.multiplier);
        }
        let bounds = all_rolled.then_some((total.min.min(total.max), total.min.max(total.max)));
        let mut f = StatFilter {
            id,
            text: r.pseudo.replace('#', &format_value(trunc_to(total.value, places(total.value, false)))),
            value: Some(total.value),
            min: default_min(total.value, bounds, unique, false),
            max: None,
            enabled: r.enabled,
            kind: "pseudo".into(),
            roll: None,
            tier: None,
            hidden: false,
        };
        if r.chaos {
            let only_crafted = sources.len() == 1 && sources[0].0.m.kind == ModKind::Crafted;
            f.hidden = only_crafted;
            f.enabled = !only_crafted;
        }
        let pos = out.len();
        out.push(f);
        for s in &r.sources {
            consumed.push((s.template, s.keep));
        }
        if let Some(replaced) = r.replaces.and_then(|g| groups.get(g)) {
            removed.extend(replaced.iter().copied());
        }
        if let Some(g) = r.group {
            groups.entry(g).or_default().push(pos);
        }
    }

    // One "total to <element> Resistance": the highest, kept out of sight; none on a tie.
    if let Some(res) = groups.get("to_x_ele_res") {
        let mut sorted = res.clone();
        sorted.sort_by(|a, b| out[*b].value.unwrap_or(0.0).total_cmp(&out[*a].value.unwrap_or(0.0)));
        let tie = sorted.len() > 1 && out[sorted[0]].value == out[sorted[1]].value;
        for (i, pos) in sorted.iter().enumerate() {
            if i == 0 && !tie {
                out[*pos].hidden = true;
            } else {
                removed.push(*pos);
            }
        }
    }
    // Attributes: "all Attributes" or the three, not both; a far smallest one hidden.
    if let Some(attrs) = groups.get("to_x_attr").filter(|a| a.len() == 3) {
        let mut sorted = attrs.clone();
        sorted.sort_by(|a, b| out[*b].value.unwrap_or(0.0).total_cmp(&out[*a].value.unwrap_or(0.0)));
        let v = |p: usize| out[p].value.unwrap_or(0.0);
        let to_all = groups.get("to_all_attrs");
        if sorted.iter().all(|p| v(*p) == v(sorted[0])) && to_all.is_some() {
            removed.extend(sorted.iter().copied());
        } else {
            if let Some(all) = to_all {
                removed.extend(all.iter().copied());
            }
            if v(sorted[0]) > 0.0 && v(sorted[2]) / v(sorted[0]) < 0.3 {
                if v(sorted[1]) == v(sorted[2]) {
                    out[sorted[1]].hidden = true;
                }
                out[sorted[2]].hidden = true;
            }
        }
    }

    let mut i = start;
    let mut kept = Vec::new();
    for f in out.drain(start..) {
        if !removed.contains(&i) {
            kept.push(f);
        }
        i += 1;
    }
    out.extend(kept);

    mods.retain(|md| {
        !PSEUDO_KINDS.contains(&md.m.kind)
            || !consumed.iter().any(|(t, keep)| *t == md.template && (!keep || !unique))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::price::item::parse;
    use crate::price::stats::{ItemsResponse, StatsResponse};

    fn index() -> StatIndex {
        let stats: StatsResponse = serde_json::from_str(
            r##"{"result":[
              {"id":"pseudo","entries":[
                {"id":"pseudo.pseudo_total_life","text":"+# total maximum Life"},
                {"id":"pseudo.pseudo_total_elemental_resistance","text":"+#% total Elemental Resistance"},
                {"id":"pseudo.pseudo_total_fire_resistance","text":"+#% total to Fire Resistance"},
                {"id":"pseudo.pseudo_total_cold_resistance","text":"+#% total to Cold Resistance"},
                {"id":"pseudo.pseudo_total_lightning_resistance","text":"+#% total to Lightning Resistance"},
                {"id":"pseudo.pseudo_global_critical_strike_multiplier","text":"+#% Global Critical Strike Multiplier"}
              ]},
              {"id":"explicit","entries":[
                {"id":"explicit.stat_3299347043","text":"+# to maximum Life"},
                {"id":"explicit.stat_1671376347","text":"+#% to Lightning Resistance"},
                {"id":"explicit.stat_3639275092","text":"#% increased Attribute Requirements"},
                {"id":"explicit.stat_709508406","text":"Adds # to # Fire Damage (Local)"},
                {"id":"explicit.stat_1509134228","text":"#% increased Physical Damage (Local)"},
                {"id":"explicit.stat_829382474","text":"+# to Level of Socketed Melee Gems"},
                {"id":"explicit.stat_1310194496","text":"#% increased Global Physical Damage"},
                {"id":"explicit.stat_2901986750","text":"+#% to all Elemental Resistances"},
                {"id":"explicit.stat_2899095498","text":"Enemies within 2.5 metres are Intimidated"}
              ]},
              {"id":"implicit","entries":[
                {"id":"implicit.stat_2915988346","text":"+#% to Fire and Cold Resistances"},
                {"id":"implicit.stat_3556824919","text":"+#% to Global Critical Strike Multiplier"},
                {"id":"implicit.stat_4080418644","text":"+# to Strength"}
              ]}
            ]}"##,
        )
        .unwrap();
        let items: ItemsResponse =
            serde_json::from_str(r#"{"result":[{"entries":[{"type":"Curved Blade"},{"type":"Two-Toned Boots"}]}]}"#).unwrap();
        StatIndex::new(&stats, &items)
    }

    fn rows(f: &[StatFilter]) -> Vec<(String, bool, bool, Option<f64>)> {
        f.iter().map(|f| (f.id.clone(), f.enabled, f.hidden, f.min)).collect()
    }

    #[test]
    fn weapon_gets_dps_and_its_mods_are_covered() {
        let item = parse(
            "Item Class: Two Hand Swords
Rarity: Rare
Pain Impaler
Curved Blade
--------
Two Hand Sword
Physical Damage: 73-120 (augmented)
Elemental Damage: 49-102 (augmented)
Critical Strike Chance: 6.00%
Attacks per Second: 1.35
Weapon Range: 1.3 metres
--------
Item Level: 35
--------
{ Implicit Modifier }
+40% to Global Critical Strike Multiplier (implicit)
--------
{ Prefix Modifier \"Searing\" (Tier: 6) }
Adds 49(46-53) to 102(90-114) Fire Damage
{ Prefix Modifier \"Cruel\" (Tier: 6) }
77(65-84)% increased Physical Damage
{ Suffix Modifier \"of Combat\" (Tier: 2) }
+1 to Level of Socketed Melee Gems",
        )
        .unwrap();
        let f = build(&index(), &item);
        let texts: Vec<&str> = f.iter().map(|f| f.text.as_str()).collect();
        assert_eq!(
            texts,
            vec![
                "Total DPS: 258 (at 20% quality)",
                "Elemental DPS: 101",
                "Physical DPS: 156 (at 20% quality)",
                "Attacks per Second: 1.35",
                "Critical Strike Chance: 6%",
                "+40% Global Critical Strike Multiplier",
                "+1 to Level of Socketed Melee Gems",
            ]
        );
        let r = rows(&f);
        // Only total DPS on; its minimum is what the mods' lowest rolls give (237).
        assert_eq!(r[0], ("weapon.dps".into(), true, false, Some(237.0)));
        assert_eq!((r[1].1, r[1].2), (false, true));
        assert_eq!((r[2].1, r[2].2), (false, true));
        assert!(r[3..].iter().all(|(_, enabled, _, _)| !enabled));
    }

    #[test]
    fn rare_armour_piece_gets_life_and_resistance_totals() {
        let item = parse(
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
        let f = build(&index(), &item);
        assert_eq!(
            rows(&f),
            vec![
                ("pseudo.pseudo_total_elemental_resistance".into(), true, false, Some(68.0)),
                ("pseudo.pseudo_total_lightning_resistance".into(), false, true, Some(32.0)),
                ("pseudo.pseudo_total_life".into(), true, false, Some(105.0)),
                ("explicit.stat_3639275092".into(), false, false, None),
            ]
        );
    }

    #[test]
    fn unique_shows_its_rolled_mods_and_turns_them_on_when_few() {
        let item = parse(
            "Item Class: Belts
Rarity: Unique
Belt of the Deceiver
Heavy Belt
--------
Item Level: 84
--------
{ Implicit Modifier }
+32(25-35) to Strength (implicit)
--------
{ Unique Modifier }
18(15-25)% increased Global Physical Damage
{ Unique Modifier }
+37(30-40) to maximum Life
{ Unique Modifier }
+15(10-15)% to all Elemental Resistances
{ Unique Modifier }
Enemies within 2.5 metres are Intimidated \u{2014} Unscalable Value",
        )
        .unwrap();
        let f = build(&index(), &item);
        let shown: Vec<(&str, bool, Option<f64>)> =
            f.iter().filter(|f| !f.hidden).map(|f| (f.text.as_str(), f.enabled, f.min)).collect();
        assert_eq!(
            shown,
            vec![
                ("18% increased Global Physical Damage", true, Some(17.0)),
                ("+37 to maximum Life", true, Some(36.0)),
                ("+15% to all Elemental Resistances", true, Some(15.0)),
            ]
        );
        assert_eq!(f.iter().filter(|f| f.hidden).count(), 2);
    }

    #[test]
    fn minimums() {
        assert_eq!(default_min(258.2, Some((237.5, 280.0)), false, false), Some(237.0));
        assert_eq!(default_min(117.0, None, false, false), Some(105.0));
        assert_eq!(default_min(1.35, Some((1.35, 1.35)), false, true), Some(1.35));
        assert_eq!(default_min(40.0, Some((40.0, 40.0)), false, false), Some(40.0));
        assert_eq!(default_min(1.35, None, false, true), Some(1.21));
        assert_eq!(default_min(18.0, Some((15.0, 25.0)), true, false), Some(17.0));
        assert_eq!(default_min(15.0, Some((10.0, 15.0)), true, false), Some(15.0));
        assert_eq!(default_min(-32.0, None, false, false), None);
    }
}
