import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  formatChaos,
  formatListingPrice,
  priceSearch,
  timeAgo,
  type PriceCheck,
  type StatFilter,
  type TradeResult,
} from "./price";

const RARITY_LABEL: Record<string, string> = {
  normal: "Normal",
  magic: "Magic",
  rare: "Rare",
  unique: "Unique",
  currency: "Currency",
  gem: "Gem",
  divinationCard: "Divination card",
  other: "",
};

export function PriceResult({ check }: { check: PriceCheck }) {
  const { item } = check;
  const [filters, setFilters] = useState<StatFilter[]>(check.filters);
  const [trade, setTrade] = useState<TradeResult | null>(check.trade);
  const [tradeError, setTradeError] = useState<string | null>(check.tradeError);
  const [searching, setSearching] = useState(false);
  const [dirty, setDirty] = useState(false);

  useEffect(() => {
    setFilters(check.filters);
    setTrade(check.trade);
    setTradeError(check.tradeError);
    setDirty(false);
  }, [check]);

  function updateFilter(i: number, patch: Partial<StatFilter>) {
    setFilters((fs) => fs.map((f, j) => (j === i ? { ...f, ...patch } : f)));
    setDirty(true);
  }

  async function search() {
    setSearching(true);
    try {
      setTrade(await priceSearch(check, filters));
      setTradeError(null);
      setDirty(false);
    } catch (e) {
      setTradeError(String(e));
    } finally {
      setSearching(false);
    }
  }

  const stack = item.stackSize ?? 1;
  const title = item.rarity === "rare" || item.rarity === "unique" ? item.name : item.typeLine;
  const showBase = (item.rarity === "rare" || item.rarity === "unique" || item.rarity === "magic") && check.baseType !== title;
  const canSearch = !["currency", "divinationCard"].includes(item.rarity) || !check.ninja.length;

  return (
    <div className="price">
      <header className={`item-head rarity-${item.rarity}`}>
        <div className="item-title">{title}</div>
        {showBase && <div className="item-base">{check.baseType}</div>}
        <div className="item-meta">
          {[
            RARITY_LABEL[item.rarity],
            item.itemLevel && `ilvl ${item.itemLevel}`,
            item.gemLevel && `level ${item.gemLevel}`,
            item.quality && `${item.quality}% quality`,
            item.links >= 5 && `${item.links}-link`,
            stack > 1 && `${stack}×`,
            item.corrupted && "corrupted",
            !item.identified && "unidentified",
            ...item.influences,
          ]
            .filter(Boolean)
            .join(" · ")}
        </div>
      </header>

      {check.ninja.length > 0 && (
        <section className="ninja">
          <div className="section-label">poe.ninja · {check.league}</div>
          {check.ninja.map((p, i) => (
            <div key={i} className={`ninja-row ${i === 0 ? "best" : ""}`}>
              <span className="ninja-variant">
                {p.variant ?? (p.links ? `${p.links}-link` : p.name)}
                {p.listingCount != null && <small>{p.listingCount} listed</small>}
              </span>
              <span className="ninja-price">
                {formatChaos(p.chaos, check.divineChaos)}
                {stack > 1 && i === 0 && (
                  <small>{formatChaos(p.chaos * stack, check.divineChaos)} for {stack}</small>
                )}
              </span>
            </div>
          ))}
        </section>
      )}
      {check.ninjaError && <p className="bad small">{check.ninjaError}</p>}

      {filters.length > 0 && (
        <section className="filters">
          <div className="section-label">Search for</div>
          {filters.map((f, i) => (
            <label key={f.id + i} className={`filter ${f.enabled ? "" : "off"}`}>
              <input
                type="checkbox"
                checked={f.enabled}
                onChange={(e) => updateFilter(i, { enabled: e.target.checked })}
              />
              <span className={`filter-text kind-${f.kind}`}>{f.text}</span>
              {f.value != null && f.value > 0 && (
                <input
                  className="filter-min"
                  type="number"
                  value={f.min ?? ""}
                  title="Minimum"
                  onChange={(e) =>
                    updateFilter(i, { min: e.target.value === "" ? null : Number(e.target.value) })
                  }
                />
              )}
            </label>
          ))}
        </section>
      )}

      {canSearch && (
        <section className="trade">
          <div className="trade-head">
            <div className="section-label">
              Trade{trade ? ` · ${trade.total} found` : ""}
            </div>
            <div className="trade-actions">
              {(dirty || (!trade && !tradeError)) && (
                <button className="btn primary" onClick={search} disabled={searching}>
                  {searching ? "Searching…" : "Search"}
                </button>
              )}
              {trade && (
                <button className="btn" onClick={() => openUrl(trade.url)}>
                  Open on trade site
                </button>
              )}
            </div>
          </div>
          {tradeError && <p className="bad small">{tradeError}</p>}
          {trade && trade.listings.length === 0 && (
            <p className="muted small">Nothing listed. Try turning some filters off.</p>
          )}
          {trade && trade.listings.length > 0 && (
            <table className="listings">
              <tbody>
                {trade.listings.map((l, i) => (
                  <tr key={i}>
                    <td className="listing-price">{formatListingPrice(l)}</td>
                    <td className="muted">{l.itemLevel ? `ilvl ${l.itemLevel}` : ""}</td>
                    <td className="muted">{timeAgo(l.indexed)}</td>
                    <td className="listing-account" title={l.account}>
                      {l.account}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </section>
      )}
    </div>
  );
}
