//! Parses the text Path of Exile 1 puts on the clipboard when you press Ctrl+C on an item.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Rarity {
    Normal,
    Magic,
    Rare,
    Unique,
    Currency,
    Gem,
    DivinationCard,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModKind {
    Explicit,
    Implicit,
    Crafted,
    Fractured,
    Enchant,
    Scourge,
    Crucible,
}

impl ModKind {
    /// The id of the matching group in the trade site's stat list.
    pub fn trade_group(self) -> &'static str {
        match self {
            ModKind::Explicit => "explicit",
            ModKind::Implicit => "implicit",
            ModKind::Crafted => "crafted",
            ModKind::Fractured => "fractured",
            ModKind::Enchant => "enchant",
            ModKind::Scourge => "scourge",
            ModKind::Crucible => "crucible",
        }
    }
}

/// The range a mod can roll in, from the advanced copy (Ctrl+Alt+C): `18(15-25)%`.
/// For lines with several numbers, the averages, as the trade site counts them.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Roll {
    pub min: f64,
    pub max: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemMod {
    pub kind: ModKind,
    /// The line as the game wrote it, without ranges or the `(implicit)`-style suffix.
    pub text: String,
    #[serde(default)]
    pub roll: Option<Roll>,
    /// From the advanced copy's `{ … }` header: Prefix, Suffix, Unique, Implicit, …
    #[serde(default)]
    pub affix: Option<String>,
    #[serde(default)]
    pub tier: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub item_class: String,
    pub rarity: Rarity,
    /// Unique or rare name; for other items the full name line.
    pub name: Option<String>,
    /// The second header line of rares and uniques, or the single name line of
    /// normal and magic items (the base is then found with the trade site's base list).
    pub type_line: String,
    pub item_level: Option<u32>,
    pub stack_size: Option<u32>,
    pub gem_level: Option<u32>,
    pub quality: Option<u32>,
    pub map_tier: Option<u32>,
    pub sockets: Option<String>,
    pub links: u32,
    pub corrupted: bool,
    pub identified: bool,
    pub mirrored: bool,
    #[serde(default)]
    pub fractured: bool,
    #[serde(default)]
    pub synthesised: bool,
    pub influences: Vec<String>,
    pub mods: Vec<ItemMod>,
    /// Weapon properties as the item shows them (after its mods and quality).
    #[serde(default)]
    pub weapon: WeaponProps,
    /// Armour properties as the item shows them.
    #[serde(default)]
    pub armour: ArmourProps,
    /// The Wickerman part a viscera is ("Head"), from the line under its name.
    #[serde(default)]
    pub slot: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WeaponProps {
    /// (min, max) damage.
    pub physical: Option<(f64, f64)>,
    /// Each element's (min, max) damage.
    pub elemental: Vec<(f64, f64)>,
    pub chaos: Option<(f64, f64)>,
    /// Percent.
    pub crit: Option<f64>,
    pub aps: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ArmourProps {
    pub armour: Option<f64>,
    pub evasion: Option<f64>,
    pub energy_shield: Option<f64>,
    pub ward: Option<f64>,
    /// Percent.
    pub block: Option<f64>,
}

/// Keys of the weapon and armour property section; the section's first line (the
/// weapon type, such as "Two Hand Sword") has none.
const PROPERTY_KEYS: &[&str] = &[
    "Physical Damage", "Elemental Damage", "Chaos Damage", "Critical Strike Chance",
    "Attacks per Second", "Weapon Range", "Armour", "Evasion Rating", "Energy Shield", "Ward",
    "Chance to Block", "Quality",
];

const SEPARATOR: &str = "--------";

const INFLUENCES: &[&str] = &[
    "Shaper", "Elder", "Crusader", "Redeemer", "Hunter", "Warlord", "Searing Exarch",
    "Eater of Worlds",
];

pub fn parse(text: &str) -> Option<Item> {
    let text = text.replace("\r\n", "\n");
    let sections: Vec<Vec<&str>> = text
        .split(SEPARATOR)
        .map(|s| s.lines().map(str::trim).filter(|l| !l.is_empty()).collect())
        .filter(|s: &Vec<&str>| !s.is_empty())
        .collect();
    let header = sections.first()?;

    let item_class = header
        .iter()
        .find_map(|l| l.strip_prefix("Item Class: "))?
        .to_string();
    // Some items have no rarity line at all (Wickerman viscera, for one).
    let rarity = match header.iter().find_map(|l| l.strip_prefix("Rarity: ")).unwrap_or_default() {
        "Normal" => Rarity::Normal,
        "Magic" => Rarity::Magic,
        "Rare" => Rarity::Rare,
        "Unique" => Rarity::Unique,
        "Currency" => Rarity::Currency,
        "Gem" => Rarity::Gem,
        "Divination Card" => Rarity::DivinationCard,
        _ => Rarity::Other,
    };
    let names: Vec<&str> = header
        .iter()
        .copied()
        .filter(|l| !l.starts_with("Item Class: ") && !l.starts_with("Rarity: "))
        .collect();
    let (name, type_line) = match names.as_slice() {
        [name, base, ..] => (Some(name.to_string()), base.to_string()),
        [single] => (None, single.to_string()),
        [] => return None,
    };

    let mut item = Item {
        item_class,
        rarity,
        name,
        type_line,
        item_level: None,
        stack_size: None,
        gem_level: None,
        quality: None,
        map_tier: None,
        sockets: None,
        links: 0,
        corrupted: false,
        identified: true,
        mirrored: false,
        fractured: false,
        synthesised: false,
        influences: Vec::new(),
        mods: Vec::new(),
        weapon: WeaponProps::default(),
        armour: ArmourProps::default(),
        slot: None,
    };
    if item.name.is_none() {
        item.name = Some(item.type_line.clone());
    }

    if let Some([slot]) = sections.get(1).map(Vec::as_slice).filter(|_| item.item_class == "Viscera") {
        item.slot = Some(slot.to_string());
    }
    for section in &sections[1..] {
        if section[0] == "Requirements:" {
            continue;
        }
        let property_section = section.iter().any(|l| {
            l.split_once(": ").is_some_and(|(k, _)| PROPERTY_KEYS.contains(&k))
        });
        if parse_properties(section, &mut item) || property_section {
            continue;
        }
        if parse_flags(section, &mut item) {
            continue;
        }
        if matches!(item.rarity, Rarity::Magic | Rarity::Rare | Rarity::Unique | Rarity::Normal) {
            parse_mods(section, &mut item);
        }
    }
    if item.mods.iter().any(|m| m.kind == ModKind::Fractured) {
        item.fractured = true;
    }
    if item.type_line.starts_with("Synthesised ") {
        item.synthesised = true;
    }
    Some(item)
}

/// Sections made of `Key: value` lines. Returns whether the section was one.
fn parse_properties(section: &[&str], item: &mut Item) -> bool {
    let mut any = false;
    for line in section {
        let Some((key, value)) = line.split_once(": ") else {
            continue;
        };
        let number = || leading_number(value);
        match key {
            "Item Level" => item.item_level = number(),
            "Stack Size" => {
                item.stack_size = value.split('/').next().and_then(|v| leading_number(&v.replace(['.', ','], "")))
            }
            "Level" if item.rarity == Rarity::Gem => item.gem_level = number(),
            "Quality" | "Quality (Anomalous)" | "Quality (Divergent)" | "Quality (Phantasmal)" => {
                item.quality = number()
            }
            "Map Tier" => item.map_tier = number(),
            "Physical Damage" => item.weapon.physical = ranges(value).first().copied(),
            "Elemental Damage" => item.weapon.elemental = ranges(value),
            "Chaos Damage" => item.weapon.chaos = ranges(value).first().copied(),
            "Critical Strike Chance" => item.weapon.crit = decimal(value),
            "Attacks per Second" => item.weapon.aps = decimal(value),
            "Armour" => item.armour.armour = decimal(value),
            "Evasion Rating" => item.armour.evasion = decimal(value),
            "Energy Shield" => item.armour.energy_shield = decimal(value),
            "Ward" => item.armour.ward = decimal(value),
            "Chance to Block" => item.armour.block = decimal(value),
            "Sockets" => {
                let s = value.trim().to_string();
                item.links = s
                    .split(' ')
                    .map(|group| group.split('-').count() as u32)
                    .max()
                    .unwrap_or(0);
                item.sockets = Some(s);
            }
            _ => {}
        }
        any = true;
    }
    any && section.iter().all(|l| l.contains(": ") || l.ends_with(':'))
}

/// Single-word marker sections such as `Corrupted`. Returns whether the section was one.
fn parse_flags(section: &[&str], item: &mut Item) -> bool {
    let mut all = true;
    for line in section {
        match *line {
            "Corrupted" => item.corrupted = true,
            "Unidentified" => item.identified = false,
            "Mirrored" => item.mirrored = true,
            "Fractured Item" => item.fractured = true,
            "Synthesised Item" => item.synthesised = true,
            l if l.ends_with(" Item") && INFLUENCES.contains(&l.trim_end_matches(" Item")) => {
                item.influences.push(l.trim_end_matches(" Item").to_string())
            }
            _ => all = false,
        }
    }
    all
}

fn parse_mods(section: &[&str], item: &mut Item) {
    let suffixes = [
        (" (implicit)", ModKind::Implicit),
        (" (crafted)", ModKind::Crafted),
        (" (fractured)", ModKind::Fractured),
        (" (enchant)", ModKind::Enchant),
        (" (scourge)", ModKind::Scourge),
        (" (crucible)", ModKind::Crucible),
    ];
    let mut header: Option<Header> = None;
    for line in section {
        if let Some(h) = parse_header(line) {
            header = Some(h);
            continue;
        }
        if line.starts_with("Note: ") || line.starts_with('"') {
            continue;
        }
        // Reminder text in the advanced copy, such as "(Elemental Damage is …)".
        if line.starts_with('(') && line.ends_with(')') {
            continue;
        }
        let line = line.trim_end_matches(" \u{2014} Unscalable Value");
        let (marker, text) = suffixes
            .iter()
            .find_map(|(suffix, kind)| line.strip_suffix(suffix).map(|t| (Some(*kind), t)))
            .unwrap_or((None, line));
        let kind = marker
            .or(header.as_ref().map(|h| h.kind))
            .unwrap_or(ModKind::Explicit);
        // Flavour text and usage hints: only in the plain copy, which has no headers.
        if header.is_none() && marker.is_none() && !looks_like_mod(text) {
            continue;
        }
        let (text, roll) = strip_ranges(text);
        item.mods.push(ItemMod {
            kind,
            text,
            roll,
            affix: header.as_ref().map(|h| h.affix.clone()),
            tier: header.as_ref().and_then(|h| h.tier),
        });
    }
}

struct Header {
    kind: ModKind,
    affix: String,
    tier: Option<u32>,
}

/// `{ Prefix Modifier "Athlete's" (Tier: 1) — Life }`, `{ Unique Modifier }`,
/// `{ Implicit Modifier — Attribute }`, `{ Master Crafted Suffix Modifier … }`.
fn parse_header(line: &str) -> Option<Header> {
    let inner = line.strip_prefix('{')?.strip_suffix('}')?.trim();
    let kind = if inner.contains("Implicit") {
        ModKind::Implicit
    } else if inner.contains("Crafted") {
        ModKind::Crafted
    } else if inner.contains("Fractured") {
        ModKind::Fractured
    } else if inner.contains("Enchant") {
        ModKind::Enchant
    } else if inner.contains("Scourge") {
        ModKind::Scourge
    } else {
        ModKind::Explicit
    };
    let affix = ["Prefix", "Suffix", "Unique", "Implicit", "Enchant", "Searing Exarch", "Eater of Worlds"]
        .iter()
        .find(|a| inner.contains(*a))
        .map(|a| a.to_string())
        .unwrap_or_else(|| "Modifier".into());
    let tier = ["(Tier: ", "(Rank: "].iter().find_map(|p| {
        let rest = &inner[inner.find(p)? + p.len()..];
        leading_number(rest)
    });
    Some(Header { kind, affix, tier })
}

/// `"18(15-25)% increased Global Physical Damage"` →
/// (`"18% increased Global Physical Damage"`, `Some(15..25)`).
pub fn strip_ranges(line: &str) -> (String, Option<Roll>) {
    let mut out = String::with_capacity(line.len());
    let mut mins = Vec::new();
    let mut maxes = Vec::new();
    let mut rest = line;
    while let Some(open) = rest.find('(') {
        let before = &rest[..open];
        let after = &rest[open + 1..];
        let range = after
            .find(')')
            .filter(|_| before.ends_with(|c: char| c.is_ascii_digit()))
            .and_then(|close| parse_range(&after[..close]).map(|r| (close, r)));
        match range {
            Some((close, (min, max))) => {
                out.push_str(before);
                mins.push(min);
                maxes.push(max);
                rest = &after[close + 1..];
            }
            None => {
                out.push_str(&rest[..=open]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    let roll = (!mins.is_empty()).then(|| Roll {
        min: mins.iter().sum::<f64>() / mins.len() as f64,
        max: maxes.iter().sum::<f64>() / maxes.len() as f64,
    });
    (out, roll)
}

/// `"15-25"`, `"-15--5"`, `"0.5-1.5"` → (min, max).
fn parse_range(s: &str) -> Option<(f64, f64)> {
    let bytes = s.as_bytes();
    let split = (1..bytes.len()).find(|&i| bytes[i] == b'-' && bytes[i - 1].is_ascii_digit())?;
    let min = s[..split].trim().parse().ok()?;
    let max = s[split + 1..].trim().parse().ok()?;
    Some((min, max))
}

fn looks_like_mod(line: &str) -> bool {
    !(line.starts_with("Place into ")
        || line.starts_with("Right click ")
        || line.starts_with("Travel to ")
        || line.starts_with("Can be ")
        || line.ends_with('.') && !line.chars().any(|c| c.is_ascii_digit()))
}

/// `"1.35 (augmented)"` → 1.35, `"6.00%"` → 6.0.
fn decimal(s: &str) -> Option<f64> {
    let n: String = s
        .trim()
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    n.parse().ok()
}

/// `"49-102, 10-20 (augmented)"` → [(49, 102), (10, 20)].
fn ranges(s: &str) -> Vec<(f64, f64)> {
    s.split(" (")
        .next()
        .unwrap_or("")
        .split(',')
        .filter_map(|r| {
            let (a, b) = r.trim().split_once('-')?;
            Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
        })
        .collect()
}

fn leading_number(s: &str) -> Option<u32> {
    let digits: String = s
        .trim_start_matches('+')
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const RARE: &str = "Item Class: Boots
Rarity: Rare
Grim League
Two-Toned Boots
--------
Armour: 240 (augmented)
Evasion Rating: 240 (augmented)
--------
Requirements:
Level: 70
Str: 42 (augmented)
Dex: 42 (augmented)
--------
Sockets: R-G-G B
--------
Item Level: 80
--------
+20% to Fire and Cold Resistances (implicit)
--------
32% reduced Attribute Requirements
83% increased Armour and Evasion
+117 to maximum Life
+36% to Lightning Resistance
+25 to maximum Mana (crafted)
--------
Corrupted
--------
Hunter Item
";

    #[test]
    fn parses_a_rare() {
        let item = parse(&RARE.replace('\n', "\r\n")).unwrap();
        assert_eq!(item.item_class, "Boots");
        assert_eq!(item.rarity, Rarity::Rare);
        assert_eq!(item.name.as_deref(), Some("Grim League"));
        assert_eq!(item.type_line, "Two-Toned Boots");
        assert_eq!(item.item_level, Some(80));
        assert_eq!(item.links, 3);
        assert!(item.corrupted);
        assert_eq!(item.influences, vec!["Hunter"]);
        assert_eq!(item.mods.len(), 6);
        assert_eq!(item.mods[0].kind, ModKind::Implicit);
        assert_eq!(item.mods[0].text, "+20% to Fire and Cold Resistances");
        assert_eq!(item.mods[0].roll, None);
        assert_eq!(item.mods[5].kind, ModKind::Crafted);
    }

    #[test]
    fn parses_currency_stack() {
        let item = parse(
            "Item Class: Stackable Currency
Rarity: Currency
Chaos Orb
--------
Stack Size: 1,234/5000
--------
Reforges a rare item with new random modifiers
--------
Right click this item then left click a rare item to apply it.",
        )
        .unwrap();
        assert_eq!(item.rarity, Rarity::Currency);
        assert_eq!(item.type_line, "Chaos Orb");
        assert_eq!(item.stack_size, Some(1234));
        assert!(item.mods.is_empty());
    }

    #[test]
    fn parses_an_item_without_a_rarity() {
        let item = parse(
            "Item Class: Viscera
Inquisitor's Head
--------
Head
--------
Item Level: 64
--------
{ Implicit Modifier }
Causes Bleeding on Melee Critical Strike — Unscalable Value
(Bleeding deals Physical Damage over time, based on the base Physical Damage of the Skill. Damage is 200% higher while moving)
--------
Take this to the Wickerman in a town or hideout to attach it.",
        )
        .unwrap();
        assert_eq!(item.item_class, "Viscera");
        assert_eq!(item.rarity, Rarity::Other);
        assert_eq!(item.type_line, "Inquisitor's Head");
        assert_eq!(item.item_level, Some(64));
        assert_eq!(item.slot.as_deref(), Some("Head"));
    }

    #[test]
    fn parses_a_gem() {
        let item = parse(
            "Item Class: Skill Gems
Rarity: Gem
Cold Snap of Power
--------
Spell, AoE, Cold, Duration
Level: 21 (Max)
Quality: +20% (augmented)
--------
Requirements:
Level: 72
--------
Corrupted",
        )
        .unwrap();
        assert_eq!(item.rarity, Rarity::Gem);
        assert_eq!(item.gem_level, Some(21));
        assert_eq!(item.quality, Some(20));
        assert!(item.corrupted);
    }

    #[test]
    fn parses_a_unique() {
        let item = parse(
            "Item Class: Body Armours
Rarity: Unique
Tabula Rasa
Simple Robe
--------
Sockets: W-W-W-W-W-W
--------
Item Level: 68
--------
Item has no Level requirement
--------
\"Each scar tells a story.\"",
        )
        .unwrap();
        assert_eq!(item.rarity, Rarity::Unique);
        assert_eq!(item.name.as_deref(), Some("Tabula Rasa"));
        assert_eq!(item.type_line, "Simple Robe");
        assert_eq!(item.links, 6);
    }

    const ADVANCED_UNIQUE: &str = "Item Class: Belts
Rarity: Unique
Belt of the Deceiver
Heavy Belt
--------
Requirements:
Level: 20
--------
Item Level: 84
--------
{ Implicit Modifier \u{2014} Attribute }
+32(25-35) to Strength (implicit)
--------
{ Unique Modifier \u{2014} Damage, Physical, Attack }
18(15-25)% increased Global Physical Damage
{ Unique Modifier \u{2014} Life }
+37(30-40) to maximum Life
{ Unique Modifier \u{2014} Elemental, Fire, Cold, Lightning, Resistance }
+15(10-15)% to all Elemental Resistances
{ Unique Modifier }
Enemies within 2.5 metres are Intimidated \u{2014} Unscalable Value
--------
Before you can lie, you must believe.
";

    #[test]
    fn parses_the_advanced_copy() {
        let item = parse(ADVANCED_UNIQUE).unwrap();
        let mods: Vec<(ModKind, &str, Option<(f64, f64)>, Option<&str>)> = item
            .mods
            .iter()
            .map(|m| (m.kind, m.text.as_str(), m.roll.map(|r| (r.min, r.max)), m.affix.as_deref()))
            .collect();
        assert_eq!(
            mods,
            vec![
                (ModKind::Implicit, "+32 to Strength", Some((25.0, 35.0)), Some("Implicit")),
                (ModKind::Explicit, "18% increased Global Physical Damage", Some((15.0, 25.0)), Some("Unique")),
                (ModKind::Explicit, "+37 to maximum Life", Some((30.0, 40.0)), Some("Unique")),
                (ModKind::Explicit, "+15% to all Elemental Resistances", Some((10.0, 15.0)), Some("Unique")),
                (ModKind::Explicit, "Enemies within 2.5 metres are Intimidated", None, Some("Unique")),
            ]
        );
    }

    #[test]
    fn reads_tiers_and_crafted_headers() {
        let item = parse(
            "Item Class: Boots
Rarity: Rare
Grim League
Two-Toned Boots
--------
Item Level: 80
--------
{ Prefix Modifier \"Athlete's\" (Tier: 1) \u{2014} Life }
+117(115-129) to maximum Life
{ Master Crafted Suffix Modifier \"of Craft\" (Rank: 2) \u{2014} Mana }
+25(20-30) to maximum Mana (crafted)",
        )
        .unwrap();
        assert_eq!(item.mods[0].tier, Some(1));
        assert_eq!(item.mods[0].affix.as_deref(), Some("Prefix"));
        assert_eq!(item.mods[1].kind, ModKind::Crafted);
        assert_eq!(item.mods[1].tier, Some(2));
    }

    #[test]
    fn strips_ranges_from_lines() {
        assert_eq!(
            strip_ranges("Adds 10(8-12) to 20(18-24) Physical Damage"),
            ("Adds 10 to 20 Physical Damage".into(), Some(Roll { min: 13.0, max: 18.0 }))
        );
        assert_eq!(
            strip_ranges("-10(-15--5)% to Fire Resistance"),
            ("-10% to Fire Resistance".into(), Some(Roll { min: -15.0, max: -5.0 }))
        );
        assert_eq!(strip_ranges("Socketed Gems (Level 5)"), ("Socketed Gems (Level 5)".into(), None));
    }

    #[test]
    fn reads_weapon_properties_and_keeps_them_out_of_the_mods() {
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
Requirements:
Level: 35
--------
Item Level: 35
--------
{ Implicit Modifier \u{2014} Damage, Critical }
+40% to Global Critical Strike Multiplier (implicit)
--------
{ Prefix Modifier \"Searing\" (Tier: 6) \u{2014} Damage, Elemental, Fire, Attack }
Adds 49(44-53) to 102(84-114) Fire Damage
{ Prefix Modifier \"Cruel\" (Tier: 6) \u{2014} Damage, Physical, Attack }
77(65-84)% increased Physical Damage",
        )
        .unwrap();
        assert_eq!(item.weapon.physical, Some((73.0, 120.0)));
        assert_eq!(item.weapon.elemental, vec![(49.0, 102.0)]);
        assert_eq!(item.weapon.crit, Some(6.0));
        assert_eq!(item.weapon.aps, Some(1.35));
        let texts: Vec<&str> = item.mods.iter().map(|m| m.text.as_str()).collect();
        assert_eq!(
            texts,
            vec![
                "+40% to Global Critical Strike Multiplier",
                "Adds 49 to 102 Fire Damage",
                "77% increased Physical Damage"
            ]
        );
    }

    #[test]
    fn rejects_other_text() {
        assert!(parse("hello world").is_none());
    }
}
