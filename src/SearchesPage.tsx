import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  addGroup,
  addLink,
  clearSession,
  deleteGroup,
  deleteSearch,
  listGroups,
  liveState,
  onLiveHit,
  onLiveStatus,
  renameGroup,
  searchUrl,
  sessionStatus,
  setSession,
  updateSearch,
  actOnListing,
  type LiveHit,
  type LiveStatus,
  type SearchGroup,
  type SessionStatus,
} from "./searches";
import { timeAgo } from "./price";

export function SearchesPage() {
  const [groups, setGroups] = useState<SearchGroup[]>([]);
  const [statuses, setStatuses] = useState<Record<string, LiveStatus>>({});
  const [hits, setHits] = useState<LiveHit[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [newGroup, setNewGroup] = useState("");

  const reload = () => listGroups().then(setGroups, (e) => setError(String(e)));

  useEffect(() => {
    reload();
    liveState().then(([s, h]) => {
      setStatuses(s);
      setHits(h);
    });
    const a = onLiveStatus(setStatuses);
    const b = onLiveHit((hit) => setHits((h) => [hit, ...h].slice(0, 100)));
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

  return (
    <div className="page">
      <header className="page-head">
        <div>
          <h1>Searches</h1>
          <p className="lede">
            Keep trade searches by build. Paste a link from the trade site, or press Save search in the price
            window. Turn on Live to be told the moment a new item is listed, and buy it from here.
          </p>
        </div>
      </header>

      <SessionCard />

      <section className="card">
        <form
          className="inline-form"
          onSubmit={(e) => {
            e.preventDefault();
            if (!newGroup.trim()) return;
            run(addGroup(newGroup).then(() => setNewGroup("")));
          }}
        >
          <input
            type="text"
            placeholder="New build, e.g. Shockwave Cyclone"
            value={newGroup}
            onChange={(e) => setNewGroup(e.target.value)}
          />
          <button className="btn primary" type="submit" disabled={!newGroup.trim()}>
            Add build
          </button>
        </form>
        {error && <p className="bad small">{error}</p>}
      </section>

      {groups.map((g) => (
        <GroupCard key={g.id} group={g} statuses={statuses} run={run} />
      ))}

      <LiveFeed hits={hits} />
    </div>
  );
}

function SessionCard() {
  const [status, setStatus] = useState<SessionStatus | null>(null);
  const [value, setValue] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    sessionStatus().then(setStatus, (e) => setError(String(e)));
  }, []);

  async function save() {
    setBusy(true);
    try {
      setStatus(await setSession(value));
      setValue("");
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="card">
      <div className="field">
        <div className="field-head">
          <span className="label">Trade site session (POESESSID)</span>
          <span className={status?.error ? "bad small" : "muted small"}>
            {!status
              ? "…"
              : !status.set
                ? "Not set"
                : status.account
                  ? `Signed in as ${status.account}`
                  : status.error ?? "Saved"}
          </span>
        </div>
        <form
          className="inline-form"
          onSubmit={(e) => {
            e.preventDefault();
            save();
          }}
        >
          <input
            type="password"
            placeholder={status?.set ? "Paste a new one to replace it" : "Paste your POESESSID"}
            value={value}
            onChange={(e) => setValue(e.target.value)}
            autoComplete="off"
          />
          <button className="btn primary" type="submit" disabled={!value.trim() || busy}>
            Save
          </button>
          {status?.set && (
            <button
              className="btn"
              type="button"
              onClick={() => clearSession().then(() => setStatus({ set: false, account: null, error: null }))}
            >
              Remove
            </button>
          )}
        </form>
        {error && <p className="bad small">{error}</p>}
        <p className="hint">
          Live searches and buying (Travel to hideout, whisper) act as you on the trade site, so they need its
          session cookie: sign in on pathofexile.com, open the browser's developer tools (F12) › Application ›
          Cookies › pathofexile.com, and copy the value of POESESSID. It works like a password for your account:
          PoE Tool keeps it in Windows Credential Manager and sends it only to pathofexile.com. Signing out on the
          site ends it.
        </p>
      </div>
    </section>
  );
}

function GroupCard({
  group,
  statuses,
  run,
}: {
  group: SearchGroup;
  statuses: Record<string, LiveStatus>;
  run: (p: Promise<unknown>) => void;
}) {
  const [link, setLink] = useState("");
  const [name, setName] = useState("");
  const [title, setTitle] = useState(group.name);

  useEffect(() => setTitle(group.name), [group.name]);

  return (
    <section className="card group">
      <div className="group-head">
        <input
          className="group-title"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          onBlur={() => title.trim() && title !== group.name && run(renameGroup(group.id, title))}
        />
        <button
          className="icon-btn"
          title="Remove this build and its searches"
          onClick={() => confirm(`Remove ${group.name} and its searches?`) && run(deleteGroup(group.id))}
        >
          ×
        </button>
      </div>

      {group.searches.length === 0 && <p className="muted small">No searches yet.</p>}
      {group.searches.map((s) => (
        <SearchRow key={s.id} search={s} status={statuses[s.id]} run={run} />
      ))}

      <form
        className="inline-form"
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
          Add
        </button>
      </form>
    </section>
  );
}

function SearchRow({
  search,
  status,
  run,
}: {
  search: { id: string; name: string; league: string; live: boolean };
  status: LiveStatus | undefined;
  run: (p: Promise<unknown>) => void;
}) {
  const [name, setName] = useState(search.name);
  const [opening, setOpening] = useState(false);
  useEffect(() => setName(search.name), [search.name]);

  async function open() {
    setOpening(true);
    try {
      openUrl(await searchUrl(search.id));
    } catch (e) {
      alert(String(e));
    } finally {
      setOpening(false);
    }
  }

  const state =
    !search.live || !status
      ? null
      : status.state === "live"
        ? { label: "Live", tone: "good" }
        : status.state === "connecting"
          ? { label: "Connecting…", tone: "muted" }
          : { label: status.message, tone: "bad" };

  return (
    <div className="search-row">
      <div className="search-main">
        <input
          className="search-name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          onBlur={() => name.trim() && name !== search.name && run(updateSearch(search.id, { name }))}
        />
        <small className="muted">
          {search.league}
          {state && <span className={`live-state ${state.tone}`}> · {state.label}</span>}
        </small>
      </div>
      <label className="switch small-switch" title="Watch live">
        <input type="checkbox" checked={search.live} onChange={(e) => run(updateSearch(search.id, { live: e.target.checked }))} />
        <span />
      </label>
      <button className="btn small-btn" onClick={open} disabled={opening} title="Open this search on the trade site">
        {opening ? "…" : "Trade ↗"}
      </button>
      <button className="icon-btn" title="Remove" onClick={() => run(deleteSearch(search.id))}>
        ×
      </button>
    </div>
  );
}

function LiveFeed({ hits }: { hits: LiveHit[] }) {
  const [done, setDone] = useState<Record<string, string>>({});

  async function act(hit: LiveHit, token: string, label: string) {
    const key = token;
    setDone((d) => ({ ...d, [key]: "…" }));
    try {
      await actOnListing(token, hit.league);
      setDone((d) => ({ ...d, [key]: label }));
    } catch (e) {
      setDone((d) => ({ ...d, [key]: String(e) }));
    }
  }

  return (
    <section className="card">
      <div className="label">Live</div>
      {hits.length === 0 && <p className="muted small">New items from searches marked Live show up here.</p>}
      {hits.map((h, i) => {
        const token = h.hideoutToken ?? h.whisperToken;
        const result = token ? done[token] : undefined;
        return (
          <div key={i} className="hit">
            <div className="hit-main">
              <div className="hit-item">{h.itemName ? `${h.itemName} ${h.typeLine}` : h.typeLine}</div>
              <small className="muted">
                {h.group} · {h.searchName} · {h.account} · {timeAgo(h.seen)} ago
              </small>
              {result && <small className={result.startsWith("Travel") || result.startsWith("Whisper") ? "muted" : "bad"}>{result}</small>}
            </div>
            <div className="hit-price">{h.amount != null ? `${h.amount} ${h.currency}` : "—"}</div>
            {h.hideoutToken ? (
              <button className="btn primary small-btn" onClick={() => act(h, h.hideoutToken!, "Travelling to hideout")}>
                Travel to hideout
              </button>
            ) : h.whisperToken ? (
              <button className="btn small-btn" onClick={() => act(h, h.whisperToken!, "Whisper sent")}>
                Whisper
              </button>
            ) : h.whisper ? (
              <button className="btn small-btn" onClick={() => navigator.clipboard.writeText(h.whisper!)}>
                Copy whisper
              </button>
            ) : null}
          </div>
        );
      })}
    </section>
  );
}
