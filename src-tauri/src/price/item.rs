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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemMod {
    pub kind: ModKind,
    /// The line as the game wrote it, without the `(implicit)`-style suffix.
    pub text: String,
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
    pub influences: Vec<String>,
    pub mods: Vec<ItemMod>,
}

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
    let rarity = match header.iter().find_map(|l| l.strip_prefix("Rarity: "))? {
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
        influences: Vec::new(),
        mods: Vec::new(),
    };
    if item.name.is_none() {
        item.name = Some(item.type_line.clone());
    }

    for section in &sections[1..] {
        if section[0] == "Requirements:" {
            continue;
        }
        if parse_properties(section, &mut item) {
            continue;
        }
        if parse_flags(section, &mut item) {
            continue;
        }
        if matches!(item.rarity, Rarity::Magic | Rarity::Rare | Rarity::Unique | Rarity::Normal) {
            parse_mods(section, &mut item);
        }
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
            l if l.ends_with(" Item") && INFLUENCES.contains(&l.trim_end_matches(" Item")) => {
                item.influences.push(l.trim_end_matches(" Item").to_string())
            }
            _ => all = false,
        }
    }
    all
}

fn parse_mods(section: &[&str], item: &mut Item) {
    for line in section {
        if line.starts_with("Note: ") || line.starts_with('"') {
            continue;
        }
        let suffixes = [
            (" (implicit)", ModKind::Implicit),
            (" (crafted)", ModKind::Crafted),
            (" (fractured)", ModKind::Fractured),
            (" (enchant)", ModKind::Enchant),
            (" (scourge)", ModKind::Scourge),
            (" (crucible)", ModKind::Crucible),
        ];
        let (kind, text) = suffixes
            .iter()
            .find_map(|(suffix, kind)| line.strip_suffix(suffix).map(|t| (*kind, t)))
            .unwrap_or((ModKind::Explicit, line));
        // Flavour text and usage hints have no numbers or are known prose.
        if kind == ModKind::Explicit && !looks_like_mod(text) {
            continue;
        }
        item.mods.push(ItemMod {
            kind,
            text: text.to_string(),
        });
    }
}

fn looks_like_mod(line: &str) -> bool {
    !(line.starts_with("Place into ")
        || line.starts_with("Right click ")
        || line.starts_with("Travel to ")
        || line.starts_with("Can be ")
        || line.ends_with('.') && !line.chars().any(|c| c.is_ascii_digit()))
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
        assert_eq!(
            item.mods[0],
            ItemMod {
                kind: ModKind::Implicit,
                text: "+20% to Fire and Cold Resistances".into()
            }
        );
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

    #[test]
    fn rejects_other_text() {
        assert!(parse("hello world").is_none());
    }
}
