import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type AreaKind = "town" | "hideout" | "other" | "unknown";

export interface TownMuteSettings {
  enabled: boolean;
  /** The game's volume in town, in percent of its normal volume. 0 is silent. */
  townVolume: number;
  includeHideouts: boolean;
  clientLogPath: string | null;
  extraAreas: string[];
}

export interface PriceCheckSettings {
  enabled: boolean;
  hotkey: string;
  /** null: the current challenge league. */
  league: string | null;
}

export interface Settings {
  townMute: TownMuteSettings;
  priceCheck: PriceCheckSettings;
}

export interface Status {
  logPath: string | null;
  logError: string | null;
  gameRunning: boolean;
  areaId: string | null;
  areaName: string | null;
  areaKind: AreaKind;
  lowered: boolean;
  gameSessions: number;
  audioError: string | null;
}

export const getSettings = () => invoke<Settings>("get_settings");
export const setSettings = (settings: Settings) => invoke<void>("set_settings", { settings });
export const getStatus = () => invoke<Status>("get_status");
export const onStatus = (cb: (s: Status) => void): Promise<UnlistenFn> =>
  listen<Status>("town-mute-status", (e) => cb(e.payload));
