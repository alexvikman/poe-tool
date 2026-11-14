import { useEffect, useMemo, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  actOnListing,
  addGroup,
  addLink,
  clearSession,
  deleteGroup,
  deleteSearch,
  listGroups,
  liveState,
  onLiveHits,
  onLiveStatus,
  renameGroup,
  searchUrl,
  sessionStatus,
  setSession,
  signIn,
  updateSearch,
  type LiveHit,
  type LiveStatus,
  type SavedSearch,
  type SearchGroup,
  type SessionStatus,
} from "./searches";
import { timeAgo } from "./price";

/** "just now" under a minute, else "3 min ago". */
function since(iso: string): string {
  return Date.now() - new Date(iso).getTime() < 60_000 ? "just now" : `${timeAgo(iso)} ago`;
}

/** The sidebar's first entry: every live search and everything they found. */
const ALL = "__all";

function remembered(): string {
  try {
    return localStorage.getItem("searches.selected") ?? ALL;
  } catch {
    return ALL;
  }
}

function remember(id: string) {
  try {
    localStorage.setItem("searches.selected", id);
  } catch {
    // Only a convenience.
  }
}

export function SearchesPage() {
  const [groups, setGroups] = useState<SearchGroup[]>([]);
  const [statuses, setStatuses] = useState<Record<string, LiveStatus>>({});
  const [hits, setHits] = useState<LiveHit[]>([]);
  const [selected, setSelected] = useState<string>(remembered);
  const [error, setError] = useState<string | null>(null);

  const reload = () => listGroups().then(setGroups, (e) => setError(String(e)));

  useEffect(() => {
    reload();
    liveState().then(([s, h]) => {
      setStatuses(s);
      setHits(h);
    });
    const a = onLiveStatus(setStatuses);
    const b = onLiveHits(setHits);
    return () => {
      a.then((f) => f());
      b.then((f) => f());
    };
  }, []);

  async function run(action: Promise<unknown>) {
    try {
      await action;
      setError(null);
    } catch (e) {
      setError(String(e));
    }
    reload();
  }

  function select(id: string) {
    setSelected(id);
    remember(id);
  }

  const group = groups.find((g) => g.id === selected);

  return (
    <div className="searches">
      <BuildList
        groups={groups}
        statuses={statuses}
        hits={hits}
        selected={group ? group.id : ALL}
        onSelect={select}
        onAdd={(name) => run(addGroup(name).then((g) => select(g.id)))}
      />
      {/* Keyed by what's shown, so each opens at the top. */}
      <section className="searches-main" key={group ? group.id : ALL}>
        {error && (
          <p className="bad small banner" onClick={() => setError(null)}>
            {error}
          </p>
        )}
        {group ? (
          <BuildView
            key={group.id}
            group={group}
            statuses={statuses}
            hits={hits}
            run={run}
            onDeleted={() => select(ALL)}
          />
        ) : (
          // Also where a removed build falls back to.
          <Overview groups={groups} statuses={statuses} hits={hits} run={run} onSelect={select} />
        )}
      </section>
    </div>
  );
}

/* Sidebar */

function BuildList({
  groups,
  statuses,
  hits,
  selected,
  onSelect,
  onAdd,
}: {
  groups: SearchGroup[];
  statuses: Record<string, LiveStatus>;
  hits: LiveHit[];
  selected: string;
  onSelect: (id: string) => void;
  onAdd: (name: string) => void;
}) {
  const [filter, setFilter] = useState("");
  const [adding, setAdding] = useState(false);
  const [name, setName] = useState("");

  const shown = groups.filter((g) => g.name.toLowerCase().includes(filter.trim().toLowerCase()));
  const liveIds = groups.flatMap((g) => g.searches.filter((s) => s.live).map((s) => s.id));
  const available = hits.filter((h) => !h.gone).length;

  return (
    <aside className="builds">
      <SessionBox />
      <button className={`build-item ${selected === ALL ? "active" : ""}`} onClick={() => onSelect(ALL)}>
        <span className="build-name">Live overview</span>
        <span className="build-meta">
          {liveIds.length > 0 && <LiveDot statuses={statuses} ids={liveIds} />}
          {available > 0 && <span className="count">{available}</span>}
        </span>
      </button>

      <div className="builds-head">
        <span className="label">Builds</span>
        <button className="icon-btn" title="New build" onClick={() => setAdding((a) => !a)}>
          +
        </button>
      </div>
      {adding && (
        <form
          className="builds-add"
          onSubmit={(e) => {
            e.preventDefault();
            if (!name.trim()) return;
            onAdd(name.trim());
            setName("");
            setAdding(false);
          }}
        >
          <input
            autoFocus
            type="text"
            placeholder="e.g. Shockwave Cyclone"
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => e.key === "Escape" && setAdding(false)}
          />
        </form>
      )}
      {groups.length > 6 && (
        <input
          className="builds-filter"
          type="search"
          placeholder="Find a build"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        />
      )}
      <div className="builds-scroll">
        {shown.map((g) => {
          const ids = g.searches.filter((s) => s.live).map((s) => s.id);
          return (
            <button
              key={g.id}
              className={`build-item ${selected === g.id ? "active" : ""}`}
              onClick={() => onSelect(g.id)}
            >
              <span className="build-name">{g.name}</span>
              <span className="build-meta">
                {ids.length > 0 && <LiveDot statuses={statuses} ids={ids} />}
                <span className="muted">{g.searches.length}</span>
              </span>
            </button>
          );
        })}
        {groups.length === 0 && !adding && (
          <p className="muted small builds-empty">Press + to make a build and keep its searches together.</p>
        )}
      </div>
    </aside>
  );
}

/** Green when every watched search is connected, red when one has a problem. */
function LiveDot({ statuses, ids }: { statuses: Record<string, LiveStatus>; ids: string[] }) {
  const states = ids.map((id) => statuses[id]?.state);
  const tone = states.some((s) => s === "failed" || s === "retrying")
    ? "bad"
    : states.every((s) => s === "live")
      ? "good"
      : "muted";
  return <span className={`live-dot ${tone}`} title={`${ids.length} watched live`} />;
}

function SessionBox() {
  const [status, setStatus] = useState<SessionStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [pasting, setPasting] = useState(false);
  const [value, setValue] = useState("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    sessionStatus().then(setStatus, (e) => setError(String(e)));
  }, []);

  async function act(action: Promise<SessionStatus>) {
    setBusy(true);
    try {
      setStatus(await action);
      setError(null);
      setPasting(false);
      setValue("");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  if (!status) return <div className="session-box muted small">…</div>;

  if (status.set && status.account) {
    return (
      <div className="session-box">
        <div className="session-line">
          <span className="live-dot good" />
          <span className="session-account" title="Signed in on pathofexile.com">
            {status.account}
          </span>
          <button
            className="link-btn"
            onClick={() => clearSession().then(() => setStatus({ set: false, account: null, error: null }))}
          >
            Sign out
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="session-box">
      <button className="btn primary" disabled={busy} onClick={() => act(signIn())}>
        {busy ? "Waiting for sign-in…" : "Sign in to pathofexile.com"}
      </button>
      {status.set && status.error && <p className="bad small">{status.error}</p>}
      {error && <p className="bad small">{error}</p>}
      <button className="link-btn" onClick={() => setPasting((p) => !p)}>
        {pasting ? "Cancel" : "Or paste a POESESSID"}
      </button>
      {pasting && (
        <form
          className="inline-form"
          onSubmit={(e) => {
            e.preventDefault();
            act(setSession(value));
          }}
        >
          <input
            type="password"
            placeholder="POESESSID"
            value={value}
            onChange={(e) => setValue(e.target.value)}
            autoComplete="off"
          />
          <button className="btn" type="submit" disabled={!value.trim() || busy}>
            Save
          </button>
        </form>
      )}
      <p className="hint">
        Live searches and buying act as you on the trade site. Sign-in happens on pathofexile.com's own page; PoE
        Tool keeps only the session, in Windows Credential Manager.
      </p>
    </div>
  );
}

/* Main area */

function Overview({
  groups,
  statuses,
  hits,
  run,
  onSelect,
}: {
  groups: SearchGroup[];
  statuses: Record<string, LiveStatus>;
  hits: LiveHit[];
  run: (p: Promise<unknown>) => void;
  onSelect: (id: string) => void;
}) {
  const live = groups.flatMap((g) => g.searches.filter((s) => s.live).map((s) => ({ group: g, search: s })));
  return (
    <>
      <header className="main-head">
        <div>
          <h1>Live overview</h1>
          <p className="lede">Every search you watch, and what they've found, newest first.</p>
        </div>
      </header>
      {live.length > 0 && (
        <div className="search-table">
          {live.map(({ group, search }) => (
            <SearchRow
              key={search.id}
              search={search}
              status={statuses[search.id]}
              hits={hits}
              run={run}
              build={group.name}
              onBuild={() => onSelect(group.id)}
            />
          ))}
        </div>
      )}
      <HitFeed hits={hits} searches={live.map((l) => l.search)} />
    </>
  );
}

function BuildView({
  group,
  statuses,
  hits,
  run,
  onDeleted,
}: {
  group: SearchGroup;
  statuses: Record<string, LiveStatus>;
  hits: LiveHit[];
  run: (p: Promise<unknown>) => void;
  onDeleted: () => void;
}) {
  const [title, setTitle] = useState(group.name);
  const [link, setLink] = useState("");
  const [name, setName] = useState("");
  useEffect(() => setTitle(group.name), [group.name]);

  const ids = new Set(group.searches.map((s) => s.id));
  const own = hits.filter((h) => ids.has(h.searchId));

  return (
    <>
      <header className="main-head">
        <input
          className="build-title"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          onBlur={() => title.trim() && title !== group.name && run(renameGroup(group.id, title))}
          onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
        />
        <button
          className="btn"
          onClick={() => {
            if (confirm(`Remove ${group.name} and its ${group.searches.length} searches?`)) {
              run(deleteGroup(group.id));
              onDeleted();
            }
          }}
        >
          Remove build
        </button>
      </header>

      <div className="search-table">
        {group.searches.length === 0 && (
          <p className="muted small empty-row">
            No searches yet. Paste a link from the trade site below, or press Save search in the price window.
          </p>
        )}
        {group.searches.map((s) => (
          <SearchRow key={s.id} search={s} status={statuses[s.id]} hits={hits} run={run} />
        ))}
        <form
          className="add-search"
          onSubmit={(e) => {
            e.preventDefault();
            if (!link.trim()) return;
            run(
              addLink(group.id, link, name).then(() => {
                setLink("");
                setName("");
              }),
            );
          }}
        >
          <input
            type="text"
            placeholder="Paste a trade site search link"
            value={link}
            onChange={(e) => setLink(e.target.value)}
          />
          <input
            type="text"
            className="short"
            placeholder="Name (optional)"
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
          <button className="btn" type="submit" disabled={!link.trim()}>
            Add search
          </button>
        </form>
      </div>

      <HitFeed hits={own} searches={group.searches.filter((s) => s.live)} />
    </>
  );
}

function SearchRow({
  search,
  status,
  hits,
  run,
  build,
  onBuild,
}: {
  search: SavedSearch;
  status: LiveStatus | undefined;
  hits: LiveHit[];
  run: (p: Promise<unknown>) => void;
  /** The build's name, where searches from several builds are listed together. */
  build?: string;
  onBuild?: () => void;
}) {
  const [name, setName] = useState(search.name);
  const [opening, setOpening] = useState(false);
  useEffect(() => setName(search.name), [search.name]);

  const found = hits.filter((h) => h.searchId === search.id);

  async function open() {
    setOpening(true);
    try {
      await openUrl(await searchUrl(search.id));
    } catch (e) {
      alert(String(e));
    } finally {
      setOpening(false);
    }
  }

  const state = !search.live
    ? null
    : !status || status.state === "connecting"
      ? { label: "Connecting", tone: "muted" }
      : status.state === "live"
        ? { label: "Live", tone: "good" }
        : { label: status.message, tone: "bad" };

  return (
    <div className="search-row">
      <label className="switch small-switch" title={search.live ? "Stop watching" : "Watch live"}>
        <input
          type="checkbox"
          checked={search.live}
          onChange={(e) => run(updateSearch(search.id, { live: e.target.checked }))}
        />
        <span />
      </label>
      <div className="search-main">
        <input
          className="search-name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          onBlur={() => name.trim() && name !== search.name && run(updateSearch(search.id, { name }))}
          onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
        />
        <small className="muted">
          {build && (
            <>
              <button className="link-btn" onClick={onBuild}>
                {build}
              </button>
              {" · "}
            </>
          )}
          {search.league}
          {state && <span className={`live-state ${state.tone}`}> · {state.label}</span>}
        </small>
      </div>
      <small className="muted search-found">
        {found.length > 0 && `${found.length} found · last ${since(found[0].seen)}`}
      </small>
      <button className="btn small-btn" onClick={open} disabled={opening} title="Open this search on the trade site">
        {opening ? "…" : "Trade ↗"}
      </button>
      <button className="icon-btn" title="Remove this search" onClick={() => run(deleteSearch(search.id))}>
        ×
      </button>
    </div>
  );
}

/* Hits */

function HitFeed({ hits, searches }: { hits: LiveHit[]; searches: SavedSearch[] }) {
  const [only, setOnly] = useState<string | null>(null);
  const [hideGone, setHideGone] = useState(false);
  const [, tick] = useState(0);
  // Keeps "2 min ago" moving.
  useEffect(() => {
    const t = setInterval(() => tick((n) => n + 1), 30_000);
    return () => clearInterval(t);
  }, []);

  const counts = useMemo(() => {
    const c: Record<string, number> = {};
    for (const h of hits) c[h.searchId] = (c[h.searchId] ?? 0) + 1;
    return c;
  }, [hits]);

  const shown = hits.filter((h) => (!only || h.searchId === only) && !(hideGone && h.gone));
  const filterable = searches.filter((s) => counts[s.id]);

  return (
    <section className="feed">
      <div className="feed-head">
        <span className="label">Found</span>
        <div className="chips">
          {filterable.length > 1 && (
            <>
              <button className={`chip ${only === null ? "on" : ""}`} onClick={() => setOnly(null)}>
                All <span className="muted">{hits.length}</span>
              </button>
              {filterable.map((s) => (
                <button key={s.id} className={`chip ${only === s.id ? "on" : ""}`} onClick={() => setOnly(s.id)}>
                  {s.name} <span className="muted">{counts[s.id]}</span>
                </button>
              ))}
            </>
          )}
        </div>
        <label className="check small">
          <input type="checkbox" checked={hideGone} onChange={(e) => setHideGone(e.target.checked)} /> Hide sold
        </label>
      </div>
      {shown.length === 0 && (
        <p className="muted small">
          {searches.length === 0
            ? "Turn on Live for a search, and new listings show up here the moment they're listed."
            : "Watching. New listings show up here the moment they're listed."}
        </p>
      )}
      {shown.map((h) => (
        <HitCard key={h.id} hit={h} />
      ))}
    </section>
  );
}

function HitCard({ hit }: { hit: LiveHit }) {
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<{ text: string; bad: boolean } | null>(null);
  const { item } = hit;

  async function act() {
    setBusy(true);
    setNote(null);
    try {
      const did = await actOnListing(hit.id);
      setNote({ text: did === "travel" ? "Travelling to the hideout" : "Whisper sent", bad: false });
    } catch (e) {
      setNote({ text: String(e), bad: true });
    } finally {
      setBusy(false);
    }
  }

  const facts = [item.itemLevel != null && `Item level ${item.itemLevel}`, item.sockets, ...item.properties].filter(
    Boolean,
  ) as string[];

  return (
    <article className={`hit-card ${hit.gone ? "gone" : ""}`}>
      <div className="hit-icon">{item.icon && <img src={item.icon} alt="" loading="lazy" />}</div>
      <div className="hit-body">
        <div className={`hit-title rarity-${item.rarity}`}>
          {item.name || item.typeLine}
          {item.name && <span className="hit-base"> {item.typeLine}</span>}
        </div>
        {facts.length > 0 && <div className="muted small">{facts.join(" · ")}</div>}
        {item.mods.length > 0 && (
          <ul className="hit-mods">
            {item.mods.map((m, i) => (
              <li key={i} className={`mod-${m.kind}`}>
                {m.text}
                {m.range && <span className="mod-range"> ({m.range})</span>}
              </li>
            ))}
          </ul>
        )}
        {item.corrupted && <div className="corrupted small">Corrupted</div>}
      </div>
      <div className="hit-side">
        <div className="hit-price">{hit.amount != null ? `${hit.amount} ${hit.currency}` : "No price"}</div>
        <small className="muted">
          {hit.account} · {since(hit.seen)}
        </small>
        <small className="muted">{hit.searchName}</small>
        {hit.gone ? (
          <span className="bad small">No longer available</span>
        ) : hit.hideoutToken ? (
          <button className="btn primary small-btn" disabled={busy} onClick={act}>
            {busy ? "…" : "Travel to hideout"}
          </button>
        ) : hit.whisperToken ? (
          <button className="btn small-btn" disabled={busy} onClick={act}>
            {busy ? "…" : "Whisper"}
          </button>
        ) : hit.whisper ? (
          <button className="btn small-btn" onClick={() => navigator.clipboard.writeText(hit.whisper!)}>
            Copy whisper
          </button>
        ) : null}
        {note && <small className={note.bad ? "bad" : "good"}>{note.text}</small>}
      </div>
    </article>
  );
}
