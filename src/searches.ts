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

export interface LiveHit {
  searchId: string;
  searchName: string;
  group: string;
  league: string;
  seen: string;
  itemName: string;
  typeLine: string;
  amount: number | null;
  currency: string | null;
  account: string;
  hideoutToken: string | null;
  whisperToken: string | null;
  whisper: string | null;
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

export const liveState = () => read<[Record<string, LiveStatus>, LiveHit[]]>("live_state");
export const actOnListing = (token: string, league: string) =>
  invoke<void>("live_use_token", { token, league });

export const onLiveHit = (cb: (hit: LiveHit) => void): Promise<UnlistenFn> =>
  listen<LiveHit>("live-hit", (e) => cb(e.payload));
export const onLiveStatus = (cb: (s: Record<string, LiveStatus>) => void): Promise<UnlistenFn> =>
  listen<Record<string, LiveStatus>>("live-status", (e) => cb(e.payload));
