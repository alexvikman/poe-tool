import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { read } from "./api";

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
  /** Search only listings that can be bought at once, unless changed per search. */
  instantBuyout: boolean;
}

export interface ChatCommand {
  enabled: boolean;
  hotkey: string;
  text: string;
  /** Press Enter after typing. */
  send: boolean;
}

export interface Settings {
  townMute: TownMuteSettings;
  priceCheck: PriceCheckSettings;
  chatCommands: ChatCommand[];
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

export const getSettings = () => read<Settings>("get_settings");
export const setSettings = (settings: Settings) => invoke<void>("set_settings", { settings });
export const getStatus = () => read<Status>("get_status");
export const onStatus = (cb: (s: Status) => void): Promise<UnlistenFn> =>
  listen<Status>("town-mute-status", (e) => cb(e.payload));
