import { useEffect, useState } from "react";
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
  tradeCurrencies,
  type Currency,
  type SearchSort,
  type SearchDetail,
  searchDetail,
  editSearch,
  duplicateSearch,
  runSavedSearch,
  runSearch,
  searchMore,
  addQuery,
  type SearchPage,
  type LiveHit,
  type LiveStatus,
  type SavedSearch,
  type SearchGroup,
  type SessionStatus,
} from "./searches";
import { getLeagues, timeAgo } from "./price";
import { SearchForm } from "./SearchForm";
import { ItemPicture, ItemTooltip, Sortable, type Sorting } from "./ItemView";

/** "just now" under a minute, else "3 min ago". */
function since(iso: string): string {
  return Date.now() - new Date(iso).getTime() < 60_000 ? "just now" : `${timeAgo(iso)} ago`;
}

/** The sidebar's first entry: searching the trade site with its own filters. */
const SEARCH = "__search";
/** Every live search and everything they found. */
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
  const view = group ? group.id : selected === SEARCH ? SEARCH : ALL;

  return (
    <div className="searches">
      <BuildList
        groups={groups}
        statuses={statuses}
        hits={hits}
        selected={view}
        onSelect={select}
        onAdd={(name) => run(addGroup(name).then((g) => select(g.id)))}
      />
      {/* Keyed by what's shown, so each opens at the top. */}
      <section className="searches-main" key={view}>
        {error && (
          <p className="bad small banner" onClick={() => setError(null)}>
            {error}
          </p>
        )}
        {view === SEARCH ? (
          <TradeSearchView
            groups={groups}
            onSaved={(groupId, searchId) => {
              rememberSearch(groupId, searchId);
              reload();
              select(groupId);
            }}
          />
        ) : group ? (
          <BuildView
            key={group.id}
            group={group}
            groups={groups}
            statuses={statuses}
            hits={hits}
            run={run}
            onDeleted={() => select(ALL)}
          />
        ) : (
          // Also where a removed build falls back to.
          <Overview groups={groups} statuses={statuses} hits={hits} run={run} />
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
      <button className={`build-item ${selected === SEARCH ? "active" : ""}`} onClick={() => onSelect(SEARCH)}>
        <span className="build-name">Search</span>
      </button>
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

/* Main area: a build's searches on the left, the chosen one's finds beside them */

/** When each search's finds were last looked at, so new ones can be counted. */
function useSeen(): [Record<string, number>, (id: string) => void] {
  const [seen, setSeen] = useState<Record<string, number>>(() => {
    try {
      return JSON.parse(localStorage.getItem("searches.seen") ?? "{}");
    } catch {
      return {};
    }
  });
  const mark = (id: string) =>
    setSeen((old) => {
      const next = { ...old, [id]: Date.now() };
      try {
        localStorage.setItem("searches.seen", JSON.stringify(next));
      } catch {
        // Only a convenience.
      }
      return next;
    });
  return [seen, mark];
}

function rememberedSearch(view: string): string | null {
  try {
    return localStorage.getItem(`searches.open.${view}`);
  } catch {
    return null;
  }
}

function rememberSearch(view: string, id: string) {
  try {
    localStorage.setItem(`searches.open.${view}`, id);
  } catch {
    // Only a convenience.
  }
}

interface Entry {
  search: SavedSearch;
  group: SearchGroup;
}

/** "All finds" in the overview's list. */
const EVERYTHING = "__everything";

function Board({
  view,
  entries,
  groups,
  statuses,
  hits,
  run,
  showBuild,
  allowAll,
  footer,
  empty,
}: {
  /** The build's id, or ALL: where the open search is remembered. */
  view: string;
  entries: Entry[];
  groups: SearchGroup[];
  statuses: Record<string, LiveStatus>;
  hits: LiveHit[];
  run: (p: Promise<unknown>) => void;
  /** Name each search's build (the overview lists several builds). */
  showBuild?: boolean;
  /** Offer "All finds" at the top. */
  allowAll?: boolean;
  footer?: React.ReactNode;
  empty: string;
}) {
  const [seen, markSeen] = useSeen();
  const [open, setOpen] = useState<string | null>(() => rememberedSearch(view));
  // The edit form gets the whole width, as the trade site's form has.
  const [editing, setEditing] = useState(false);
  // A copy just made opens in its filters, to be changed.
  const [editNext, setEditNext] = useState<string | null>(null);

  const ids = entries.map((e) => e.search.id);
  const current =
    open === EVERYTHING && allowAll ? EVERYTHING : open && ids.includes(open) ? open : (ids[0] ?? null);
  const entry = entries.find((e) => e.search.id === current);

  function choose(id: string) {
    setOpen(id);
    rememberSearch(view, id);
  }

  const own = (id: string) => hits.filter((h) => h.searchId === id);
  const fresh = (id: string) =>
    own(id).filter((h) => !h.gone && Date.parse(h.seen) > (seen[id] ?? 0)).length;

  // What's on screen counts as seen, including finds that arrive while it is.
  const shownCount = entry ? own(entry.search.id).length : 0;
  useEffect(() => {
    if (entry) markSeen(entry.search.id);
  }, [entry?.search.id, shownCount]);

  return (
    <div className={`board ${editing ? "editing" : ""}`}>
      <div className="search-list">
        {allowAll && entries.length > 0 && (
          <button className={`search-item ${current === EVERYTHING ? "active" : ""}`} onClick={() => choose(EVERYTHING)}>
            <span className="search-item-main">
              <span className="search-item-name">All finds</span>
              <small className="muted">Every live search, newest first</small>
            </span>
            <span className="build-meta">
              <span className="muted">{hits.filter((h) => !h.gone).length}</span>
            </span>
          </button>
        )}
        {entries.map(({ search, group }) => {
          const status = statuses[search.id];
          const n = fresh(search.id);
          const found = own(search.id);
          return (
            <button
              key={search.id}
              className={`search-item ${current === search.id ? "active" : ""}`}
              onClick={() => choose(search.id)}
            >
              <span className="search-item-main">
                <span className="search-item-name">{search.name}</span>
                <small className="muted">
                  {showBuild ? `${group.name} · ` : ""}
                  {found.length > 0 ? `last ${since(found[0].seen)}` : search.league}
                </small>
              </span>
              <span className="build-meta">
                {search.live && <StatusDot status={status} />}
                {n > 0 ? (
                  <span className="count" title="New since you last looked">
                    {n}
                  </span>
                ) : (
                  found.length > 0 && <span className="muted">{found.filter((h) => !h.gone).length}</span>
                )}
              </span>
            </button>
          );
        })}
        {entries.length === 0 && <p className="muted small empty-row">{empty}</p>}
        {footer}
      </div>
      <div className="search-pane">
        {current === EVERYTHING ? (
          <HitFeed hits={hits} />
        ) : entry ? (
          <SearchPane
            key={entry.search.id}
            entry={entry}
            groups={groups}
            status={statuses[entry.search.id]}
            hits={own(entry.search.id)}
            run={run}
            onEditing={setEditing}
            startEditing={editNext === entry.search.id}
            onStartedEditing={() => setEditNext(null)}
            onDuplicate={() =>
              run(
                duplicateSearch(entry.search.id).then((copy) => {
                  setEditNext(copy.id);
                  choose(copy.id);
                }),
              )
            }
          />
        ) : null}
      </div>
    </div>
  );
}

function StatusDot({ status }: { status: LiveStatus | undefined }) {
  const tone = !status || status.state === "connecting" ? "muted" : status.state === "live" ? "good" : "bad";
  const title = !status
    ? "Connecting"
    : status.state === "live"
      ? "Live"
      : status.state === "connecting"
        ? "Connecting"
        : status.message;
  return <span className={`live-dot ${tone}`} title={title} />;
}

function Overview({
  groups,
  statuses,
  hits,
  run,
}: {
  groups: SearchGroup[];
  statuses: Record<string, LiveStatus>;
  hits: LiveHit[];
  run: (p: Promise<unknown>) => void;
}) {
  const entries = groups.flatMap((g) => g.searches.filter((s) => s.live).map((s) => ({ group: g, search: s })));
  return (
    <>
      <header className="main-head">
        <div>
          <h1>Live overview</h1>
          <p className="lede">Every search you watch, across your builds.</p>
        </div>
      </header>
      <Board
        view={ALL}
        entries={entries}
        groups={groups}
        statuses={statuses}
        hits={hits}
        run={run}
        showBuild
        allowAll
        empty="No search is live. Open a build and turn on Live for a search."
      />
    </>
  );
}

function BuildView({
  group,
  groups,
  statuses,
  hits,
  run,
  onDeleted,
}: {
  group: SearchGroup;
  groups: SearchGroup[];
  statuses: Record<string, LiveStatus>;
  hits: LiveHit[];
  run: (p: Promise<unknown>) => void;
  onDeleted: () => void;
}) {
  const [title, setTitle] = useState(group.name);
  const [link, setLink] = useState("");
  const [name, setName] = useState("");
  useEffect(() => setTitle(group.name), [group.name]);

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
      <Board
        view={group.id}
        entries={group.searches.map((search) => ({ search, group }))}
        groups={groups}
        statuses={statuses}
        hits={hits}
        run={run}
        empty="No searches yet."
        footer={
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
              placeholder="Name (optional: taken from the search)"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
            <button className="btn" type="submit" disabled={!link.trim()}>
              Add search
            </button>
            <p className="hint">Or press Save search in the price window.</p>
          </form>
        }
      />
    </>
  );
}

/** One search: what it looks for, its controls, and what it has found. */
function SearchPane({
  entry,
  groups,
  status,
  hits,
  run,
  onEditing,
  startEditing,
  onStartedEditing,
  onDuplicate,
}: {
  entry: Entry;
  groups: SearchGroup[];
  status: LiveStatus | undefined;
  hits: LiveHit[];
  run: (p: Promise<unknown>) => void;
  onEditing: (editing: boolean) => void;
  /** Open in the search form (a copy just made), once. */
  startEditing?: boolean;
  onStartedEditing?: () => void;
  onDuplicate: () => void;
}) {
  const { search } = entry;
  // Live finds, what's listed now (an ordinary search), or the search form.
  const [tab, setTab] = useState<"finds" | "listed" | "edit">(startEditing ? "edit" : "finds");
  useEffect(() => {
    if (startEditing) onStartedEditing?.();
  }, []);
  // Asks Listed now to search: "Save and search" in the form does.
  const [searchAsked, setSearchAsked] = useState(0);
  useEffect(() => onEditing(tab === "edit"), [tab]);
  // Leaving the search (or the page) ends editing.
  useEffect(() => () => onEditing(false), []);
  const [opening, setOpening] = useState(false);
  const editing = tab === "edit";

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
    ? "Not watched"
    : !status || status.state === "connecting"
      ? "Connecting"
      : status.state === "live"
        ? "Live"
        : status.message;
  const tone = !search.live ? "muted" : status?.state === "live" ? "good" : status && status.state !== "connecting" ? "bad" : "muted";

  return (
    <>
      <div className="pane-head">
        <div className="pane-title">
          <h2>{search.name}</h2>
          <small className="muted">
            {search.league} · <span className={`live-state ${tone}`}>{state}</span>
          </small>
        </div>
        <div className="pane-actions">
          <label className="live-toggle" title={search.live ? "Stop watching" : "Watch for new listings"}>
            <span className="switch small-switch">
              <input
                type="checkbox"
                checked={search.live}
                onChange={(e) => run(updateSearch(search.id, { live: e.target.checked }))}
              />
              <span />
            </span>
            Live
          </label>
          <button className="btn small-btn" onClick={open} disabled={opening} title="Open this search on the trade site">
            {opening ? "…" : "Trade ↗"}
          </button>
          <button className="btn small-btn" onClick={onDuplicate} title="Make a copy of this search to change">
            Duplicate
          </button>
          <button
            className="icon-btn"
            title="Remove this search"
            onClick={() => confirm(`Remove ${search.name}?`) && run(deleteSearch(search.id))}
          >
            ×
          </button>
        </div>
      </div>
      <div className="tabs">
        <button className={`tab ${tab === "finds" ? "on" : ""}`} onClick={() => setTab("finds")}>
          Live finds {hits.length > 0 && <span className="muted">{hits.filter((h) => !h.gone).length}</span>}
        </button>
        <button className={`tab ${tab === "listed" ? "on" : ""}`} onClick={() => setTab("listed")}>
          Listed now
        </button>
        <button className={`tab ${tab === "edit" ? "on" : ""}`} onClick={() => setTab("edit")}>
          Edit filters
        </button>
      </div>
      {editing && (
        <EditSearch
          id={search.id}
          groups={groups}
          onDone={(saved, thenSearch) => {
            setTab(thenSearch ? "listed" : "finds");
            if (saved) {
              // What was listed came from the old filters.
              searchRuns.delete(search.id);
              if (thenSearch) setSearchAsked((n) => n + 1);
              run(Promise.resolve());
            }
          }}
        />
      )}
      {tab === "listed" && (
        <Listings
          key={`${search.id}-${searchAsked}`}
          cacheKey={search.id}
          start={(sort) => runSavedSearch(search.id, sort)}
          asked={searchAsked}
          searchId={search.id}
          league={search.league}
        />
      )}
      {tab === "finds" && (
        <HitFeed
          hits={hits}
          waiting={
            !search.live
              ? "Turn on Live, and new listings show up here the moment they're listed."
              : status && (status.state === "failed" || status.state === "retrying")
                ? status.message
                : "Watching. New listings show up here the moment they're listed."
          }
        />
      )}
    </>
  );
}

/** Change a saved search: its name and build, and everything the trade site's search
 *  form has, or swap in another link from the site. */
function EditSearch({
  id,
  groups,
  onDone,
}: {
  id: string;
  groups: SearchGroup[];
  /** Closes the form; saved says whether anything changed, thenSearch whether to
   *  show what the search finds now. */
  onDone: (saved: boolean, thenSearch?: boolean) => void;
}) {
  const [detail, setDetail] = useState<SearchDetail | null>(null);
  const [leagues, setLeagues] = useState<string[]>([]);
  const [link, setLink] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    searchDetail(id).then(setDetail, (e) => setError(String(e)));
    getLeagues().then((l) => setLeagues(l.map((x) => x.id)), () => {});
  }, [id]);

  if (!detail) return <section className="edit-search">{error ? <p className="bad small">{error}</p> : "…"}</section>;

  const set = (patch: Partial<SearchDetail>) => setDetail({ ...detail, ...patch });

  async function save(thenSearch = false) {
    setSaving(true);
    try {
      await editSearch(id, { ...detail!, link: link.trim() || null });
      onDone(true, thenSearch);
    } catch (e) {
      // Stays open, so a bad link can be fixed.
      setError(String(e));
      setSaving(false);
    }
  }

  return (
    <section className="edit-search">
      <div className="edit-grid">
        <label>
          <span className="label">Name</span>
          <input type="text" value={detail.name} onChange={(e) => set({ name: e.target.value })} />
        </label>
        <label>
          <span className="label">Build</span>
          <select value={detail.groupId} onChange={(e) => set({ groupId: e.target.value })}>
            {groups.map((g) => (
              <option key={g.id} value={g.id}>
                {g.name}
              </option>
            ))}
          </select>
        </label>
      </div>

      <SearchForm
        query={detail.query}
        onChange={(query) => set({ query })}
        league={detail.league}
        leagues={leagues}
        onLeague={(league) => set({ league })}
      />

      <label>
        <span className="label">Or search for another link instead</span>
        <input
          type="text"
          placeholder="A link from the trade site replaces everything above but the name and build"
          value={link}
          onChange={(e) => setLink(e.target.value)}
        />
      </label>
      {error && <p className="bad small">{error}</p>}
      <div className="edit-actions">
        <button className="btn primary" onClick={() => save(true)} disabled={saving}>
          Save and search
        </button>
        <button className="btn" onClick={() => save()} disabled={saving}>
          Save
        </button>
        <button className="btn" onClick={() => onDone(false)}>
          Cancel
        </button>
      </div>
    </section>
  );
}

/* Ordinary searches */

/** An ordinary search's last results, kept while the app runs. */
interface SearchRun {
  /** The parent's request this answered (see `asked`). */
  asked: number;
  page: SearchPage | null;
  listings: LiveHit[];
  /** When it searched (ms). */
  at: number;
  error: string | null;
  /** What it's sorted by; none is by price, cheapest first. */
  sort: SearchSort | null;
}

/** By saved search id, or SEARCH for the search form's. */
const searchRuns = new Map<string, SearchRun>();

/** Listings of an ordinary search. It searches only when asked, so moving between
 *  searches costs nothing against the trade site's rate limit: by its own button
 *  (which also searches again) or when `asked` goes up. The last results stay. */
function Listings({
  cacheKey,
  start,
  asked = 0,
  button = true,
  searchId,
  league,
}: {
  cacheKey: string;
  start: (sort: SearchSort | null) => Promise<SearchPage>;
  /** Goes up each time the parent asks for a search (0: not asked). */
  asked?: number;
  /** Offer the Search button here (the search form has its own). */
  button?: boolean;
  /** The saved search they come from; none for the search form's. */
  searchId: string | null;
  league: string;
}) {
  const [run, setRun] = useState<SearchRun | undefined>(() => searchRuns.get(cacheKey));
  const [busy, setBusy] = useState(false);
  const [, tick] = useState(0);
  // Keeps "searched 2 min ago" moving.
  useEffect(() => {
    const t = setInterval(() => tick((n) => n + 1), 30_000);
    return () => clearInterval(t);
  }, []);

  function keep(next: SearchRun) {
    searchRuns.set(cacheKey, next);
    setRun(next);
  }

  async function search(forAsked: number, sort: SearchSort | null = searchRuns.get(cacheKey)?.sort ?? null) {
    setBusy(true);
    try {
      const page = await start(sort);
      keep({ asked: forAsked, page, listings: page.listings, at: Date.now(), error: null, sort });
    } catch (e) {
      const last = searchRuns.get(cacheKey);
      keep({
        asked: forAsked,
        page: last?.page ?? null,
        listings: last?.listings ?? [],
        at: Date.now(),
        error: String(e),
        sort: last?.sort ?? null,
      });
    } finally {
      setBusy(false);
    }
  }

  /** Price, cheapest first, unless a value was clicked. */
  const sorted: SearchSort = run?.sort ?? { key: "price", dir: "asc", label: "price" };

  /** As on the site: a value sorts highest first (price cheapest first); clicking it
   *  again turns the order round. Each sort is a new search. */
  function sortBy(key: string, label: string) {
    if (busy) return;
    const dir = sorted.key === key ? (sorted.dir === "desc" ? "asc" : "desc") : key === "price" ? "asc" : "desc";
    search(run?.asked ?? asked, key === "price" && dir === "asc" ? null : { key, dir, label });
  }

  useEffect(() => {
    if (asked > 0 && searchRuns.get(cacheKey)?.asked !== asked) search(asked);
  }, [asked]);

  async function more() {
    if (!run?.page) return;
    setBusy(true);
    try {
      const next = run.page.ids.slice(run.listings.length, run.listings.length + 20);
      const got = await searchMore(searchId, league, run.page.queryId, next);
      keep({ ...run, listings: [...run.listings, ...got], error: null });
    } catch (e) {
      keep({ ...run, error: String(e) });
    } finally {
      setBusy(false);
    }
  }

  const page = run?.page;
  const count = !page
    ? null
    : page.total === 0
      ? "Nothing listed"
      : `Showing ${run!.listings.length} of ${page.total >= 10000 ? "10,000+" : page.total.toLocaleString()}`;

  return (
    <section className="feed">
      <div className="feed-head">
        <span className="label">
          {busy && !page ? "Searching…" : count ?? "Not searched yet"}
          {run && <span className="muted"> · searched {since(new Date(run.at).toISOString())}</span>}
          {run?.sort && (
            <span className="muted">
              {" "}
              · sorted by {run.sort.label} {run.sort.dir === "desc" ? "▼" : "▲"} ·{" "}
              <button className="link-btn" onClick={() => !busy && search(run.asked, null)}>
                sort by price
              </button>
            </span>
          )}
        </span>
        {button && (
          <button className="btn small-btn" onClick={() => search(asked)} disabled={busy}>
            {busy ? "Searching…" : run ? "Search again" : "Search"}
          </button>
        )}
      </div>
      {!run && !busy && button && (
        <p className="muted small">What's listed right now, as the trade site's Search shows it. Each search counts against the site's rate limit, so it only searches when you press Search.</p>
      )}
      {run?.error && <p className="bad small">{run.error}</p>}
      {run?.listings.map((h) => (
        <HitCard key={h.id} hit={h} sorting={{ current: sorted, onSort: sortBy }} />
      ))}
      {page && run!.listings.length < page.ids.length && (
        <button className="btn load-more" onClick={more} disabled={busy}>
          {busy ? "…" : "Load more"}
        </button>
      )}
    </section>
  );
}

/** The search form's last search, kept while the app runs. */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const lastSearch: { query: any; league: string | null; run: number } = {
  query: { status: { option: "securable" } },
  league: null,
  run: 0,
};

/** Search the trade site like on the site itself, and keep a search in a build. */
export function TradeSearchView({
  groups,
  onSaved,
}: {
  groups: SearchGroup[];
  /** Opens the build a search was saved to, at that search. */
  onSaved: (groupId: string, searchId: string) => void;
}) {
  const [query, setQuery] = useState(lastSearch.query);
  const [league, setLeague] = useState<string | null>(lastSearch.league);
  const [leagues, setLeagues] = useState<string[]>([]);
  // Each search remounts the listings; 0 is none yet.
  const [run, setRun] = useState(lastSearch.run);
  const [showForm, setShowForm] = useState(lastSearch.run === 0);
  const [saving, setSaving] = useState(false);
  const [groupId, setGroupId] = useState("");
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getLeagues().then((l) => {
      const ids = l.map((x) => x.id);
      setLeagues(ids);
      setLeague((cur) => cur ?? ids[0] ?? null);
    }, () => {});
  }, []);

  useEffect(() => {
    lastSearch.query = query;
    lastSearch.league = league;
    lastSearch.run = run;
  }, [query, league, run]);

  function search() {
    setRun((n) => n + 1);
    setShowForm(false);
  }

  async function save() {
    const target = groupId || groups[0]?.id;
    if (!target || !league) return;
    try {
      const saved = await addQuery(target, name, league, query);
      setSaving(false);
      setName("");
      onSaved(target, saved.id);
    } catch (e) {
      setError(String(e));
    }
  }

  const type = typeof query.type === "string" ? query.type : query.type?.option;
  const what = [query.name, type].filter(Boolean).join(" ") || query.term || "Any item";
  const statCount = (query.stats ?? []).reduce(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (n: number, g: any) => n + (g.filters ?? []).filter((f: any) => !f.disabled).length,
    0,
  );

  return (
    <>
      <header className="main-head">
        <div>
          <h1>Search</h1>
          <p className="lede">Search the trade site with its own filters. Keep a search in a build to watch it live.</p>
        </div>
      </header>

      {showForm ? (
        league && (
          <SearchForm query={query} onChange={setQuery} league={league} leagues={leagues} onLeague={setLeague} />
        )
      ) : (
        <button className="search-summary" onClick={() => setShowForm(true)}>
          <span>{what}</span>
          <span className="muted small">
            {league} · {statCount} stat{statCount === 1 ? "" : "s"} · Show filters
          </span>
        </button>
      )}

      <div className="search-actions">
        <button className="btn primary" onClick={search} disabled={!league}>
          Search
        </button>
        {showForm && run > 0 && (
          <button className="btn" onClick={() => setShowForm(false)}>
            Hide filters
          </button>
        )}
        <button className="btn" onClick={() => setQuery({ status: query.status ?? { option: "securable" } })}>
          Clear
        </button>
        <span className="spacer" />
        {saving ? (
          <>
            <select value={groupId || groups[0]?.id || ""} onChange={(e) => setGroupId(e.target.value)}>
              {groups.map((g) => (
                <option key={g.id} value={g.id}>
                  {g.name}
                </option>
              ))}
            </select>
            <input
              type="text"
              placeholder={`Name (${what})`}
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
            <button className="btn primary" onClick={save} disabled={groups.length === 0}>
              Save
            </button>
            <button className="btn" onClick={() => setSaving(false)}>
              Cancel
            </button>
          </>
        ) : (
          <button className="btn" onClick={() => setSaving(true)} disabled={groups.length === 0} title={groups.length === 0 ? "Make a build first" : undefined}>
            Save as search…
          </button>
        )}
      </div>
      {error && <p className="bad small">{error}</p>}

      {run > 0 && league && (
        <Listings
          cacheKey={SEARCH}
          start={(sort) => runSearch(league, query, sort)}
          asked={run}
          button={false}
          searchId={null}
          league={league}
        />
      )}
    </>
  );
}

/* Hits */

function HitFeed({
  hits,
  waiting = "Watching. New listings show up here the moment they're listed.",
}: {
  hits: LiveHit[];
  /** What to say while nothing has been found. */
  waiting?: string;
}) {
  const [hideGone, setHideGone] = useState(false);
  const [, tick] = useState(0);
  // Keeps "2 min ago" moving.
  useEffect(() => {
    const t = setInterval(() => tick((n) => n + 1), 30_000);
    return () => clearInterval(t);
  }, []);

  const shown = hits.filter((h) => !(hideGone && h.gone));
  const gone = hits.filter((h) => h.gone).length;

  return (
    <section className="feed">
      <div className="feed-head">
        <span className="label">
          Found {hits.length > 0 && <span className="muted">{hits.length - gone} available{gone > 0 && `, ${gone} sold`}</span>}
        </span>
        {gone > 0 && (
          <label className="check small">
            <input type="checkbox" checked={hideGone} onChange={(e) => setHideGone(e.target.checked)} /> Hide sold
          </label>
        )}
      </div>
      {shown.length === 0 && (
        <p className="muted small">{waiting}</p>
      )}
      {shown.map((h) => (
        <HitCard key={h.id} hit={h} />
      ))}
    </section>
  );
}

let currencies: Promise<Record<string, Currency>> | null = null;

/** Currency names and pictures, fetched once. */
function useCurrencies(): Record<string, Currency> {
  const [map, setMap] = useState<Record<string, Currency>>({});
  useEffect(() => {
    currencies ??= tradeCurrencies().catch(() => {
      currencies = null; // Try again next time.
      return {};
    });
    currencies.then(setMap);
  }, []);
  return map;
}

function HitCard({ hit, sorting }: { hit: LiveHit; sorting?: Sorting }) {
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<{ text: string; bad: boolean } | null>(null);
  const currency = useCurrencies()[hit.currency ?? ""];

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

  return (
    <article className={`hit-card ${hit.gone ? "gone" : ""}`}>
      <div className="hit-icon">
        <ItemPicture item={hit.item} />
      </div>
      <ItemTooltip
        item={hit.item}
        note={hit.amount != null ? `${hit.priceType ?? "~price"} ${hit.amount} ${hit.currency}` : null}
        sorting={sorting}
      />
      <div className="hit-side">
        <span className="muted small">{hit.priceType === "~price" ? "Asking price" : "Buyout"}</span>
        <Sortable sorting={sorting} sortKey="price" label="price" row>
          <div className="hit-price">
            {hit.amount == null ? (
              "No price"
            ) : (
              <>
                {hit.amount}×{currency?.icon && <img className="currency-icon" src={currency.icon} alt="" />}
                <span>{currency?.name ?? hit.currency}</span>
              </>
            )}
          </div>
        </Sortable>
        <small className="muted">
          <span className="seller">{hit.account}</span> ·{" "}
          <Sortable sorting={sorting} sortKey="indexed" label="when listed">
            listed {since(hit.seen)}
          </Sortable>
        </small>
        <small className="muted">{hit.searchName}</small>
        {hit.gone ? (
          <span className="bad small">Item no longer available</span>
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

