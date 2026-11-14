import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { read } from "./api";
import type { Item, SearchOptions, StatFilter } from "./price";

export interface SavedSearch {
  id: string;
  name: string;
  league: string;
  live: boolean;
}

/** A group of searches, usually a build ("Shockwave Cyclone"). */
export interface SearchGroup {
  id: string;
  name: string;
  searches: SavedSearch[];
}

export type LiveStatus =
  | { state: "connecting" }
  | { state: "live" }
  | { state: "retrying"; message: string }
  | { state: "failed"; message: string };

export interface HitMod {
  text: string;
  kind: "enchant" | "implicit" | "fractured" | "explicit" | "crafted" | "mutated";
  /** What it can roll, where that varies: "160–200". */
  range: string | null;
  /** The trade site's sort key for it ("stat.explicit.stat_…"). */
  sort: string | null;
}

/** One requirement: "Level" and "49", or "Int" and "134". */
export interface HitRequirement {
  name: string;
  value: string;
  sort: string | null;
}

export interface HitItem {
  name: string;
  typeLine: string;
  icon: string | null;
  /** Its size in inventory cells. */
  width: number;
  height: number;
  rarity: "normal" | "magic" | "rare" | "unique" | "gem" | "currency" | "card" | "relic";
  itemLevel: number | null;
  corrupted: boolean;
  sockets: string | null;
  properties: HitProperty[];
  requirements: HitRequirement[];
  mods: HitMod[];
  /** The site's line under the item: base percentile, defences at 20% quality, DPS. */
  extended: HitProperty[];
  unidentified: boolean;
  mirrored: boolean;
  /** The seller's note, "~b/o 1 alch". */
  note: string | null;
}

/** "Quality: +20%" is label "Quality", value "+20%"; some lines come whole as the label. */
export interface HitProperty {
  label: string;
  value: string | null;
  /** Changed by the item's mods: the game shows it blue. */
  augmented: boolean;
  /** The trade site's sort key for it, where results sort by it. */
  sort: string | null;
}

/** How an ordinary search's results are sorted, as clicking a value on the site does. */
export interface SearchSort {
  key: string;
  dir: "asc" | "desc";
  /** What was clicked, to say what it's sorted by. */
  label: string;
}

const sortBody = (sort?: SearchSort | null) => (sort ? { [sort.key]: sort.dir } : null);

export interface Currency {
  name: string;
  icon: string | null;
}

export interface LiveHit {
  id: string;
  searchId: string;
  searchName: string;
  group: string;
  league: string;
  seen: string;
  item: HitItem;
  amount: number | null;
  currency: string | null;
  /** "~b/o" or "~price". */
  priceType: string | null;
  account: string;
  hideoutToken: string | null;
  whisperToken: string | null;
  whisper: string | null;
  /** Sold or taken down. */
  gone: boolean;
}

export interface SessionStatus {
  set: boolean;
  account: string | null;
  error: string | null;
}

export const listGroups = () => read<SearchGroup[]>("searches_list");
export const addGroup = (name: string) => invoke<SearchGroup>("searches_add_group", { name });
export const renameGroup = (id: string, name: string) => invoke<void>("searches_rename_group", { id, name });
export const deleteGroup = (id: string) => invoke<void>("searches_delete_group", { id });
export const addLink = (groupId: string, link: string, name: string) =>
  invoke<void>("searches_add_link", { groupId, link, name });
export const updateSearch = (id: string, patch: { name?: string; live?: boolean }) =>
  invoke<void>("searches_update", { id, name: patch.name ?? null, live: patch.live ?? null });
/** A saved search as the edit form shows it: the trade site's query itself. */
export interface SearchDetail {
  name: string;
  league: string;
  groupId: string;
  // The site's own free-form query JSON.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  query: any;
}

export const searchDetail = (id: string) => invoke<SearchDetail>("searches_detail", { id });
export const editSearch = (id: string, edit: SearchDetail & { link: string | null }) =>
  invoke<void>("searches_edit", { id, edit });

/** A page of an ordinary search: the first listings, and the ids of the rest. */
export interface SearchPage {
  queryId: string;
  /** How many match (the site counts up to 10,000). */
  total: number;
  /** The listings the site shows (up to 100), in order. */
  ids: string[];
  listings: LiveHit[];
}

/** Runs a saved search now, as the trade site's Search button does. */
export const runSavedSearch = (id: string, sort?: SearchSort | null) =>
  invoke<SearchPage>("trade_search", { id, league: null, query: null, sort: sortBody(sort) });
/** Runs a query from the search form. */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export const runSearch = (league: string, query: any, sort?: SearchSort | null) =>
  invoke<SearchPage>("trade_search", { id: null, league, query, sort: sortBody(sort) });
export const searchMore = (id: string | null, league: string, queryId: string, ids: string[]) =>
  invoke<LiveHit[]>("trade_search_more", { id, league, queryId, ids });
/** Keeps a query from the search form as a saved search in a build. */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export const addQuery = (groupId: string, name: string, league: string, query: any) =>
  invoke<SavedSearch>("searches_add_query", { groupId, name, league, query });

/** A copy of a saved search right after it ("Name (copy)", not live). */
export const duplicateSearch = (id: string) => invoke<SavedSearch>("searches_duplicate", { id });

export const deleteSearch = (id: string) => invoke<void>("searches_delete", { id });
export const searchUrl = (id: string) => invoke<string>("searches_open", { id });

export const saveCheckSearch = (args: {
  groupId: string;
  name: string;
  item: Item;
  baseType: string;
  filters: StatFilter[];
  options: SearchOptions;
  league: string;
}) => invoke<void>("searches_save_check", args);

export const sessionStatus = () => invoke<SessionStatus>("session_status");
export const setSession = (value: string) => invoke<SessionStatus>("session_set", { value });
export const clearSession = () => invoke<void>("session_clear");
/** Opens pathofexile.com's sign-in in a window; resolves once signed in. */
export const signIn = () => invoke<SessionStatus>("session_sign_in");

/** Currencies listings are priced in, by id ("alch" → Orb of Alchemy). */
export const tradeCurrencies = () => read<Record<string, Currency>>("trade_currencies");

export const liveState = () => read<[Record<string, LiveStatus>, LiveHit[]]>("live_state");
/** Travel to the seller's hideout, or whisper them: says which it did. */
export const actOnListing = (id: string) => invoke<"travel" | "whisper">("live_act", { id });

export const onLiveHits = (cb: (hits: LiveHit[]) => void): Promise<UnlistenFn> =>
  listen<LiveHit[]>("live-hits", (e) => cb(e.payload));
export const onLiveStatus = (cb: (s: Record<string, LiveStatus>) => void): Promise<UnlistenFn> =>
  listen<Record<string, LiveStatus>>("live-status", (e) => cb(e.payload));
