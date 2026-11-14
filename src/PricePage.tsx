import { useEffect, useRef, useState } from "react";
import { getSettings, setSettings, type Settings } from "./townMute";
import { getLeagues, priceCheck, type League, type PriceCheck } from "./price";
import { PriceResult } from "./PriceResult";
import { GemSearch } from "./GemSearch";

export function PricePage() {
  const [settings, setLocal] = useState<Settings | null>(null);
  const [leagues, setLeagues] = useState<League[]>([]);
  const [text, setText] = useState("");
  const [check, setCheck] = useState<PriceCheck | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const saveTimer = useRef<number | undefined>(undefined);

  useEffect(() => {
    getSettings().then(setLocal);
    getLeagues().then(setLeagues, () => setLeagues([]));
  }, []);

  if (!settings) return null;
  const pc = settings.priceCheck;

  function update(patch: Partial<Settings["priceCheck"]>, debounce = false) {
    const next = { ...settings!, priceCheck: { ...pc, ...patch } };
    setLocal(next);
    window.clearTimeout(saveTimer.current);
    const save = () => setSettings(next).catch((e) => setError(String(e)));
    if (debounce) saveTimer.current = window.setTimeout(save, 400);
    else save();
  }

  async function refreshLeagues() {
    setRefreshing(true);
    try {
      setLeagues(await getLeagues(true));
    } catch (e) {
      setError(String(e));
    } finally {
      setRefreshing(false);
    }
  }

  const knownLeague = pc.league == null || leagues.some((l) => l.id === pc.league);
  const privateLeague = knownLeague ? "" : pc.league ?? "";

  async function run() {
    setLoading(true);
    setError(null);
    setCheck(null);
    try {
      setCheck(await priceCheck(text));
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="page">
      <header className="page-head">
        <div>
          <h1>Price check</h1>
          <p className="lede">
            Hover over an item in the game and press <kbd>{pc.hotkey || "Ctrl+D"}</kbd>. The tool
            copies the item (one Ctrl+Alt+C, the game's advanced copy) and shows prices from poe.ninja and the trade site beside
            your cursor. The hotkey only works while the game is in front.
          </p>
        </div>
        <label className="switch" title="Turn price check on or off">
          <input
            type="checkbox"
            checked={pc.enabled}
            onChange={(e) => update({ enabled: e.target.checked })}
          />
          <span />
        </label>
      </header>

      <section className="card">
        <div className="field">
          <div className="field-head">
            <span className="label">League</span>
            <button className="btn small-btn" onClick={refreshLeagues} disabled={refreshing}>
              {refreshing ? "Refreshing…" : "Refresh"}
            </button>
          </div>
          <div className="radios">
            <label className="radio">
              <input type="radio" name="league" checked={pc.league == null} onChange={() => update({ league: null })} />
              <span>
                Current league{leagues[0] ? ` (${leagues[0].id})` : ""}
                <small>Switches to a new league by itself when one starts.</small>
              </span>
            </label>
            {leagues.map((l) => (
              <label key={l.id} className="radio">
                <input
                  type="radio"
                  name="league"
                  checked={pc.league === l.id}
                  onChange={() => update({ league: l.id })}
                />
                <span>{l.id}</span>
              </label>
            ))}
          </div>
          <div className="private-league">
            <label htmlFor="private-league">or private league</label>
            <input
              id="private-league"
              type="text"
              placeholder="My League (PL12345)"
              value={privateLeague}
              onChange={(e) => update({ league: e.target.value.trim() ? e.target.value : null }, true)}
            />
          </div>
          <p className="hint">
            Leagues come from the trade site, so new ones show up without updating the tool.
          </p>
        </div>
      </section>

      <section className="card">
        <p className="hint">
          The hotkey ({pc.hotkey || "none"}) is set under Hotkeys. Path of Exile must run in Windowed or
          Windowed Fullscreen for the overlay to show over it.
        </p>
      </section>

      <section className="card">
        <div className="field">
          <span className="label">Labyrinth: transfigured gems</span>
          <p className="hint">
            The game can't copy the gems in the Divine Font. Type the gem's name to see what each
            transfigured version is worth. The overlay offers this too when nothing could be copied.
          </p>
        </div>
        <GemSearch />
      </section>

      <section className="card">
        <div className="field">
          <label htmlFor="paste">Try it: paste an item</label>
          <textarea
            id="paste"
            rows={5}
            placeholder={"Item Class: Boots\nRarity: Rare\n…"}
            value={text}
            onChange={(e) => setText(e.target.value)}
          />
        </div>
        <div>
          <button className="btn primary" onClick={run} disabled={!text.trim() || loading}>
            {loading ? "Checking…" : "Check price"}
          </button>
        </div>
        {error && <p className="bad">{error}</p>}
        {check && <PriceResult check={check} />}
      </section>
    </div>
  );
}
