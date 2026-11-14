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
}

export interface HitItem {
  name: string;
  typeLine: string;
  icon: string | null;
  rarity: "normal" | "magic" | "rare" | "unique" | "gem" | "currency" | "card" | "relic";
  itemLevel: number | null;
  corrupted: boolean;
  sockets: string | null;
  properties: HitProperty[];
  /** "Level 8, 42 Dex". */
  requirements: string | null;
  mods: HitMod[];
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
}

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
