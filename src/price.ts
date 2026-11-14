import { invoke } from "@tauri-apps/api/core";
import { read } from "./api";

export type Rarity =
  | "normal"
  | "magic"
  | "rare"
  | "unique"
  | "currency"
  | "gem"
  | "divinationCard"
  | "other";

export interface Roll {
  min: number;
  max: number;
}

export interface ItemMod {
  kind: string;
  text: string;
  roll: Roll | null;
  affix: string | null;
  tier: number | null;
}

export interface Item {
  itemClass: string;
  rarity: Rarity;
  name: string | null;
  typeLine: string;
  itemLevel: number | null;
  stackSize: number | null;
  gemLevel: number | null;
  quality: number | null;
  mapTier: number | null;
  sockets: string | null;
  links: number;
  corrupted: boolean;
  identified: boolean;
  mirrored: boolean;
  influences: string[];
  mods: ItemMod[];
}

export interface NinjaPrice {
  name: string;
  baseType: string | null;
  variant: string | null;
  links: number | null;
  gemLevel: number | null;
  gemQuality: number | null;
  corrupted: boolean | null;
  chaos: number;
  listingCount: number | null;
  /** Price change over the last 7 days, in percent. */
  change7d: number | null;
}

export interface StatFilter {
  /** Empty when the trade site has no stat for this line. */
  id: string;
  text: string;
  value: number | null;
  min: number | null;
  max: number | null;
  enabled: boolean;
  kind: string;
  roll: Roll | null;
  tier: number | null;
}

export interface SearchOptions {
  /** null: either. */
  corrupted: boolean | null;
  instantBuyout: boolean;
}

export interface Listing {
  amount: number;
  currency: string;
  indexed: string;
  account: string;
  itemLevel: number | null;
  corrupted: boolean;
  /** Exchange listings: how many the seller has. */
  stock: number | null;
}

export interface TradeResult {
  /** From the currency exchange: prices are per item, with stock. */
  exchange: boolean;
  total: number;
  url: string;
  listings: Listing[];
}

export interface PriceCheck {
  item: Item;
  baseType: string;
  league: string;
  divineChaos: number;
  ninja: NinjaPrice[];
  ninjaError: string | null;
  filters: StatFilter[];
  options: SearchOptions;
  trade: TradeResult | null;
  tradeError: string | null;
}

export interface League {
  id: string;
}

export const priceCheck = (text: string) => invoke<PriceCheck>("price_check", { text });

export const priceSearch = (check: PriceCheck, filters: StatFilter[], options: SearchOptions) =>
  invoke<TradeResult>("price_search", {
    item: check.item,
    baseType: check.baseType,
    filters,
    options,
    league: check.league,
  });

export const getLeagues = (refresh = false) => read<League[]>("get_leagues", { refresh });

/** "1.2 div" when worth a divine or more, else "35 c". */
export function formatChaos(chaos: number, divineChaos: number): string {
  if (divineChaos > 0 && chaos >= divineChaos) {
    const div = chaos / divineChaos;
    return `${div >= 10 ? Math.round(div) : div.toFixed(1)} div`;
  }
  return `${chaos >= 10 ? Math.round(chaos) : +chaos.toFixed(1)} c`;
}

const CURRENCY_SHORT: Record<string, string> = {
  chaos: "c",
  divine: "div",
  exalted: "ex",
  alch: "alch",
  mirror: "mirror",
};

export function formatListingPrice(l: Listing): string {
  return `${+l.amount.toFixed(2)} ${CURRENCY_SHORT[l.currency] ?? l.currency}`;
}

export function timeAgo(iso: string): string {
  const s = Math.max(0, (Date.now() - new Date(iso).getTime()) / 1000);
  if (s < 3600) return `${Math.max(1, Math.round(s / 60))} min`;
  if (s < 86400) return `${Math.round(s / 3600)} h`;
  return `${Math.round(s / 86400)} d`;
}
