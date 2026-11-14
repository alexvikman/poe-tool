//! Saved trade searches, grouped (a group is usually a build: "Shockwave Cyclone").
//!
//! A search keeps the trade site's own query, so it can be run, opened on the site or
//! watched live. Links from the trade site import directly: the id in a search link
//! is the query itself, gzip-compressed and base64url-encoded.

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::Read;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchGroup {
    /// Stable, so a build can refer to its group later.
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub searches: Vec<SavedSearch>,
    /// The build from Path of Building, once imported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<crate::builds::StoredBuild>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedSearch {
    pub id: String,
    pub name: String,
    pub league: String,
    /// The request body for `/api/trade/search`: `{"query": …, "sort": …}`.
    pub body: Value,
    /// Watched with the trade site's live search.
    #[serde(default)]
    pub live: bool,
}

pub struct SearchStore {
    path: PathBuf,
    pub groups: Vec<SearchGroup>,
}

impl SearchStore {
    pub fn load(path: PathBuf) -> Self {
        let groups = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        SearchStore { path, groups }
    }

    pub fn save(&self) -> Result<(), String> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_vec_pretty(&self.groups).map_err(|e| e.to_string())?;
        std::fs::write(&self.path, json).map_err(|e| e.to_string())
    }

    pub fn add_group(&mut self, name: &str) -> Result<SearchGroup, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Give the build a name.".into());
        }
        let group = SearchGroup { id: new_id(), name: name.to_string(), searches: Vec::new(), build: None };
        self.groups.push(group.clone());
        self.save()?;
        Ok(group)
    }

    pub fn group_mut(&mut self, id: &str) -> Result<&mut SearchGroup, String> {
        self.groups.iter_mut().find(|g| g.id == id).ok_or_else(|| "That build is gone.".to_string())
    }

    pub fn search_mut(&mut self, id: &str) -> Result<&mut SavedSearch, String> {
        self.groups
            .iter_mut()
            .flat_map(|g| g.searches.iter_mut())
            .find(|s| s.id == id)
            .ok_or_else(|| "That search is gone.".to_string())
    }

    /// A copy of a search, right after it in its build: "Name (copy)", not live.
    pub fn duplicate(&mut self, id: &str) -> Result<SavedSearch, String> {
        let group = self
            .groups
            .iter_mut()
            .find(|g| g.searches.iter().any(|s| s.id == id))
            .ok_or("That search is gone.")?;
        let at = group.searches.iter().position(|s| s.id == id).unwrap();
        let mut copy = group.searches[at].clone();
        copy.id = new_id();
        copy.name = format!("{} (copy)", copy.name);
        copy.live = false;
        group.searches.insert(at + 1, copy.clone());
        self.save()?;
        Ok(copy)
    }

    /// Moves a search to another build, at the end.
    pub fn move_search(&mut self, id: &str, group_id: &str) -> Result<(), String> {
        if self.find(id).is_some_and(|(g, _)| g.id == group_id) {
            return Ok(());
        }
        self.group_mut(group_id)?;
        let mut moved = None;
        for g in self.groups.iter_mut() {
            if let Some(i) = g.searches.iter().position(|s| s.id == id) {
                moved = Some(g.searches.remove(i));
            }
        }
        let search = moved.ok_or("That search is gone.")?;
        self.group_mut(group_id)?.searches.push(search);
        Ok(())
    }

    pub fn find(&self, id: &str) -> Option<(&SearchGroup, &SavedSearch)> {
        self.groups
            .iter()
            .find_map(|g| g.searches.iter().find(|s| s.id == id).map(|s| (g, s)))
    }

    pub fn add_search(&mut self, group_id: &str, mut search: SavedSearch) -> Result<SavedSearch, String> {
        if search.name.trim().is_empty() {
            search.name = default_name(&search.body);
        }
        let group = self.group_mut(group_id)?;
        group.searches.push(search.clone());
        self.save()?;
        Ok(search)
    }

    /// Every watched search, with its build's name.
    pub fn live_searches(&self) -> Vec<(String, SavedSearch)> {
        self.groups
            .iter()
            .flat_map(|g| g.searches.iter().filter(|s| s.live).map(|s| (g.name.clone(), s.clone())))
            .collect()
    }
}

pub fn new_id() -> String {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("{nanos:x}")
}

/// A saved search from a trade site link:
/// `https://www.pathofexile.com/trade/search/<League>/<id>` (also with `/live` after).
pub fn from_link(link: &str, name: &str) -> Result<SavedSearch, String> {
    let rest = link
        .trim()
        .split("/trade/search/")
        .nth(1)
        .ok_or("That isn't a trade search link (…/trade/search/<league>/<id>).")?;
    let mut parts = rest.split(['/', '?', '#']);
    let league = percent_decode(parts.next().unwrap_or_default());
    let id = parts.next().unwrap_or_default();
    if league.is_empty() || id.is_empty() {
        return Err("That trade link has no league or search in it.".into());
    }
    let query = decode_search_id(id)?;
    let body = json!({ "query": query, "sort": { "price": "asc" } });
    Ok(SavedSearch { id: new_id(), name: name.trim().to_string(), league, body, live: false })
}

/// The query inside a search id: base64url of gzip of the query's JSON.
pub fn decode_search_id(id: &str) -> Result<Value, String> {
    let unreadable = || "This search link is in an older form the tool can't read. Open it on the trade site, \
                         press Search, and copy the new link."
        .to_string();
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(id.trim_end_matches('='))
        .map_err(|_| unreadable())?;
    let mut json = String::new();
    flate2::read::GzDecoder::new(&bytes[..])
        .read_to_string(&mut json)
        .map_err(|_| unreadable())?;
    serde_json::from_str(&json).map_err(|_| unreadable())
}

/// What a search looks for, as the trade site shows it: "The Ignomon Gold Amulet",
/// "Two-Toned Boots", "Two-Handed Sword" (a category), or the typed search text.
pub fn default_name(body: &Value) -> String {
    let q = &body["query"];
    let text = |v: &Value| match v {
        Value::String(s) => Some(s.trim().to_string()),
        Value::Object(o) => o.get("option").and_then(|v| v.as_str()).map(|s| s.trim().to_string()),
        _ => None,
    }
    .filter(|s| !s.is_empty());
    let item = [text(&q["name"]), text(&q["type"])].into_iter().flatten().collect::<Vec<_>>().join(" ");
    if !item.is_empty() {
        return item;
    }
    if let Some(term) = text(&q["term"]) {
        return term;
    }
    q["filters"]["type_filters"]["filters"]["category"]["option"]
        .as_str()
        .map(|c| crate::price::category::category_label(c).map(str::to_string).unwrap_or_else(|| c.to_string()))
        .unwrap_or_else(|| "Search".into())
}

/// `Hardcore%20Covens` → `Hardcore Covens`.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(b)) => {
                out.push(b);
                i += 3;
            }
            (b'+', _) => {
                out.push(b' ');
                i += 1;
            }
            (b, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINK: &str = "https://www.pathofexile.com/trade/search/Covens/H4sIAAAAAAAACnXRQWvDIBgG4L9S3rOMqDE23noau-82SnGJ7QSjIWrZCPnvw45BII0n9eXxk--bEZNOOULNCGOywUMhmi5P-tMZLAReDwYK71_m8HbzYQgeBOlnLJevwfWH05CdSSCPlyLUx_wfa9-D4GpdMtNfYHsomO_R2c6mlwIuVEhOeSWbGgR37bIpfxmsh2JyWchTdKy45LLhdGMopXuI0bqmLauE2Cq2qziXTLCa1RIEvY2lLz1UmrLZEZQ1vGnFsRVbcV7Oq46U6rG7rM6rbRemKY-p0NVsrtrFx1yGnPRueDcx2ZvV7llc1i_10VIH-AEAAA";

    #[test]
    fn imports_a_trade_link() {
        let s = from_link(LINK, "").unwrap();
        assert_eq!(s.league, "Covens");
        assert_eq!(s.body["query"]["name"], "The Ignomon");
        assert_eq!(s.body["query"]["type"], "Gold Amulet");
        assert_eq!(s.body["query"]["stats"][0]["filters"][3]["disabled"], true);
        assert_eq!(default_name(&s.body), "The Ignomon Gold Amulet");
        assert!(!s.live);
    }

    #[test]
    fn reads_leagues_with_spaces_and_live_links() {
        let link = LINK.replace("/Covens/", "/Hardcore%20Covens/") + "/live";
        assert_eq!(from_link(&link, "x").unwrap().league, "Hardcore Covens");
    }

    #[test]
    fn rejects_other_links() {
        assert!(from_link("https://poe.ninja/economy", "").is_err());
        assert!(from_link("https://www.pathofexile.com/trade/search/Covens/Wn8bkzVcm", "").is_err());
    }

    #[test]
    fn duplicates_next_to_the_original() {
        let path = std::env::temp_dir().join(format!("poe-tool-test-{}.json", new_id()));
        let mut store = SearchStore::load(path.clone());
        let group = store.add_group("Build").unwrap();
        let mut first = from_link(LINK, "First").unwrap();
        first.live = true;
        let first = store.add_search(&group.id, first).unwrap();
        store.add_search(&group.id, from_link(LINK, "Second").unwrap()).unwrap();

        let copy = store.duplicate(&first.id).unwrap();
        let names: Vec<_> = store.groups[0].searches.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["First", "First (copy)", "Second"]);
        assert_ne!(copy.id, first.id);
        assert!(!copy.live);
        assert_eq!(copy.body, first.body);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn names_category_searches() {
        let body = json!({ "query": { "filters": { "type_filters": { "filters": { "category": { "option": "weapon.twosword" } } } } } });
        assert_eq!(default_name(&body), "Two-Handed Sword");
        assert_eq!(default_name(&json!({ "query": { "type": "Two-Toned Boots" } })), "Two-Toned Boots");
        assert_eq!(default_name(&json!({ "query": { "term": "Mageblood" } })), "Mageblood");
    }
}
