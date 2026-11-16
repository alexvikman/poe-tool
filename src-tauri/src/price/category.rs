//! Item categories: the game's "Item Class" line, the trade site's category ids, and
//! which categories are weapons or armour.

/// (item class as the game writes it, trade site category id, label)
const CATEGORIES: &[(&str, &str, &str)] = &[
    ("Claws", "weapon.claw", "Claw"),
    ("Daggers", "weapon.dagger", "Dagger"),
    ("Rune Daggers", "weapon.runedagger", "Rune Dagger"),
    ("Wands", "weapon.wand", "Wand"),
    ("One Hand Swords", "weapon.onesword", "One-Handed Sword"),
    ("Thrusting One Hand Swords", "weapon.onesword", "One-Handed Sword"),
    ("One Hand Axes", "weapon.oneaxe", "One-Handed Axe"),
    ("One Hand Maces", "weapon.onemace", "One-Handed Mace"),
    ("Sceptres", "weapon.sceptre", "Sceptre"),
    ("Bows", "weapon.bow", "Bow"),
    ("Staves", "weapon.staff", "Staff"),
    ("Warstaves", "weapon.warstaff", "Warstaff"),
    ("Two Hand Swords", "weapon.twosword", "Two-Handed Sword"),
    ("Two Hand Axes", "weapon.twoaxe", "Two-Handed Axe"),
    ("Two Hand Maces", "weapon.twomace", "Two-Handed Mace"),
    ("Fishing Rods", "weapon.rod", "Fishing Rod"),
    ("Quivers", "armour.quiver", "Quiver"),
    ("Shields", "armour.shield", "Shield"),
    ("Helmets", "armour.helmet", "Helmet"),
    ("Body Armours", "armour.chest", "Body Armour"),
    ("Gloves", "armour.gloves", "Gloves"),
    ("Boots", "armour.boots", "Boots"),
    ("Belts", "accessory.belt", "Belt"),
    ("Amulets", "accessory.amulet", "Amulet"),
    ("Rings", "accessory.ring", "Ring"),
    ("Trinkets", "accessory.trinket", "Trinket"),
    ("Jewels", "jewel", "Jewel"),
    ("Abyss Jewels", "jewel.abyss", "Abyss Jewel"),
    ("Life Flasks", "flask", "Flask"),
    ("Mana Flasks", "flask", "Flask"),
    ("Hybrid Flasks", "flask", "Flask"),
    ("Utility Flasks", "flask", "Flask"),
    ("Tinctures", "tincture", "Tincture"),
];

/// (the Wickerman part a viscera is, as the game writes it, trade site category id,
/// label). The trade site says Arm, Body and Leg; both spellings are taken.
const VISCERA: &[(&[&str], &str, &str)] = &[
    (&["Head"], "viscera.head", "Head Viscera"),
    (&["Arm", "Upper Limb"], "viscera.arm", "Arm Viscera"),
    (&["Body", "Torso"], "viscera.body", "Body Viscera"),
    (&["Leg", "Lower Limb"], "viscera.leg", "Leg Viscera"),
];

/// A label for a trade site category id: "weapon.twosword" → "Two-Handed Sword".
pub fn category_label(id: &str) -> Option<&'static str> {
    CATEGORIES
        .iter()
        .map(|(_, c, label)| (*c, *label))
        .chain(VISCERA.iter().map(|(_, c, label)| (*c, *label)))
        .chain([("viscera", "Viscera")])
        .find(|(c, _)| *c == id)
        .map(|(_, label)| label)
}

/// The trade site's category id and a label, for a viscera's Wickerman part; any
/// viscera when the part is one it doesn't know.
pub fn viscera_category(slot: Option<&str>) -> (&'static str, &'static str) {
    VISCERA
        .iter()
        .find(|(parts, _, _)| slot.is_some_and(|s| parts.contains(&s)))
        .map(|(_, id, label)| (*id, *label))
        .unwrap_or(("viscera", "Viscera"))
}

/// The trade site's category id and a label, for an item class.
pub fn trade_category(item_class: &str) -> Option<(&'static str, &'static str)> {
    CATEGORIES
        .iter()
        .find(|(class, _, _)| *class == item_class)
        .map(|(_, id, label)| (*id, *label))
}

pub fn is_weapon(item_class: &str) -> bool {
    trade_category(item_class).is_some_and(|(id, _)| id.starts_with("weapon."))
}

/// Armour with armour, evasion, energy shield or ward (quivers have none).
pub fn is_armour(item_class: &str) -> bool {
    matches!(item_class, "Helmets" | "Body Armours" | "Gloves" | "Boots" | "Shields")
}

/// Weapons whose price follows their physical DPS.
pub fn physical_dps_matters(item_class: &str) -> bool {
    matches!(
        item_class,
        "One Hand Axes" | "Two Hand Axes" | "One Hand Swords" | "Thrusting One Hand Swords"
            | "Two Hand Swords" | "Bows" | "Warstaves"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_item_classes() {
        assert_eq!(trade_category("Two Hand Swords"), Some(("weapon.twosword", "Two-Handed Sword")));
        assert_eq!(trade_category("Body Armours"), Some(("armour.chest", "Body Armour")));
        assert!(is_weapon("Bows"));
        assert!(!is_weapon("Quivers"));
        assert!(is_armour("Gloves"));
        assert_eq!(trade_category("Stackable Currency"), None);
    }

    #[test]
    fn maps_viscera_parts() {
        assert_eq!(viscera_category(Some("Head")), ("viscera.head", "Head Viscera"));
        assert_eq!(viscera_category(Some("Torso")), ("viscera.body", "Body Viscera"));
        assert_eq!(viscera_category(None), ("viscera", "Viscera"));
        assert_eq!(category_label("viscera.leg"), Some("Leg Viscera"));
    }
}
