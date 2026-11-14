import { invoke } from "@tauri-apps/api/core";

export type Rarity =
  | "normal"
  | "magic"
  | "rare"
  | "unique"
  | "currency"
  | "gem"
  | "divinationCard"
  | "other";

export interface ItemMod {
  kind: string;
  text: string;
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
}

export interface StatFilter {
  id: string;
  text: string;
  value: number | null;
  min: number | null;
  enabled: boolean;
  kind: string;
}

export interface Listing {
  amount: number;
  currency: string;
  indexed: string;
  account: string;
  itemLevel: number | null;
  corrupted: boolean;
}

export interface TradeResult {
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
  trade: TradeResult | null;
  tradeError: string | null;
}

export interface League {
  id: string;
}

export const priceCheck = (text: string) => invoke<PriceCheck>("price_check", { text });

export const priceSearch = (check: PriceCheck, filters: StatFilter[]) =>
  invoke<TradeResult>("price_search", {
    item: check.item,
    baseType: check.baseType,
    filters,
    league: check.league,
  });

export const getLeagues = () => invoke<League[]>("get_leagues");

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
