import { invoke } from "@tauri-apps/api/core";
import type { HitItem } from "./searches";

/** A Path of Building build as the Builds page shows it. */
export interface BuildView {
  class: string;
  ascendancy: string | null;
  level: number;
  /** The main skill ("Ball Lightning of Orbiting"). */
  mainSkill: string | null;
  /** PoB's game version ("3_27"). */
  version: string | null;
  bandit: string | null;
  pantheon: string[];
  /** What PoB calculated at export, by PoB's names ("CombinedDPS", "Life"): for the
   *  loadout it was exported with (`active`). */
  stats: Record<string, number>;
  itemSets: { id: string | null; title: string | null; gear: GearItem[] }[];
  skillSets: { id: string | null; title: string | null; groups: GemGroup[] }[];
  trees: TreeView[];
  configs: { id: string | null; title: string | null; config: string[] }[];
  /** Sets that go together, by name as in PoB ("Lvl 96"). */
  loadouts: Loadout[];
  /** What PoB had chosen when the build was exported. */
  active: Loadout;
  notes: string;
  code: string;
  source: string | null;
}

export interface GearItem {
  slot: string;
  item: HitItem;
}

/** One of each kind of set, by its place in the build's lists. */
export interface Loadout {
  title: string;
  itemSet: number;
  skillSet: number;
  tree: number;
  config: number;
}

export interface GemGroup {
  label: string | null;
  slot: string | null;
  enabled: boolean;
  /** The build's main skill is in this group. */
  main: boolean;
  /** The item that grants the group's skill, when an item does. */
  source: string | null;
  gems: Gem[];
}

export interface Gem {
  name: string;
  level: number;
  quality: number;
  enabled: boolean;
  support: boolean;
  colour: "red" | "green" | "blue" | "white" | null;
  /** What the wiki says of the gem; none for skills items grant. */
  info: GemInfo | null;
}

export interface GemInfo {
  icon: string | null;
  requiredLevel: number | null;
  /** The gem's wiki page. */
  wiki: string;
  /** Where the build's class gets it in the campaign; no npc for a quest's reward. */
  rewards: { act: number; npc: string | null; quest: string }[];
}

export interface TreeView {
  title: string | null;
  version: string | null;
  /** Jewels socketed in this tree. */
  jewels: GearItem[];
  points: number;
  ascendancyPoints: number;
  keystones: string[];
  notables: string[];
  ascendancyNotables: string[];
  masteries: string[];
}

/** Imports a pobb.in, pastebin or poe.ninja link, or a PoB code: into a build, or as a
 *  new build named after it. Resolves to the build's id. */
export const importBuild = (input: string, groupId: string | null) =>
  invoke<string>("builds_import", { input, groupId });
export const buildView = (groupId: string) => invoke<BuildView>("builds_view", { groupId });
/** Stats from the PoB service for a build with these sets chosen. */
export const calcLoadout = (groupId: string, pick: Loadout) =>
  invoke<{ stats: Record<string, number>; pobVersion: string | null }>("builds_calc", { groupId, pick });

/** Takes the PoB build off a build, keeping its searches. */
export const removeBuild = (groupId: string) => invoke<void>("builds_remove", { groupId });
