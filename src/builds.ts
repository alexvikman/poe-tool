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
  /** Configurations, each with its own bandit and pantheon. */
  configs: { id: string | null; title: string | null; config: string[]; bandit: string | null; pantheon: string[] }[];
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
  /** The tree's node ids that are allocated. */
  allocated: number[];
  /** Socket node id → its jewel's place in `jewels`. */
  sockets: Record<string, number>;
  /** Mastery node id → its chosen effect. */
  masteryPicks: Record<string, string>;
}

/** A build in the Builds list. */
export interface SavedBuild {
  id: string;
  name: string;
  source: string | null;
  /** The group of searches it uses, if any. */
  searches: string | null;
}

export const listBuilds = () => invoke<SavedBuild[]>("builds_list");
/** Imports a pobb.in, pastebin or poe.ninja link, or a PoB code, as a new build named
 *  after it. Resolves to its id. */
export const importBuild = (input: string) => invoke<string>("builds_import", { input });
export const buildView = (id: string) => invoke<BuildView>("builds_view", { id });
/** One loadout's stats from Path of Building, or why it couldn't be calculated. */
export interface LoadoutStats {
  stats: Record<string, number> | null;
  pobVersion: string | null;
  error: string | null;
}
/** Stats from Path of Building for loadouts of a build, in the order asked. */
export const calcLoadouts = (id: string, picks: Loadout[]) => invoke<LoadoutStats[]>("builds_calc", { id, picks });
/** Renames a build, or sets the group of searches it uses (null for none). */
export const updateBuild = (id: string, change: { name?: string; searches?: string | null }) =>
  invoke<void>("builds_update", {
    id,
    name: change.name ?? null,
    setSearches: "searches" in change,
    searches: change.searches ?? null,
  });
/** Removes a build; its group of searches stays. */
export const removeBuild = (id: string) => invoke<void>("builds_remove", { id });

/** The passive tree as it's drawn: the same for every build, fetched once. */
export interface TreeLayout {
  nodes: TreeNode[];
  edges: { a: number; b: number; arc: [number, number, number] | null }[];
  sprites: Record<string, { url: string; coords: Record<string, [number, number, number, number]> }>;
  zoom: number;
}

export interface TreeNode {
  id: number;
  x: number;
  y: number;
  kind: "normal" | "notable" | "keystone" | "mastery" | "jewel" | "classStart" | "ascendancyStart";
  name: string;
  stats: string[];
  icon: string;
  inactiveIcon: string | null;
  activeIcon: string | null;
  options: string[];
  ascendancy: string | null;
  class: string | null;
}

let layout: Promise<TreeLayout> | null = null;
export function treeLayout(): Promise<TreeLayout> {
  layout ??= invoke<TreeLayout>("builds_tree").catch((e) => {
    layout = null;
    throw e;
  });
  return layout;
}
