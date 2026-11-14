//! Builds from Path of Building: imported from a pobb.in link, pastebin, poe.ninja or a
//! PoB code, and shown like pobb.in shows them. PoB's export carries the stats PoB
//! calculated (DPS, life, resistances…), so showing a build needs no PoB here.
//!
//! A build belongs to a build in Searches (`SearchGroup`), so its searches and its PoB
//! build sit together.

use crate::live::{HitItem, HitMod, HitProperty, HitRequirement};
use crate::price::PriceService;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// What's kept of an imported build.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredBuild {
    /// The PoB code, as PoB exports it.
    pub code: String,
    /// Where it came from: a pobb.in, pastebin or poe.ninja link.
    pub source: Option<String>,
}

/* Import */

/// The PoB code behind what the user pasted: a pobb.in, pastebin or poe.ninja link, or
/// the code itself. Says where it came from, for links.
pub async fn fetch_code(http: &reqwest::Client, input: &str) -> Result<(String, Option<String>), String> {
    let input = input.trim();
    let Some(raw_url) = raw_url(input) else {
        let code: String = input.split_whitespace().collect();
        decode(&code)?; // Says so now if it isn't one.
        return Ok((code, None));
    };
    let resp = http
        .get(&raw_url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("Couldn't get the build: {e}"))?;
    let code: String = resp.text().await.map_err(|e| e.to_string())?.split_whitespace().collect();
    decode(&code).map_err(|_| "That link doesn't hold a Path of Building build.".to_string())?;
    Ok((code, Some(input.to_string())))
}

/// Where a link's PoB code can be read as plain text, as PoB itself imports them.
fn raw_url(input: &str) -> Option<String> {
    let url = input.split(['?', '#']).next()?.trim_end_matches('/');
    let after = |host: &str| url.split_once(host).map(|(_, rest)| rest.trim_start_matches('/'));
    if let Some(id) = after("pobb.in") {
        let id = id.trim_end_matches("/raw");
        return (!id.is_empty()).then(|| format!("https://pobb.in/{id}/raw"));
    }
    if let Some(id) = after("pastebin.com") {
        let id = id.trim_start_matches("raw/");
        return (!id.is_empty()).then(|| format!("https://pastebin.com/raw/{id}"));
    }
    if let Some(rest) = after("poe.ninja") {
        // poe.ninja/pob/<id>, also with /poe1 in front.
        let id = rest.trim_start_matches("poe1/").trim_start_matches("pob/").trim_start_matches("raw/");
        return (!id.is_empty() && rest.contains("pob")).then(|| format!("https://poe.ninja/pob/raw/{id}"));
    }
    None
}

/// A PoB code's XML: base64 (either alphabet) of zlib-compressed XML.
pub fn decode(code: &str) -> Result<String, String> {
    let not_a_build = || "That isn't a Path of Building code or a link to one.".to_string();
    let clean = code.trim().trim_end_matches('=').replace('+', "-").replace('/', "_");
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(clean).map_err(|_| not_a_build())?;
    let mut xml = String::new();
    flate2::read::ZlibDecoder::new(&bytes[..])
        .read_to_string(&mut xml)
        .map_err(|_| not_a_build())?;
    if !xml.contains("<PathOfBuilding") {
        return Err(not_a_build());
    }
    Ok(xml)
}

/* The build as shown */

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BuildView {
    pub class: String,
    pub ascendancy: Option<String>,
    pub level: u32,
    /// The main skill's name, for the title ("Ball Lightning of Orbiting").
    pub main_skill: Option<String>,
    /// The game version, from the tree ("3_27").
    pub version: Option<String>,
    pub bandit: Option<String>,
    pub pantheon: Vec<String>,
    /// What PoB calculated at export, by PoB's own names ("CombinedDPS", "Life"): for
    /// the loadout it was exported with (`active`).
    pub stats: BTreeMap<String, f64>,
    /// PoB's sets, as the build has them; a build without sets has one of each.
    pub item_sets: Vec<ItemSetView>,
    pub skill_sets: Vec<SkillSetView>,
    pub trees: Vec<TreeView>,
    pub configs: Vec<ConfigView>,
    /// Sets that go together, by name as in PoB ("Lvl 96"): items, gems, tree, config.
    pub loadouts: Vec<Loadout>,
    /// What PoB had chosen when the build was exported.
    pub active: Loadout,
    pub notes: String,
    pub code: String,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ItemSetView {
    /// PoB's id for the set, to choose it.
    pub id: Option<String>,
    pub title: Option<String>,
    pub gear: Vec<GearItem>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SkillSetView {
    /// PoB's id for the set, to choose it.
    pub id: Option<String>,
    pub title: Option<String>,
    pub groups: Vec<GemGroup>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConfigView {
    /// PoB's id for the set, to choose it.
    pub id: Option<String>,
    pub title: Option<String>,
    /// "Boss", "30% Shock", "Custom Mods".
    pub config: Vec<String>,
}

/// One of each kind of set, by its place in the lists above.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Loadout {
    #[serde(default)]
    pub title: String,
    pub item_set: usize,
    pub skill_set: usize,
    pub tree: usize,
    pub config: usize,
}

/// An item in a slot ("Helmet", "Ring 1", "Flask 3", or a jewel in the tree).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GearItem {
    pub slot: String,
    pub item: HitItem,
}

/// A group of linked gems.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GemGroup {
    pub label: Option<String>,
    pub slot: Option<String>,
    pub enabled: bool,
    /// The build's main skill is in this group.
    pub main: bool,
    pub gems: Vec<Gem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Gem {
    pub name: String,
    pub level: u32,
    pub quality: u32,
    pub enabled: bool,
    pub support: bool,
    /// red, green, blue or white, from the gem's attribute.
    pub colour: Option<&'static str>,
    #[serde(skip)]
    variant_id: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TreeView {
    pub title: Option<String>,
    pub version: Option<String>,
    /// Jewels socketed in this tree.
    pub jewels: Vec<GearItem>,
    /// Points spent outside the ascendancy.
    pub points: usize,
    pub ascendancy_points: usize,
    pub keystones: Vec<String>,
    pub notables: Vec<String>,
    pub ascendancy_notables: Vec<String>,
    /// Chosen mastery effects: "Attributes Mastery: 5% increased Attributes".
    pub masteries: Vec<String>,
    #[serde(skip)]
    nodes: Vec<u32>,
    #[serde(skip)]
    mastery_effects: Vec<(u32, u32)>,
}

/// Reads a build's XML.
pub fn parse(xml: &str) -> Result<BuildView, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| format!("The build couldn't be read: {e}"))?;
    let root = doc.root_element();
    let child = |name: &str| root.children().find(|n| n.has_tag_name(name));
    let mut view = BuildView::default();
    let mut main_group = 1;

    if let Some(build) = child("Build") {
        view.class = build.attribute("className").unwrap_or_default().to_string();
        view.ascendancy = build.attribute("ascendClassName").filter(|a| !a.is_empty() && *a != "None").map(str::to_string);
        view.level = build.attribute("level").and_then(|l| l.parse().ok()).unwrap_or(1);
        view.version = build.attribute("targetVersion").map(str::to_string);
        // PoB writes killing them all as "None".
        view.bandit = build.attribute("bandit").map(|b| if b == "None" { "Kill All".to_string() } else { b.to_string() });
        for god in ["pantheonMajorGod", "pantheonMinorGod"] {
            if let Some(g) = build.attribute(god).filter(|g| *g != "None") {
                view.pantheon.push(format!("Soul of {g}"));
            }
        }
        for stat in build.children().filter(|n| n.has_tag_name("PlayerStat")) {
            if let (Some(name), Some(value)) = (stat.attribute("stat"), stat.attribute("value").and_then(|v| v.parse().ok())) {
                view.stats.insert(name.to_string(), value);
            }
        }
        main_group = build.attribute("mainSocketGroup").and_then(|m| m.parse().ok()).unwrap_or(1);
    }

    let items = child("Items");
    let item_texts: HashMap<&str, &str> = items
        .iter()
        .flat_map(|i| i.children())
        .filter(|n| n.has_tag_name("Item"))
        .filter_map(|n| Some((n.attribute("id")?, n.text()?)))
        .collect();

    if let Some(items) = items {
        let (sets, active) = sets_of(&items, "ItemSet", "activeItemSet");
        view.item_sets = sets.iter().map(|s| ItemSetView { id: s.attribute("id").map(str::to_string), title: title_of(s), gear: read_gear(s, &item_texts) }).collect();
        view.active.item_set = active;
    }
    if let Some(skills) = child("Skills") {
        let (sets, active) = sets_of(&skills, "SkillSet", "activeSkillSet");
        view.skill_sets = sets
            .iter()
            .enumerate()
            .map(|(i, s)| SkillSetView {
                id: s.attribute("id").map(str::to_string),
                title: title_of(s),
                groups: read_skills(s, (i == active).then_some(main_group)),
            })
            .collect();
        view.active.skill_set = active;
        view.main_skill = sets.get(active).and_then(|s| main_skill(s, main_group));
    }
    if let Some(tree) = child("Tree") {
        let specs: Vec<_> = tree.children().filter(|n| n.has_tag_name("Spec")).collect();
        view.trees = specs.iter().map(|s| read_spec(s, &item_texts)).collect();
        view.active.tree = tree
            .attribute("activeSpec")
            .and_then(|a| a.parse::<usize>().ok())
            .map(|a| a.saturating_sub(1))
            .filter(|a| *a < specs.len())
            .unwrap_or(0);
        // The game version the tree is for ("3_27"); the build's own is PoB's format.
        if let Some(v) = view.trees.get(view.active.tree).and_then(|t| t.version.clone()) {
            view.version = Some(v);
        }
    }
    if let Some(config) = child("Config") {
        let (sets, active) = sets_of(&config, "ConfigSet", "activeConfigSet");
        view.configs = sets.iter().map(|s| ConfigView { id: s.attribute("id").map(str::to_string), title: title_of(s), config: read_config(s) }).collect();
        view.active.config = active;
    }
    if let Some(notes) = child("Notes") {
        view.notes = notes.text().unwrap_or_default().trim().to_string();
    }
    view.loadouts = loadouts(&view);
    view.active.title = view
        .loadouts
        .iter()
        .find(|l| **l == Loadout { title: l.title.clone(), ..view.active.clone() })
        .map(|l| l.title.clone())
        .unwrap_or_default();
    Ok(view)
}

/// A section's sets ("ItemSet" in <Items>) and which is chosen. Older builds have no
/// sets: the section itself is the one set.
fn sets_of<'a>(section: &roxmltree::Node<'a, 'a>, tag: &str, active_attr: &str) -> (Vec<roxmltree::Node<'a, 'a>>, usize) {
    let sets: Vec<_> = section.children().filter(|n| n.has_tag_name(tag)).collect();
    if sets.is_empty() {
        return (vec![*section], 0);
    }
    let active = section.attribute(active_attr).unwrap_or("1");
    let at = sets.iter().position(|s| s.attribute("id") == Some(active)).unwrap_or(0);
    (sets, at)
}

fn title_of(set: &roxmltree::Node) -> Option<String> {
    set.attribute("title").map(str::trim).filter(|t| !t.is_empty()).map(str::to_string)
}

/// What ties sets into a loadout, as PoB does: a "{1}" tag in the name, else the
/// whole name.
fn loadout_key(title: &str) -> String {
    match (title.rfind('{'), title.rfind('}')) {
        (Some(a), Some(b)) if a < b => title[a + 1..b].trim().to_string(),
        _ => title.trim().to_string(),
    }
}

/// A loadout's name without its "{1}" tag.
fn loadout_name(title: &str) -> String {
    match (title.rfind('{'), title.rfind('}')) {
        (Some(a), Some(b)) if a < b => format!("{}{}", &title[..a], &title[b + 1..]).trim().to_string(),
        _ => title.trim().to_string(),
    }
}

/// Trees, gem sets, item sets and configs that share a name, in the trees' order.
/// Where a kind has no set of that name, the exported one stands in.
fn loadouts(view: &BuildView) -> Vec<Loadout> {
    let find = |titles: Vec<Option<&String>>, key: &str| {
        titles.iter().position(|t| t.is_some_and(|t| loadout_key(t) == key))
    };
    let mut out: Vec<Loadout> = Vec::new();
    for tree in &view.trees {
        let Some(title) = &tree.title else { continue };
        let key = loadout_key(title);
        let items = find(view.item_sets.iter().map(|s| s.title.as_ref()).collect(), &key);
        let skills = find(view.skill_sets.iter().map(|s| s.title.as_ref()).collect(), &key);
        let configs = find(view.configs.iter().map(|s| s.title.as_ref()).collect(), &key);
        if items.is_none() && skills.is_none() {
            continue; // Just a tree with a name.
        }
        if out.iter().any(|l| loadout_key(&l.title) == key) {
            continue;
        }
        out.push(Loadout {
            title: loadout_name(title),
            item_set: items.unwrap_or(view.active.item_set),
            skill_set: skills.unwrap_or(view.active.skill_set),
            tree: find(view.trees.iter().map(|t| t.title.as_ref()).collect(), &key).unwrap_or(view.active.tree),
            config: configs.unwrap_or(view.active.config),
        });
    }
    out
}

/// A gem set's groups, in order.
fn skill_set<'a>(set: &roxmltree::Node<'a, 'a>) -> Vec<roxmltree::Node<'a, 'a>> {
    set.children().filter(|n| n.has_tag_name("Skill")).collect()
}

/// A gem set's groups; `main_group` (counted from 1) marks the main skill's.
fn read_skills(set: &roxmltree::Node, main_group: Option<usize>) -> Vec<GemGroup> {
    skill_set(set)
        .iter()
        .enumerate()
        .map(|(i, s)| GemGroup {
            label: s.attribute("label").filter(|l| !l.is_empty()).map(str::to_string),
            slot: s.attribute("slot").map(str::to_string),
            enabled: s.attribute("enabled") != Some("false"),
            main: main_group == Some(i + 1),
            gems: s
                .children()
                .filter(|n| n.has_tag_name("Gem"))
                .map(|g| {
                    let skill_id = g.attribute("skillId").unwrap_or_default();
                    let gem_id = g.attribute("gemId").unwrap_or_default();
                    Gem {
                        name: g.attribute("nameSpec").unwrap_or(skill_id).to_string(),
                        level: g.attribute("level").and_then(|l| l.parse().ok()).unwrap_or(1),
                        quality: g.attribute("quality").and_then(|q| q.parse().ok()).unwrap_or(0),
                        enabled: g.attribute("enabled") != Some("false"),
                        support: skill_id.starts_with("Support") || gem_id.contains("/SupportGem"),
                        colour: None,
                        variant_id: g.attribute("variantId").unwrap_or(skill_id).to_string(),
                    }
                })
                .collect(),
        })
        .collect()
}

/// The main group's chosen active gem ("mainActiveSkill" counts its active gems).
fn main_skill(set: &roxmltree::Node, main_group: usize) -> Option<String> {
    let group = skill_set(set).into_iter().nth(main_group.checked_sub(1)?)?;
    let pick: usize = group.attribute("mainActiveSkill").and_then(|m| m.parse().ok()).unwrap_or(1);
    group
        .children()
        .filter(|n| n.has_tag_name("Gem"))
        .filter(|g| {
            let id = g.attribute("skillId").unwrap_or_default();
            !id.starts_with("Support") && g.attribute("enabled") != Some("false")
        })
        .nth(pick.saturating_sub(1))
        .and_then(|g| g.attribute("nameSpec"))
        .map(str::to_string)
}

/// An item set's equipped items, in PoB's slots.
fn read_gear(set: &roxmltree::Node, texts: &HashMap<&str, &str>) -> Vec<GearItem> {
    let mut gear = Vec::new();
    for slot in set.children().filter(|n| n.has_tag_name("Slot")) {
        let (Some(name), Some(id)) = (slot.attribute("name"), slot.attribute("itemId")) else { continue };
        if id == "0" || name.contains("Swap") || name.contains("Abyssal Socket") {
            continue;
        }
        if let Some(text) = texts.get(id) {
            gear.push(GearItem { slot: name.to_string(), item: read_pob_item(text) });
        }
    }
    gear
}

/// An item as PoB writes it: "Rarity: RARE", name, base, then "Key: value" lines up to
/// "Implicits: N", then N implicit mods and the explicit ones, with tags such as
/// {crafted} in front.
pub fn read_pob_item(text: &str) -> HitItem {
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let mut item = HitItem { width: 1, height: 1, rarity: "normal", ..Default::default() };
    let mut i = 0;
    let rarity = lines.first().and_then(|l| l.strip_prefix("Rarity: ")).unwrap_or("NORMAL");
    if lines.first().is_some_and(|l| l.starts_with("Rarity:")) {
        i = 1;
    }
    item.rarity = match rarity {
        "MAGIC" => "magic",
        "RARE" => "rare",
        "UNIQUE" => "unique",
        "RELIC" => "relic",
        _ => "normal",
    };
    if matches!(rarity, "RARE" | "UNIQUE" | "RELIC") {
        item.name = lines.get(i).copied().unwrap_or_default().to_string();
        item.type_line = lines.get(i + 1).copied().unwrap_or_default().to_string();
        i += 2;
    } else {
        item.type_line = lines.get(i).copied().unwrap_or_default().to_string();
        i += 1;
    }

    let mut implicits = 0;
    let mut selected_variant: Option<String> = None;
    let mut percentile = None;
    while i < lines.len() {
        let line = lines[i];
        i += 1;
        if let Some(n) = line.strip_prefix("Implicits: ") {
            implicits = n.parse().unwrap_or(0);
            break;
        }
        let (key, value) = line.split_once(": ").unwrap_or((line, ""));
        match key {
            "Item Level" => item.item_level = value.parse().ok(),
            "Quality" => item.properties.push(prop("Quality", &format!("+{value}%"), Some("quality"))),
            "Sockets" => item.sockets = Some(value.trim().to_string()),
            "LevelReq" => item.requirements.push(HitRequirement { name: "Level".into(), value: value.into(), sort: Some("lvl") }),
            "Armour" => item.properties.push(prop("Armour", value, Some("ar"))),
            "Evasion" => item.properties.push(prop("Evasion Rating", value, Some("ev"))),
            "Energy Shield" => item.properties.push(prop("Energy Shield", value, Some("es"))),
            "Ward" => item.properties.push(prop("Ward", value, Some("ward"))),
            "Selected Variant" => selected_variant = Some(value.to_string()),
            "Corrupted" => item.corrupted = true,
            "Mirrored" => item.mirrored = true,
            "Unidentified" => item.unidentified = true,
            k if k.ends_with("BasePercentile") => percentile = value.parse::<f64>().ok(),
            _ => {}
        }
    }
    if let Some(p) = percentile {
        item.extended.push(HitProperty {
            label: "Base Percentile".into(),
            value: Some(format!("{:.0}%", p * 100.0)),
            augmented: false,
            sort: Some("base_defence_percentile"),
        });
    }

    let mut n = 0;
    for line in &lines[i.min(lines.len())..] {
        match *line {
            "Corrupted" => {
                item.corrupted = true;
                continue;
            }
            "Mirrored" => {
                item.mirrored = true;
                continue;
            }
            _ => {}
        }
        let (tags, text) = split_tags(line);
        n += 1;
        let implicit = n <= implicits;
        let variants = tags.iter().find_map(|t| t.strip_prefix("variant:"));
        if variants.is_some_and(|v| !selected_variant.as_deref().is_some_and(|s| v.split(',').any(|x| x == s))) {
            continue;
        }
        if text.is_empty() {
            continue;
        }
        let kind = if tags.contains(&"enchant") {
            "enchant"
        } else if tags.iter().any(|t| *t == "crafted" || *t == "custom") {
            "crafted"
        } else if tags.contains(&"fractured") {
            "fractured"
        } else if tags.contains(&"mutated") {
            "mutated"
        } else if implicit {
            "implicit"
        } else {
            "explicit"
        };
        item.mods.push(HitMod { text: text.to_string(), kind, range: None, sort: None });
    }
    item
}

fn prop(label: &str, value: &str, sort: Option<&'static str>) -> HitProperty {
    HitProperty { label: label.into(), value: Some(value.trim().into()), augmented: false, sort }
}

/// "{crafted}{range:0.5}text" → (["crafted", "range:0.5"], "text").
fn split_tags(line: &str) -> (Vec<&str>, &str) {
    let mut tags = Vec::new();
    let mut rest = line;
    while let Some(body) = rest.strip_prefix('{') {
        let Some(end) = body.find('}') else { break };
        tags.push(&body[..end]);
        rest = &body[end + 1..];
    }
    (tags, rest.trim())
}

/// A passive tree (PoB's "Spec"): its nodes, chosen masteries and jewels.
fn read_spec(spec: &roxmltree::Node, texts: &HashMap<&str, &str>) -> TreeView {
    let nodes = spec
        .attribute("nodes")
        .unwrap_or_default()
        .split(',')
        .filter_map(|n| n.trim().parse().ok())
        .collect();
    // "{4492,5356},{…}": a mastery node and its chosen effect.
    let mastery_effects = spec
        .attribute("masteryEffects")
        .unwrap_or_default()
        .split('}')
        .filter_map(|p| {
            let (node, effect) = p.trim_start_matches([',', '{']).split_once(',')?;
            Some((node.trim().parse().ok()?, effect.trim().parse().ok()?))
        })
        .collect();
    let jewels = spec
        .descendants()
        .filter(|n| n.has_tag_name("Socket"))
        .filter_map(|s| texts.get(s.attribute("itemId").filter(|id| *id != "0")?))
        .map(|text| GearItem { slot: "Jewel".into(), item: read_pob_item(text) })
        .collect();
    TreeView {
        title: title_of(spec),
        version: spec.attribute("treeVersion").map(str::to_string),
        jewels,
        nodes,
        mastery_effects,
        ..Default::default()
    }
}

/// A config set's summary, as pobb.in gives it.
fn read_config(set: &roxmltree::Node) -> Vec<String> {
    let inputs: HashMap<&str, roxmltree::Node> = set
        .children()
        .filter(|n| n.has_tag_name("Input"))
        .filter_map(|n| Some((n.attribute("name")?, n)))
        .collect();
    let mut out = Vec::new();
    if let Some(boss) = inputs.get("enemyIsBoss").and_then(|n| n.attribute("string")).filter(|b| *b != "None") {
        out.push(boss.to_string());
    }
    if let Some(shock) = inputs.get("conditionShockEffect").and_then(|n| n.attribute("number")) {
        out.push(format!("{shock}% Shock"));
    }
    if inputs.get("customMods").and_then(|n| n.attribute("string")).is_some_and(|m| !m.trim().is_empty()) {
        out.push("Custom Mods".into());
    }
    out
}

/* Stats for other loadouts, from Path of Building */

/// The build's XML with these sets chosen, as if picked in PoB: what PoB then
/// calculates is that loadout.
pub fn with_sets(xml: &str, view: &BuildView, pick: &Loadout) -> String {
    let mut xml = xml.to_string();
    if let Some(id) = view.item_sets.get(pick.item_set).and_then(|s| s.id.as_deref()) {
        xml = set_attr(&xml, "Items", "activeItemSet", id);
    }
    if let Some(id) = view.skill_sets.get(pick.skill_set).and_then(|s| s.id.as_deref()) {
        xml = set_attr(&xml, "Skills", "activeSkillSet", id);
    }
    if view.trees.len() > 1 {
        xml = set_attr(&xml, "Tree", "activeSpec", &(pick.tree + 1).to_string());
    }
    if let Some(id) = view.configs.get(pick.config).and_then(|s| s.id.as_deref()) {
        xml = set_attr(&xml, "Config", "activeConfigSet", id);
    }
    xml
}

/// Sets an attribute on the first `<tag …>` element, adding it when missing.
fn set_attr(xml: &str, tag: &str, attr: &str, value: &str) -> String {
    let Some(start) = [format!("<{tag} "), format!("<{tag}>")].iter().filter_map(|t| xml.find(t.as_str())).min() else {
        return xml.to_string();
    };
    let end = start + xml[start..].find('>').unwrap_or(0);
    let open = &xml[start..end];
    let needle = format!(" {attr}=\"");
    let new_open = match open.find(&needle) {
        Some(at) => {
            let from = at + needle.len();
            let to = from + open[from..].find('"').unwrap_or(0);
            format!("{}{}{}", &open[..from], value, &open[to..])
        }
        None => format!("{} {attr}=\"{value}\"", open.trim_end_matches('/')) + if open.ends_with('/') { "/" } else { "" },
    };
    format!("{}{}{}", &xml[..start], new_open, &xml[end..])
}

/// Stats from the Path of Building service, as PoB lists them in an export.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Calculated {
    pub stats: BTreeMap<String, f64>,
    /// The PoB version the service runs ("2.67.2").
    pub pob_version: Option<String>,
}

/// Where Path of Building calculations run: PoE Tool's own service, its address added
/// when the app is built (the `POB_SERVICE_URL` variable), so it can move without a
/// change here. A setting can point elsewhere (a local copy, for testing).
pub const SERVICE_URL: Option<&str> = option_env!("POB_SERVICE_URL");

/// The service's key for this release, added when the app is built (the
/// `POB_SERVICE_KEY` variable). It ships in the app, so it's no real secret, but it
/// stays out of the source; a release can move to a new one.
pub const SERVICE_KEY: Option<&str> = option_env!("POB_SERVICE_KEY");

/// How often a busy or rate-limited service is asked again, and the longest wait.
const SERVICE_RETRIES: usize = 2;
const SERVICE_MAX_WAIT: Duration = Duration::from_secs(20);

/// Asks the PoB service for a build's stats. `agent` names the app and its version
/// ("PoE-Tool/0.1.30"), as the service logs it.
pub async fn calculate(http: &reqwest::Client, url: &str, key: &str, agent: &str, xml: &str) -> Result<Calculated, String> {
    let mut tries = 0;
    let (status, body) = loop {
        let mut req = http
            .post(format!("{}/v1/calc", url.trim_end_matches('/')))
            .header(reqwest::header::USER_AGENT, agent)
            // The first calculation on a fresh service waits for PoB to start.
            .timeout(Duration::from_secs(90))
            .json(&serde_json::json!({ "code": xml }));
        if !key.trim().is_empty() {
            req = req.bearer_auth(key.trim());
        }
        let resp = req.send().await.map_err(|e| format!("Couldn't reach the PoB service: {e}"))?;
        let status = resp.status();
        // Busy (503) or too many calculations (429): it says when to come back.
        if matches!(status.as_u16(), 429 | 503) && tries < SERVICE_RETRIES {
            let wait = resp
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.trim().parse::<u64>().ok())
                .map(Duration::from_secs)
                .unwrap_or(Duration::from_secs(2))
                .min(SERVICE_MAX_WAIT);
            tries += 1;
            tokio::time::sleep(wait).await;
            continue;
        }
        let body: serde_json::Value = resp.json().await.map_err(|e| format!("The PoB service answered oddly: {e}"))?;
        break (status, body);
    };
    if !status.is_success() {
        let msg = body["error"]["message"].as_str().unwrap_or("unknown error");
        return Err(match status.as_u16() {
            429 => "Path of Building is busy with too many calculations from here. Try again in a minute.".into(),
            503 => "The PoB service is busy. Try again in a moment.".into(),
            401 => "The PoB service didn't accept this version of PoE Tool. An update may help.".into(),
            _ => format!("PoB couldn't calculate it: {msg}"),
        });
    }
    let stats = body["playerStats"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| Some((s["stat"].as_str()?.to_string(), s["value"].as_f64()?)))
        .collect();
    Ok(Calculated { stats, pob_version: body["pobVersion"].as_str().map(str::to_string) })
}

/* What the build is shown with: item pictures, gem colours, the tree's names */

/// Item pictures, gem colours and the passive tree's names, fetched once a week.
#[derive(Default)]
pub struct BuildAssets {
    loaded: Mutex<Option<Arc<Assets>>>,
}

#[derive(Default)]
struct Assets {
    /// Unique name → picture.
    uniques: HashMap<String, String>,
    /// Base type → picture.
    bases: HashMap<String, String>,
    /// Longest first, to find the base in a magic item's name.
    base_names: Vec<String>,
    /// Gem variant id → colour.
    gem_colours: HashMap<String, &'static str>,
    tree: TreeIndex,
}

#[derive(Default)]
struct TreeIndex {
    nodes: HashMap<u32, TreeNode>,
}

struct TreeNode {
    name: String,
    keystone: bool,
    notable: bool,
    ascendancy: bool,
    /// The class start or ascendancy start: not a point spent.
    start: bool,
    masteries: HashMap<u32, String>,
}

const WEEK: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const NINJA: &str = "https://poe.ninja/poe1/api/economy/stash/current/item/overview?league=Standard&type=";
const GEMS: &str = "https://raw.githubusercontent.com/PathOfBuildingCommunity/PathOfBuilding/master/src/Data/Gems.lua";
const TREE: &str = "https://raw.githubusercontent.com/grindinggear/skilltree-export/master/data.json";

impl BuildAssets {
    async fn get(&self, price: &PriceService) -> Arc<Assets> {
        if let Some(a) = self.loaded.lock().unwrap().clone() {
            return a;
        }
        let assets = Arc::new(load_assets(price).await);
        *self.loaded.lock().unwrap() = Some(assets.clone());
        assets
    }

    /// Fills in pictures, gem colours and the tree's names. What can't be fetched is
    /// left out; the rest of the build shows anyway.
    pub async fn dress(&self, price: &PriceService, view: &mut BuildView) {
        let a = self.get(price).await;
        let gear = view.item_sets.iter_mut().flat_map(|s| s.gear.iter_mut());
        let jewels = view.trees.iter_mut().flat_map(|t| t.jewels.iter_mut());
        for g in gear.chain(jewels) {
            let item = &mut g.item;
            let icon = if item.rarity == "unique" || item.rarity == "relic" {
                a.uniques.get(&item.name).or_else(|| a.bases.get(&item.type_line))
            } else if item.rarity == "magic" {
                a.base_names.iter().find(|b| item.type_line.contains(b.as_str())).and_then(|b| a.bases.get(b))
            } else {
                a.bases.get(&item.type_line)
            };
            item.icon = icon.cloned();
            let (w, h) = item.icon.as_deref().and_then(icon_size).unwrap_or_else(|| slot_size(&g.slot));
            item.width = w;
            item.height = h;
        }
        for gem in view.skill_sets.iter_mut().flat_map(|s| s.groups.iter_mut()).flat_map(|g| g.gems.iter_mut()) {
            gem.colour = a.gem_colours.get(&gem.variant_id).copied();
        }
        for t in &mut view.trees {
            for id in &t.nodes {
                let Some(node) = a.tree.nodes.get(id) else { continue };
                if node.start {
                    continue;
                }
                if node.ascendancy {
                    t.ascendancy_points += 1;
                    if node.notable {
                        t.ascendancy_notables.push(node.name.clone());
                    }
                    continue;
                }
                t.points += 1;
                if node.keystone {
                    t.keystones.push(node.name.clone());
                } else if node.notable {
                    t.notables.push(node.name.clone());
                }
            }
            for (node, effect) in &t.mastery_effects {
                if let Some((n, text)) = a.tree.nodes.get(node).and_then(|n| Some((n, n.masteries.get(effect)?))) {
                    t.masteries.push(format!("{}: {text}", n.name));
                }
            }
        }
    }
}

async fn load_assets(price: &PriceService) -> Assets {
    let mut a = Assets::default();
    let json = |bytes: Result<Vec<u8>, String>| bytes.ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok());
    for kind in ["UniqueWeapon", "UniqueArmour", "UniqueAccessory", "UniqueFlask", "UniqueJewel"] {
        let file = format!("ninja-{kind}.json");
        if let Some(v) = json(price.cached_url(&file, &format!("{NINJA}{kind}"), WEEK).await) {
            for line in v["lines"].as_array().into_iter().flatten() {
                if let (Some(name), Some(icon)) = (line["name"].as_str(), line["icon"].as_str()) {
                    a.uniques.entry(name.to_string()).or_insert_with(|| icon.to_string());
                }
            }
        }
    }
    if let Some(v) = json(price.cached_url("ninja-BaseType.json", &format!("{NINJA}BaseType"), WEEK).await) {
        for line in v["lines"].as_array().into_iter().flatten() {
            if let (Some(name), Some(icon)) = (line["name"].as_str(), line["icon"].as_str()) {
                a.bases.entry(name.to_string()).or_insert_with(|| icon.to_string());
            }
        }
    }
    // Cluster jewels aren't base types there; their own list has the bases' pictures.
    if let Some(v) = json(price.cached_url("ninja-ClusterJewel.json", &format!("{NINJA}ClusterJewel"), WEEK).await) {
        for line in v["lines"].as_array().into_iter().flatten() {
            if let (Some(base), Some(icon)) = (line["baseType"].as_str(), line["icon"].as_str()) {
                a.bases.entry(base.to_string()).or_insert_with(|| icon.to_string());
            }
        }
    }
    a.base_names = a.bases.keys().cloned().collect();
    a.base_names.sort_by_key(|b| std::cmp::Reverse(b.len()));
    if let Ok(bytes) = price.cached_url("pob-gems.lua", GEMS, WEEK).await {
        a.gem_colours = gem_colours(&String::from_utf8_lossy(&bytes));
    }
    if let Some(v) = json(price.cached_url("passive-tree.json", TREE, WEEK).await) {
        a.tree = tree_index(&v);
    }
    a
}

/// Each gem's colour from PoB's gem list: the attribute it needs most, white for none.
fn gem_colours(lua: &str) -> HashMap<String, &'static str> {
    let mut out = HashMap::new();
    for block in lua.split("\n\t[\"Metadata/").skip(1) {
        let field = |name: &str| -> Option<&str> {
            let at = block.find(&format!("\n\t\t{name} = "))? + name.len() + 6;
            let rest = &block[at..];
            Some(rest[..rest.find(',')?].trim().trim_matches('"'))
        };
        let Some(variant) = field("variantId") else { continue };
        let req = |n: &str| field(n).and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
        let (s, d, i) = (req("reqStr"), req("reqDex"), req("reqInt"));
        let colour = if s == 0 && d == 0 && i == 0 {
            "white"
        } else if s >= d && s >= i {
            "red"
        } else if d >= i {
            "green"
        } else {
            "blue"
        };
        out.insert(variant.to_string(), colour);
    }
    out
}

fn tree_index(v: &serde_json::Value) -> TreeIndex {
    let mut nodes = HashMap::new();
    for (id, n) in v["nodes"].as_object().into_iter().flatten() {
        let Ok(id) = id.parse::<u32>() else { continue };
        let masteries = n["masteryEffects"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|e| {
                let stats: Vec<&str> = e["stats"].as_array()?.iter().filter_map(|s| s.as_str()).collect();
                Some((e["effect"].as_u64()? as u32, stats.join(", ")))
            })
            .collect();
        nodes.insert(
            id,
            TreeNode {
                name: n["name"].as_str().unwrap_or_default().to_string(),
                keystone: n["isKeystone"].as_bool() == Some(true),
                notable: n["isNotable"].as_bool() == Some(true),
                ascendancy: n["ascendancyName"].is_string(),
                start: n["classStartIndex"].is_number() || n["isAscendancyStart"].as_bool() == Some(true),
                masteries,
            },
        );
    }
    TreeIndex { nodes }
}

/// An item picture's size in cells, from the picture's address: the part after
/// /gen/image/ is base64 of [25, 14, {"w": 2, "h": 3, …}].
fn icon_size(url: &str) -> Option<(u64, u64)> {
    let part = url.split("/gen/image/").nth(1)?.split('/').next()?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(part.trim_end_matches('='))
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(part.trim_end_matches('=')))
        .ok()?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let info = v.as_array()?.iter().find(|x| x.is_object())?;
    Some((info["w"].as_u64()?, info["h"].as_u64()?))
}

/// What a slot's item usually measures, when its picture can't say.
fn slot_size(slot: &str) -> (u64, u64) {
    match slot {
        s if s.starts_with("Weapon") => (2, 4),
        "Body Armour" => (2, 3),
        "Helmet" | "Gloves" | "Boots" => (2, 2),
        "Belt" => (2, 1),
        s if s.starts_with("Flask") => (1, 2),
        s if s.starts_with("Graft") => (1, 2),
        _ => (1, 1),
    }
}

/// Names a build from what it is: "Ball Lightning of Orbiting Hierophant".
pub fn title(view: &BuildView) -> String {
    let who = view.ascendancy.clone().unwrap_or_else(|| view.class.clone());
    match &view.main_skill {
        Some(skill) => format!("{skill} {who}"),
        None => who,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_raw_links() {
        assert_eq!(raw_url("https://pobb.in/7dXamrP_6CiI").as_deref(), Some("https://pobb.in/7dXamrP_6CiI/raw"));
        assert_eq!(raw_url("pobb.in/abc/raw").as_deref(), Some("https://pobb.in/abc/raw"));
        assert_eq!(raw_url("https://pastebin.com/XyZ12").as_deref(), Some("https://pastebin.com/raw/XyZ12"));
        assert_eq!(raw_url("https://poe.ninja/pob/1a2b").as_deref(), Some("https://poe.ninja/pob/raw/1a2b"));
        assert_eq!(raw_url("eNrtfVtznMa28HPmV1Cqyvdi"), None);
    }

    #[test]
    fn reads_pob_items() {
        let text = "Rarity: RARE\nHate Tread\nWarlock Boots\nEnergy Shield: 171\nEnergyShieldBasePercentile: 0.8997\n\
                    Unique ID: abc\nItem Level: 84\nQuality: 20\nSockets: W-W-W B\nLevelReq: 84\nImplicits: 2\n\
                    9% increased Cooldown Recovery Rate\n{enchant}5% increased Action Speed\n\
                    {fractured}+73 to maximum Mana\n+53 to Strength\n{crafted}{range:0.5}74% increased Energy Shield\nCorrupted";
        let item = read_pob_item(text);
        assert_eq!(item.rarity, "rare");
        assert_eq!(item.name, "Hate Tread");
        assert_eq!(item.type_line, "Warlock Boots");
        assert_eq!(item.item_level, Some(84));
        assert_eq!(item.sockets.as_deref(), Some("W-W-W B"));
        assert!(item.corrupted);
        let kinds: Vec<_> = item.mods.iter().map(|m| (m.kind, m.text.as_str())).collect();
        assert_eq!(
            kinds,
            vec![
                ("implicit", "9% increased Cooldown Recovery Rate"),
                ("enchant", "5% increased Action Speed"),
                ("fractured", "+73 to maximum Mana"),
                ("explicit", "+53 to Strength"),
                ("crafted", "74% increased Energy Shield"),
            ]
        );
        assert_eq!(item.extended[0].value.as_deref(), Some("90%"));
    }

    #[test]
    fn keeps_the_selected_variant() {
        let text = "Rarity: UNIQUE\nX\nY\nVariant: Old\nVariant: New\nSelected Variant: 2\nImplicits: 0\n\
                    {variant:1}Old line\n{variant:2}New line\nBoth";
        let mods: Vec<_> = read_pob_item(text).mods.into_iter().map(|m| m.text).collect();
        assert_eq!(mods, vec!["New line", "Both"]);
    }

    #[test]
    fn colours_gems() {
        let lua = "return {\n\t[\"Metadata/Items/Gems/SkillGemFireball\"] = {\n\t\tname = \"Fireball\",\n\t\tvariantId = \"Fireball\",\n\t\treqStr = 0,\n\t\treqDex = 0,\n\t\treqInt = 100,\n\t},\n\t[\"Metadata/Items/Gems/SupportGemEnhance\"] = {\n\t\tvariantId = \"SupportEnhance\",\n\t\treqStr = 0,\n\t\treqDex = 0,\n\t\treqInt = 0,\n\t},\n}";
        let c = gem_colours(lua);
        assert_eq!(c.get("Fireball"), Some(&"blue"));
        assert_eq!(c.get("SupportEnhance"), Some(&"white"));
    }

    #[test]
    fn reads_icon_sizes() {
        let url = "https://web.poecdn.com/gen/image/WzI1LDE0LHsiZiI6IjJESXRlbXMvQXJtb3Vycy9Cb2R5QXJtb3Vycy9FbWJyYWNlIiwidyI6MiwiaCI6Mywic2NhbGUiOjEsIm11dGF0ZWQiOnRydWV9XQ/edcf542067/Embrace.png";
        assert_eq!(icon_size(url), Some((2, 3)));
    }

    /// Against the real sources: `cargo test dresses_a_build -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore]
    async fn dresses_a_build() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/build1.code");
        let code = std::fs::read_to_string(path).unwrap();
        let mut view = parse(&decode(&code).unwrap()).unwrap();
        let price = PriceService::new(std::env::temp_dir().join("poe-tool-build-test"));
        BuildAssets::default().dress(&price, &mut view).await;
        let active = view.active.clone();
        let gear = &view.item_sets[active.item_set].gear;
        let tree = &view.trees[active.tree];
        for g in gear.iter().chain(&tree.jewels) {
            println!("{:<12} {:<28} {}x{} icon={}", g.slot, format!("{} {}", g.item.name, g.item.type_line), g.item.width, g.item.height, g.item.icon.is_some());
        }
        for group in &view.skill_sets[active.skill_set].groups {
            println!("{} {:?}", if group.main { "*" } else { " " }, group.gems.iter().map(|g| format!("{}({:?})", g.name, g.colour)).collect::<Vec<_>>());
        }
        println!("points {} asc {} keystones {:?}", tree.points, tree.ascendancy_points, tree.keystones);
        println!("asc notables {:?}", tree.ascendancy_notables);
        println!("masteries {:?}", tree.masteries);
        // poe.ninja has no pictures for magic flasks or grafts.
        let missing: Vec<_> = gear.iter().chain(&tree.jewels).filter(|g| g.item.icon.is_none()).map(|g| g.slot.as_str()).collect();
        assert!(missing.iter().all(|s| s.starts_with("Flask") || s.starts_with("Graft")), "{missing:?}");
        assert!(view.skill_sets.iter().flat_map(|s| &s.groups).flat_map(|g| &g.gems).all(|g| g.colour.is_some()));
        assert!(!tree.keystones.is_empty());
    }

    /// A build with twelve loadouts (pobb.in/OvzQ5WyXeQTF), saved next to the tests.
    #[test]
    fn reads_loadouts() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/build2.code");
        let Ok(code) = std::fs::read_to_string(path) else { return };
        let view = parse(&decode(&code).unwrap()).unwrap();
        assert_eq!(view.trees.len(), 12);
        assert_eq!(view.loadouts.len(), 12);
        assert_eq!(view.loadouts[0].title, "Lvl 1-12 (Spectral Throw)");
        assert_eq!(view.active.title, "Lvl 96");
        let l = view.loadouts.iter().find(|l| l.title == "Lvl 96").unwrap();
        assert_eq!(view.trees[l.tree].title.as_deref(), Some("Lvl 96"));
        assert_eq!(view.skill_sets[l.skill_set].title.as_deref(), Some("Lvl 96"));
        assert_eq!(*l, view.active);
    }

    #[test]
    fn chooses_sets_in_the_xml() {
        let xml = r#"<PathOfBuilding><Items activeItemSet="2" x="1"/><Skills activeSkillSet="1"></Skills><Tree activeSpec="1"></Tree><Config></Config></PathOfBuilding>"#;
        assert_eq!(set_attr(xml, "Items", "activeItemSet", "5"), xml.replace(r#"activeItemSet="2""#, r#"activeItemSet="5""#));
        let added = set_attr(xml, "Config", "activeConfigSet", "3");
        assert!(added.contains(r#"<Config activeConfigSet="3">"#), "{added}");
        let self_closing = set_attr(r#"<a><Items x="1"/></a>"#, "Items", "activeItemSet", "4");
        assert_eq!(self_closing, r#"<a><Items x="1" activeItemSet="4"/></a>"#);
    }

    #[test]
    fn picks_a_loadout_in_the_xml() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/build2.code");
        let Ok(code) = std::fs::read_to_string(path) else { return };
        let xml = decode(&code).unwrap();
        let view = parse(&xml).unwrap();
        let first = view.loadouts[0].clone();
        let picked = parse(&with_sets(&xml, &view, &first)).unwrap();
        assert_eq!(picked.active, first);
    }

    /// Against a running service: `POB_SERVICE=http://localhost:18090 POB_KEY=… cargo test calculates_loadouts -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore]
    async fn calculates_loadouts() {
        let url = std::env::var("POB_SERVICE").expect("POB_SERVICE");
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/build2.code");
        let xml = decode(&std::fs::read_to_string(path).unwrap()).unwrap();
        let view = parse(&xml).unwrap();
        let key = std::env::var("POB_KEY").unwrap_or_default();
        let http = reqwest::Client::new();
        for pick in [&view.active, &view.loadouts[0]] {
            let c = calculate(&http, &url, &key, "PoE-Tool/test", &with_sets(&xml, &view, pick)).await.unwrap();
            println!(
                "{:<28} Life {:>6} ES {:>5} FullDPS {:>12} CombinedDPS {:>12} (PoB {:?})",
                pick.title, c.stats.get("Life").copied().unwrap_or(0.0), c.stats.get("EnergyShield").copied().unwrap_or(0.0),
                c.stats.get("FullDPS").copied().unwrap_or(0.0).round(), c.stats.get("CombinedDPS").copied().unwrap_or(0.0).round(), c.pob_version
            );
        }
        println!("exported: Life {:?} FullDPS {:?}", view.stats.get("Life"), view.stats.get("FullDPS"));
    }

    #[test]
    fn reads_loadout_tags() {
        assert_eq!(loadout_key("Mapping {2}"), "2");
        assert_eq!(loadout_name("Mapping {2}"), "Mapping");
        assert_eq!(loadout_key("Lvl 96"), "Lvl 96");
    }

    /// A real build (pobb.in/7dXamrP_6CiI), saved next to the tests.
    #[test]
    fn reads_a_whole_build() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/build1.code");
        let Ok(code) = std::fs::read_to_string(path) else { return };
        let view = parse(&decode(&code).unwrap()).unwrap();
        assert_eq!(view.class, "Templar");
        assert_eq!(view.ascendancy.as_deref(), Some("Hierophant"));
        assert_eq!(view.level, 98);
        assert_eq!(view.stats.get("TotalDPS").map(|d| d.round()), Some(75719981.0));
        assert_eq!(view.main_skill.as_deref(), Some("Ball Lightning of Orbiting"));
        let a = &view.active;
        assert!(view.item_sets[a.item_set].gear.iter().any(|g| g.slot == "Helmet" && g.item.name == "Indigon"));
        assert!(view.skill_sets[a.skill_set].groups.iter().any(|g| g.main));
        assert!(view.trees[a.tree].nodes.len() > 100);
        assert!(view.trees[a.tree].jewels.len() >= 10);
        assert_eq!(view.configs[a.config].config, vec!["Boss", "30% Shock", "Custom Mods"]);
        assert!(view.loadouts.is_empty());
    }
}
