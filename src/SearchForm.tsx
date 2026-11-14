// The trade site's search form, built from the site's own description of its filters
// (/api/trade/data/filters), stats and items, so it has what the site has and follows
// it when the site changes. It edits a search's query as the site would write it.

import { useEffect, useMemo, useState } from "react";
import { read } from "./api";

/* The site's data */

interface Option {
  id: string | number | null;
  text: string;
}

interface FilterDef {
  id: string;
  text?: string;
  tip?: string;
  minMax?: boolean;
  sockets?: boolean;
  fullSpan?: boolean;
  option?: { options?: Option[]; knownItem?: unknown };
  input?: { placeholder?: string };
}

interface FilterGroupDef {
  id: string;
  title?: string;
  hidden?: boolean;
  filters: FilterDef[];
}

interface StatDef {
  id: string;
  text: string;
  type: string;
  option?: { options: Option[] };
}

interface StatGroupDef {
  id: string;
  label: string;
  entries: StatDef[];
}

interface ItemDef {
  name?: string;
  type: string;
  text?: string;
  disc?: string;
  flags?: { unique?: boolean };
}

interface ItemGroupDef {
  id: string;
  label: string;
  entries: ItemDef[];
}

const loaded: Record<string, Promise<unknown>> = {};

/** One of the site's data files, fetched once (the app caches them for a day). */
function useTradeData<T>(kind: "filters" | "stats" | "items"): T | null {
  const [data, setData] = useState<T | null>(null);
  useEffect(() => {
    loaded[kind] ??= read<T>("trade_data", { kind }).catch((e) => {
      delete loaded[kind]; // Try again next time.
      throw e;
    });
    (loaded[kind] as Promise<T>).then(setData, () => {});
  }, [kind]);
  return data;
}

/* The query: the site's JSON, changed through copies */

// The query is the site's own free-form JSON.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
type Query = any;

function copy(q: Query): Query {
  return JSON.parse(JSON.stringify(q ?? {}));
}

function filterValue(q: Query, group: string, id: string) {
  return q.filters?.[group]?.filters?.[id];
}

/** Sets one filter, or removes it when it says nothing (null or no fields left). */
function withFilter(q: Query, group: string, id: string, value: Record<string, unknown> | null): Query {
  const next = copy(q);
  const clean = value && Object.fromEntries(Object.entries(value).filter(([, v]) => v !== undefined && v !== null && v !== ""));
  next.filters ??= {};
  next.filters[group] ??= { filters: {} };
  next.filters[group].filters ??= {};
  if (clean && Object.keys(clean).length > 0) {
    next.filters[group].filters[id] = clean;
  } else {
    delete next.filters[group].filters[id];
  }
  if (Object.keys(next.filters[group].filters).length === 0 && !next.filters[group].disabled) {
    delete next.filters[group];
  }
  return next;
}

/** The site's own id for a chosen option (a number stays a number); null for "Any". */
function optionId(options: Option[], chosen: string): string | number | null {
  return options.find((o) => String(o.id) === chosen)?.id ?? null;
}

function toNumber(text: string): number | undefined {
  const t = text.trim().replace(",", ".");
  return t === "" || isNaN(Number(t)) ? undefined : Number(t);
}

/* The form */

export function SearchForm({
  query,
  onChange,
  league,
  leagues,
  onLeague,
}: {
  query: Query;
  onChange: (q: Query) => void;
  league: string;
  leagues: string[];
  onLeague: (league: string) => void;
}) {
  const filters = useTradeData<FilterGroupDef[]>("filters");
  const status = filters?.find((g) => g.id === "status_filters")?.filters[0];

  return (
    <div className="search-form">
      <div className="sf-top">
        <ItemPicker query={query} onChange={onChange} />
        <select value={league} onChange={(e) => onLeague(e.target.value)}>
          {[...new Set([league, ...leagues])].map((l) => (
            <option key={l}>{l}</option>
          ))}
        </select>
        <select
          value={query.status?.option ?? "online"}
          onChange={(e) => onChange({ ...copy(query), status: { option: e.target.value } })}
        >
          {(status?.option?.options ?? [{ id: "online", text: "In Person (Online)" }]).map((o) => (
            <option key={String(o.id)} value={String(o.id)}>
              {o.text}
            </option>
          ))}
        </select>
      </div>
      {!filters && <p className="muted small">Loading the trade site's filters…</p>}
      {filters && (
        <div className="sf-columns">
          <div className="sf-groups">
            {filters
              .filter((g) => g.id !== "status_filters")
              .map((g) => (
                <FilterGroup key={g.id} def={g} query={query} onChange={onChange} />
              ))}
          </div>
          <StatFilters query={query} onChange={onChange} />
        </div>
      )}
    </div>
  );
}

/** What the search is for: a unique, a base type, a gem, or typed text. */
function ItemPicker({ query, onChange }: { query: Query; onChange: (q: Query) => void }) {
  const items = useTradeData<ItemGroupDef[]>("items");
  const [text, setText] = useState("");
  const [focused, setFocused] = useState(false);

  const type = typeof query.type === "string" ? query.type : query.type?.option;
  const current = [query.name, type].filter(Boolean).join(" ") || query.term || "";

  const all = useMemo(
    () =>
      (items ?? []).flatMap((g) =>
        g.entries.map((e) => ({ entry: e, label: e.name ? `${e.name} ${e.type}` : (e.text ?? e.type), group: g.label })),
      ),
    [items],
  );
  const words = text.toLowerCase().split(/\s+/).filter(Boolean);
  const matches =
    words.length === 0 ? [] : all.filter((i) => words.every((w) => i.label.toLowerCase().includes(w))).slice(0, 40);

  function pick(e: ItemDef) {
    const next = copy(query);
    delete next.term;
    delete next.name;
    if (e.name) next.name = e.name;
    next.type = e.disc ? { option: e.type, discriminator: e.disc } : e.type;
    onChange(next);
    setText("");
  }

  function clear() {
    const next = copy(query);
    delete next.name;
    delete next.type;
    delete next.term;
    onChange(next);
  }

  return (
    <div className="sf-item">
      <input
        type="text"
        placeholder={current ? "" : "Search items… (any item)"}
        value={text}
        onChange={(e) => setText(e.target.value)}
        onFocus={() => setFocused(true)}
        onBlur={() => setTimeout(() => setFocused(false), 150)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && text.trim()) {
            e.preventDefault();
            if (matches[0]) {
              pick(matches[0].entry);
            } else {
              // As the site: text no item matches searches item text.
              const next = copy(query);
              delete next.name;
              delete next.type;
              next.term = text.trim();
              onChange(next);
              setText("");
            }
          }
          if (e.key === "Escape") setText("");
        }}
      />
      {current && !text && (
        <span className="sf-current">
          {current}
          <button className="icon-btn" title="Search any item" onClick={clear}>
            ×
          </button>
        </span>
      )}
      {focused && matches.length > 0 && (
        <div className="sf-suggest">
          {matches.map((m, i) => (
            <button key={i} onMouseDown={() => pick(m.entry)}>
              <span className={m.entry.flags?.unique ? "sf-unique" : ""}>{m.label}</span>
              <small className="muted">{m.group}</small>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function FilterGroup({ def, query, onChange }: { def: FilterGroupDef; query: Query; onChange: (q: Query) => void }) {
  const group = query.filters?.[def.id];
  const used = group && Object.keys(group.filters ?? {}).length > 0;
  const [open, setOpen] = useState(!def.hidden || !!used);
  const on = open && !group?.disabled;

  function toggle() {
    const next = copy(query);
    if (on) {
      // Off, as on the site: kept, but not searched for.
      if (used) next.filters[def.id].disabled = true;
      setOpen(false);
    } else {
      if (next.filters?.[def.id]) delete next.filters[def.id].disabled;
      setOpen(true);
    }
    onChange(next);
  }

  return (
    <section className={`sf-group ${on ? "" : "off"}`}>
      <button className="sf-group-head" onClick={toggle}>
        <input type="checkbox" readOnly checked={on} tabIndex={-1} />
        {def.title ?? def.id}
        {!on && used && <span className="muted small"> (kept, not searched)</span>}
      </button>
      {open && (
        <div className="sf-fields">
          {def.filters.map((f) => (
            <FilterField key={f.id} group={def.id} def={f} query={query} onChange={onChange} />
          ))}
        </div>
      )}
    </section>
  );
}

function FilterField({
  group,
  def,
  query,
  onChange,
}: {
  group: string;
  def: FilterDef;
  query: Query;
  onChange: (q: Query) => void;
}) {
  const value = filterValue(query, group, def.id) ?? {};
  const set = (patch: Record<string, unknown>) => onChange(withFilter(query, group, def.id, { ...value, ...patch }));
  const options = def.option?.options;

  const num = (key: string, placeholder: string) => (
    <input
      type="text"
      className="sf-num"
      placeholder={placeholder}
      value={value[key] ?? ""}
      onChange={(e) => set({ [key]: toNumber(e.target.value) })}
    />
  );

  let control: React.ReactNode;
  if (def.sockets) {
    control = (
      <>
        {(["r", "g", "b", "w"] as const).map((c) => (
          <span key={c} className={`sf-socket sf-socket-${c}`}>
            {num(c, c.toUpperCase())}
          </span>
        ))}
        {num("min", "min")}
        {num("max", "max")}
      </>
    );
  } else if (def.minMax && options) {
    control = (
      <>
        <select value={String(value.option ?? "null")} onChange={(e) => set({ option: optionId(options, e.target.value) ?? undefined })}>
          {options.map((o) => (
            <option key={String(o.id)} value={String(o.id)}>
              {o.text}
            </option>
          ))}
        </select>
        {num("min", "min")}
        {num("max", "max")}
      </>
    );
  } else if (def.minMax) {
    control = (
      <>
        {num("min", "min")}
        {num("max", "max")}
      </>
    );
  } else if (options) {
    control = (
      <select
        value={String(value.option ?? "null")}
        onChange={(e) => {
          const id = optionId(options, e.target.value);
          onChange(withFilter(query, group, def.id, id === null ? null : { option: id }));
        }}
      >
        {options.map((o) => (
          <option key={String(o.id)} value={String(o.id)}>
            {o.text}
          </option>
        ))}
      </select>
    );
  } else {
    // Free text: a known item's name ({option}) or, for the seller, {input}.
    const key = def.input ? "input" : "option";
    control = (
      <input
        type="text"
        className="sf-text"
        placeholder={def.input?.placeholder ?? "Any"}
        value={value[key] ?? ""}
        onChange={(e) => onChange(withFilter(query, group, def.id, { [key]: e.target.value }))}
      />
    );
  }

  return (
    <div className={`sf-field ${def.fullSpan ? "full" : ""}`} title={def.tip}>
      <span className="sf-label">{def.text ?? def.id}</span>
      <span className="sf-control">{control}</span>
    </div>
  );
}

/* Stat filters */

const GROUP_TYPES: [string, string][] = [
  ["and", "And"],
  ["not", "Not"],
  ["if", "If"],
  ["count", "Count"],
  ["weight", "Weighted Sum"],
];

function StatFilters({ query, onChange }: { query: Query; onChange: (q: Query) => void }) {
  const stats = useTradeData<StatGroupDef[]>("stats");
  const byId = useMemo(() => {
    const map = new Map<string, StatDef>();
    for (const g of stats ?? []) for (const e of g.entries) map.set(e.id, e);
    return map;
  }, [stats]);

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const groups: any[] = query.stats ?? [];

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  function change(edit: (groups: any[]) => void) {
    const next = copy(query);
    next.stats ??= [];
    edit(next.stats);
    onChange(next);
  }

  return (
    <section className="sf-stats">
      <div className="sf-group-head static">Stat filters</div>
      {groups.map((g, gi) => (
        <div key={gi} className={`sf-stat-group ${g.disabled ? "off" : ""}`}>
          <div className="sf-stat-group-head">
            <input
              type="checkbox"
              checked={!g.disabled}
              title="Use this group"
              onChange={(e) => change((s) => (s[gi].disabled = !e.target.checked))}
            />
            <select value={g.type ?? "and"} onChange={(e) => change((s) => (s[gi].type = e.target.value))}>
              {GROUP_TYPES.map(([id, text]) => (
                <option key={id} value={id}>
                  {text}
                </option>
              ))}
            </select>
            {(g.type === "count" || g.type === "weight") && (
              <>
                <input
                  type="text"
                  className="sf-num"
                  placeholder="min"
                  value={g.value?.min ?? ""}
                  onChange={(e) => change((s) => (s[gi].value = { ...s[gi].value, min: toNumber(e.target.value) }))}
                />
                <input
                  type="text"
                  className="sf-num"
                  placeholder="max"
                  value={g.value?.max ?? ""}
                  onChange={(e) => change((s) => (s[gi].value = { ...s[gi].value, max: toNumber(e.target.value) }))}
                />
              </>
            )}
            <button className="icon-btn" title="Remove this group" onClick={() => change((s) => s.splice(gi, 1))}>
              ×
            </button>
          </div>
          {/* eslint-disable-next-line @typescript-eslint/no-explicit-any */}
          {(g.filters ?? []).map((f: any, fi: number) => {
            const def = byId.get(f.id);
            const set = (patch: Record<string, unknown>) =>
              change((s) => {
                const v = { ...s[gi].filters[fi].value, ...patch };
                for (const k of Object.keys(v)) if (v[k] === undefined) delete v[k];
                s[gi].filters[fi].value = v;
              });
            return (
              <div key={fi} className={`sf-stat ${f.disabled ? "off" : ""}`}>
                <input
                  type="checkbox"
                  checked={!f.disabled}
                  onChange={(e) => change((s) => (s[gi].filters[fi].disabled = !e.target.checked))}
                />
                <span className="sf-stat-text">
                  {def ? (
                    <>
                      <span className="sf-stat-type">{def.type}</span> {def.text}
                    </>
                  ) : (
                    f.id
                  )}
                </span>
                {def?.option ? (
                  <select
                    className="sf-stat-option"
                    value={String(f.value?.option ?? "")}
                    onChange={(e) => set({ option: optionId(def.option!.options, e.target.value) ?? undefined })}
                  >
                    <option value="">Any</option>
                    {def.option.options.map((o) => (
                      <option key={String(o.id)} value={String(o.id)}>
                        {o.text}
                      </option>
                    ))}
                  </select>
                ) : (
                  <>
                    {g.type === "weight" && (
                      <input
                        type="text"
                        className="sf-num"
                        placeholder="weight"
                        value={f.value?.weight ?? ""}
                        onChange={(e) => set({ weight: toNumber(e.target.value) })}
                      />
                    )}
                    <input
                      type="text"
                      className="sf-num"
                      placeholder="min"
                      value={f.value?.min ?? ""}
                      onChange={(e) => set({ min: toNumber(e.target.value) })}
                    />
                    <input
                      type="text"
                      className="sf-num"
                      placeholder="max"
                      value={f.value?.max ?? ""}
                      onChange={(e) => set({ max: toNumber(e.target.value) })}
                    />
                  </>
                )}
                <button
                  className="icon-btn"
                  title="Remove this stat"
                  onClick={() => change((s) => s[gi].filters.splice(fi, 1))}
                >
                  ×
                </button>
              </div>
            );
          })}
          <StatPicker
            stats={stats}
            onPick={(id) => change((s) => (s[gi].filters ??= []).push({ id, value: {}, disabled: false }))}
          />
        </div>
      ))}
      {groups.length === 0 && (
        <StatPicker stats={stats} onPick={(id) => change((s) => s.push({ type: "and", filters: [{ id, value: {}, disabled: false }] }))} />
      )}
      <button className="btn small-btn sf-add-group" onClick={() => change((s) => s.push({ type: "and", filters: [] }))}>
        + Add stat group
      </button>
    </section>
  );
}

/** "+ Add stat filter": type words of a mod, pick it from the site's list. */
function StatPicker({ stats, onPick }: { stats: StatGroupDef[] | null; onPick: (id: string) => void }) {
  const [text, setText] = useState("");
  const all = useMemo(() => (stats ?? []).flatMap((g) => g.entries.map((e) => ({ ...e, label: g.label }))), [stats]);
  const words = text.toLowerCase().split(/\s+/).filter(Boolean);
  const matches =
    words.length === 0 ? [] : all.filter((s) => words.every((w) => s.text.toLowerCase().includes(w))).slice(0, 50);

  return (
    <div className="sf-item sf-stat-picker">
      <input
        type="text"
        placeholder={stats ? "+ Add stat filter" : "Loading stats…"}
        disabled={!stats}
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && matches[0]) {
            e.preventDefault();
            onPick(matches[0].id);
            setText("");
          }
          if (e.key === "Escape") setText("");
        }}
      />
      {matches.length > 0 && (
        <div className="sf-suggest">
          {matches.map((m) => (
            <button
              key={m.id}
              onMouseDown={() => {
                onPick(m.id);
                setText("");
              }}
            >
              <span>{m.text}</span>
              <small className="muted">{m.label}</small>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
