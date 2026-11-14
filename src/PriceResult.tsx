import { useEffect, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { addGroup, listGroups, saveCheckSearch, type SearchGroup } from "./searches";
import {
  formatChaos,
  formatListingPrice,
  priceSearch,
  timeAgo,
  type PriceCheck,
  type SearchOptions,
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

/** Wait this long after the last filter change before searching again. */
const SEARCH_DELAY_MS = 700;

export function PriceResult({ check }: { check: PriceCheck }) {
  const { item } = check;
  const [filters, setFilters] = useState<StatFilter[]>(check.filters);
  const [options, setOptions] = useState<SearchOptions>(check.options);
  const [trade, setTrade] = useState<TradeResult | null>(check.trade);
  const [tradeError, setTradeError] = useState<string | null>(check.tradeError);
  const [searching, setSearching] = useState(false);
  const [showHidden, setShowHidden] = useState(false);
  const [saving, setSaving] = useState(false);
  const timer = useRef<number | undefined>(undefined);
  const searchId = useRef(0);

  useEffect(() => {
    window.clearTimeout(timer.current);
    setFilters(check.filters);
    setOptions(check.options);
    setTrade(check.trade);
    setTradeError(check.tradeError);
  }, [check]);

  useEffect(() => () => window.clearTimeout(timer.current), []);

  async function search(f = filters, o = options): Promise<TradeResult | null> {
    const id = ++searchId.current;
    setSearching(true);
    try {
      const result = await priceSearch(check, f, o);
      if (id === searchId.current) {
        setTrade(result);
        setTradeError(null);
      }
      return result;
    } catch (e) {
      if (id === searchId.current) setTradeError(String(e));
      return null;
    } finally {
      if (id === searchId.current) setSearching(false);
    }
  }

  function searchSoon(f: StatFilter[], o: SearchOptions) {
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => search(f, o), SEARCH_DELAY_MS);
  }

  function updateFilter(i: number, patch: Partial<StatFilter>) {
    const next = filters.map((f, j) => (j === i ? { ...f, ...patch } : f));
    setFilters(next);
    searchSoon(next, options);
  }

  function updateOptions(patch: Partial<SearchOptions>) {
    const next = { ...options, ...patch };
    setOptions(next);
    searchSoon(filters, next);
  }

  async function openTrade() {
    window.clearTimeout(timer.current);
    const result = trade && !searching ? trade : await search();
    if (result) openUrl(result.url);
  }

  const stack = item.stackSize ?? 1;
  const title = item.rarity === "rare" || item.rarity === "unique" ? item.name : item.typeLine;
  const showBase =
    ["rare", "unique", "magic"].includes(item.rarity) && check.baseType !== title;
  const bulk = trade?.exchange ?? ["currency", "divinationCard", "other"].includes(item.rarity);
  const enabledCount = filters.filter((f) => f.enabled && f.id).length;
  const hiddenCount = filters.filter((f) => f.hidden).length;
  const best = check.ninja[0];

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

      {best && (
        <section className="ninja">
          <div className="ninja-main">
            <div>
              <div className="section-label">
                poe.ninja · {check.league}
                {item.rarity === "unique" && " · any roll"}
              </div>
              <div className="ninja-price">
                {formatChaos(best.chaos, check.divineChaos)}
                {stack > 1 && (
                  <small> · {formatChaos(best.chaos * stack, check.divineChaos)} for {stack}</small>
                )}
              </div>
              <div className="ninja-sub">
                {[
                  best.variant ?? (best.links ? `${best.links}-link` : null),
                  best.listingCount != null && `${best.listingCount} listed`,
                ]
                  .filter(Boolean)
                  .join(" · ")}
              </div>
            </div>
            {best.change7d != null && (
              <div className={`trend ${best.change7d >= 0 ? "up" : "down"}`}>
                {best.change7d >= 0 ? "+" : ""}
                {Math.round(best.change7d)}%<small>last 7 days</small>
              </div>
            )}
          </div>
          {check.ninja.slice(1).map((p, i) => (
            <div key={i} className="ninja-row">
              <span>{p.variant ?? (p.links ? `${p.links}-link` : p.name)}</span>
              <span>{formatChaos(p.chaos, check.divineChaos)}</span>
            </div>
          ))}
        </section>
      )}
      {check.ninjaError && <p className="bad small">{check.ninjaError}</p>}

      <div className="chips">
        {options.category && (
          <button
            className={`chip ${options.category.enabled ? "on" : ""}`}
            onClick={() => updateOptions({ category: { ...options.category!, enabled: !options.category!.enabled } })}
            title="Search the whole category, or only this base type"
          >
            {options.category.enabled ? options.category.label : check.baseType}
          </button>
        )}
        {options.itemLevel && (
          <button
            className={`chip ${options.itemLevel.enabled ? "on" : ""}`}
            onClick={() => updateOptions({ itemLevel: { ...options.itemLevel!, enabled: !options.itemLevel!.enabled } })}
            title="Only items of this item level or higher"
          >
            Item level {options.itemLevel.value}
            {options.itemLevel.enabled ? "+" : ""}
          </button>
        )}
        {!bulk && (
          <button
            className={`chip ${options.corrupted != null ? "on" : ""}`}
            onClick={() =>
              updateOptions({
                corrupted: options.corrupted === null ? false : options.corrupted === false ? true : null,
              })
            }
            title="Click to change"
          >
            {options.corrupted === null ? "Corrupted: any" : options.corrupted ? "Corrupted" : "Not corrupted"}
          </button>
        )}
        <button
          className={`chip ${options.instantBuyout ? "on" : ""}`}
          onClick={() => updateOptions({ instantBuyout: !options.instantBuyout })}
        >
          Instant buyout only
        </button>
      </div>

      {filters.length > 0 && (
        <section className="filters">
          <div className="filters-head">
            <span className="section-label">
              {enabledCount} of {filters.length} stats
            </span>
            {hiddenCount > 0 && (
              <button className="link-btn" onClick={() => setShowHidden(!showHidden)}>
                {showHidden ? "Hide" : "Show"} {hiddenCount} hidden
              </button>
            )}
          </div>
          {filters.map((f, i) =>
            f.hidden && !showHidden ? null : f.id ? (
              <FilterRow key={i} filter={f} onChange={(patch) => updateFilter(i, patch)} />
            ) : (
              <div key={i} className="filter-unknown">
                <span className="warn">!</span>
                <div>
                  {f.text}
                  <small>Not a stat the trade site can search for</small>
                </div>
              </div>
            ),
          )}
        </section>
      )}

      <section className="trade">
        <div className="trade-head">
          <div className="section-label">
            {searching
              ? "Searching…"
              : trade
                ? `Matched: ${trade.total}`
                : "Trade"}
          </div>
          <div className="trade-actions">
            {!bulk && (
              <button className="btn" onClick={() => setSaving(!saving)} title="Keep this search in a group">
                Save search
              </button>
            )}
            <button className="btn" onClick={openTrade} title="Open this search on pathofexile.com">
              Trade ↗
            </button>
          </div>
        </div>
        {saving && (
          <SaveSearch
            defaultName={
              // As the trade site names the search: "The Ignomon Gold Amulet"; a rare's own name means nothing there.
              item.rarity === "unique" && item.name ? `${item.name} ${check.baseType}` : check.baseType
            }
            onSave={async (groupId, name) => {
              await saveCheckSearch({
                groupId,
                name,
                item: check.item,
                baseType: check.baseType,
                filters,
                options,
                league: check.league,
              });
            }}
          />
        )}
        {tradeError && <p className="bad small">{tradeError}</p>}
        {trade && trade.listings.length === 0 && !searching && (
          <p className="muted small">Nothing listed. Try turning some stats off.</p>
        )}
        {trade && trade.listings.length > 0 && (
          <table className={`listings ${searching ? "stale" : ""}`}>
            <thead>
              <tr>
                <th>{trade.exchange ? "Price each" : "Price"}</th>
                <th>Listed</th>
                <th>{trade.exchange ? "Stock" : "ilvl"}</th>
                <th>Seller</th>
              </tr>
            </thead>
            <tbody>
              {trade.listings.map((l, i) => (
                <tr key={i}>
                  <td className="listing-price">{formatListingPrice(l)}</td>
                  <td>{timeAgo(l.indexed)} ago</td>
                  <td className="muted">{trade.exchange ? l.stock ?? "" : l.itemLevel ?? ""}</td>
                  <td className="listing-account" title={l.account}>
                    {l.account}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>
    </div>
  );
}

function FilterRow({
  filter: f,
  onChange,
}: {
  filter: StatFilter;
  onChange: (patch: Partial<StatFilter>) => void;
}) {
  const hasValue = f.value != null;
  return (
    <div className={`filter ${f.enabled ? "" : "off"}`}>
      <label className="filter-line">
        <input
          type="checkbox"
          checked={f.enabled}
          onChange={(e) => onChange({ enabled: e.target.checked })}
        />
        <span className={`filter-text kind-${f.kind}`}>
          {f.text}
          {f.tier != null && <span className="tier">T{f.tier}</span>}
          {f.kind === "pseudo" && <span className="tier">pseudo</span>}
          {f.hidden && <span className="tier">hidden</span>}
        </span>
      </label>
      {hasValue && (
        <div className="filter-bounds">
          <NumberBox value={f.min} placeholder="min" onChange={(min) => onChange({ min, enabled: true })} />
          <NumberBox value={f.max} placeholder="max" onChange={(max) => onChange({ max, enabled: true })} />
        </div>
      )}
      {f.roll && f.roll.max !== f.roll.min && f.value != null && (
        <RollBar roll={f.roll} value={f.value} min={f.min} max={f.max} />
      )}
    </div>
  );
}

function NumberBox({
  value,
  placeholder,
  onChange,
}: {
  value: number | null;
  placeholder: string;
  onChange: (v: number | null) => void;
}) {
  return (
    <input
      className="filter-num"
      type="number"
      value={value ?? ""}
      placeholder={placeholder}
      onChange={(e) => onChange(e.target.value === "" ? null : Number(e.target.value))}
    />
  );
}

/** The mod's possible range, the item's roll (tick) and the searched part (filled). */
function RollBar({
  roll,
  value,
  min,
  max,
}: {
  roll: { min: number; max: number };
  value: number;
  min: number | null;
  max: number | null;
}) {
  const lo = Math.min(roll.min, roll.max);
  const hi = Math.max(roll.min, roll.max);
  const pct = (v: number) => `${((Math.min(Math.max(v, lo), hi) - lo) / (hi - lo)) * 100}%`;
  const from = min ?? lo;
  const to = max ?? hi;
  return (
    <div className="roll">
      <span className="roll-end">{lo}</span>
      <div className="roll-track">
        <div className="roll-fill" style={{ left: pct(from), right: `calc(100% - ${pct(to)})` }} />
        <div className="roll-tick" style={{ left: pct(value) }} title={`This item: ${value}`} />
      </div>
      <span className="roll-end">{hi}</span>
    </div>
  );
}

/** Saves the search as it is now in a group, new or existing. */
function SaveSearch({
  defaultName,
  onSave,
}: {
  defaultName: string;
  onSave: (groupId: string, name: string) => Promise<void>;
}) {
  const [groups, setGroups] = useState<SearchGroup[]>([]);
  const [groupId, setGroupId] = useState("");
  const [newGroup, setNewGroup] = useState("");
  const [name, setName] = useState(defaultName);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    listGroups().then((g) => {
      setGroups(g);
      setGroupId(g[0]?.id ?? "new");
    });
  }, []);

  async function save() {
    try {
      const id = groupId === "new" ? (await addGroup(newGroup)).id : groupId;
      await onSave(id, name);
      setSaved(true);
    } catch (e) {
      setError(String(e));
    }
  }

  if (saved) return <p className="muted small">Saved under Searches.</p>;
  return (
    <div className="save-search">
      <select value={groupId} onChange={(e) => setGroupId(e.target.value)}>
        {groups.map((g) => (
          <option key={g.id} value={g.id}>
            {g.name}
          </option>
        ))}
        <option value="new">New group…</option>
      </select>
      {groupId === "new" && (
        <input type="text" placeholder="Group name" value={newGroup} onChange={(e) => setNewGroup(e.target.value)} />
      )}
      <input type="text" placeholder="Search name" value={name} onChange={(e) => setName(e.target.value)} />
      <button className="btn primary" onClick={save} disabled={groupId === "new" && !newGroup.trim()}>
        Save
      </button>
      {error && <p className="bad small">{error}</p>}
    </div>
  );
}
