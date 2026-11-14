//! Decides whether an area counts as a town (Path of Exile 1).

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AreaKind {
    Town,
    Hideout,
    Other,
    Unknown,
}

/// Town area ids that don't follow the `<act>_<n>_town` pattern.
const EXTRA_TOWN_IDS: &[&str] = &[
    "HeistHub",                // The Rogue Harbour
    "KalguuranSettlersLeague", // Kingsmarch
];

/// English town names from "You have entered …". Only a fallback: the area id
/// already covers these in every client language.
const TOWN_NAMES: &[&str] = &[
    "Lioneye's Watch",
    "The Forest Encampment",
    "The Sarn Encampment",
    "Highgate",
    "Overseer's Tower",
    "The Bridge Encampment",
    "Oriath Docks",
    "Karui Shores",
    "Oriath",
    "Kingsmarch",
    "The Rogue Harbour",
];

pub fn classify_id(area_id: &str) -> AreaKind {
    let lower = area_id.to_ascii_lowercase();
    if lower.contains("_town") || EXTRA_TOWN_IDS.iter().any(|id| id.eq_ignore_ascii_case(area_id)) {
        AreaKind::Town
    } else if lower.starts_with("hideout") {
        AreaKind::Hideout
    } else {
        AreaKind::Other
    }
}

pub fn is_town_name(name: &str) -> bool {
    TOWN_NAMES.iter().any(|n| n.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_towns() {
        for id in ["1_1_town", "1_5_town", "2_6_town", "2_10_town", "HeistHub", "KalguuranSettlersLeague"] {
            assert_eq!(classify_id(id), AreaKind::Town, "{id}");
        }
    }

    #[test]
    fn classifies_hideouts_and_maps() {
        assert_eq!(classify_id("HideoutLuxurious"), AreaKind::Hideout);
        assert_eq!(classify_id("MapWorldsStrand"), AreaKind::Other);
        assert_eq!(classify_id("1_1_2"), AreaKind::Other);
    }

    #[test]
    fn town_names_are_case_insensitive() {
        assert!(is_town_name("highgate"));
        assert!(!is_town_name("The Coast"));
    }
}
