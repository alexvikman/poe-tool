import { useEffect, useRef, useState } from "react";
import { getSettings, setSettings, type Settings } from "./townMute";
import { getLeagues, priceCheck, type League, type PriceCheck } from "./price";
import { PriceResult } from "./PriceResult";

export function PricePage() {
  const [settings, setLocal] = useState<Settings | null>(null);
  const [leagues, setLeagues] = useState<League[]>([]);
  const [text, setText] = useState("");
  const [check, setCheck] = useState<PriceCheck | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
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
            copies the item (one Ctrl+C) and shows prices from poe.ninja and the trade site beside
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
        <div className="row-2">
          <div className="field">
            <label htmlFor="league">League</label>
            <select
              id="league"
              value={pc.league ?? ""}
              onChange={(e) => update({ league: e.target.value || null })}
            >
              <option value="">Current league{leagues[0] ? ` (${leagues[0].id})` : ""}</option>
              {leagues.map((l) => (
                <option key={l.id} value={l.id}>
                  {l.id}
                </option>
              ))}
            </select>
          </div>
          <div className="field">
            <label htmlFor="hotkey">Hotkey</label>
            <input
              id="hotkey"
              type="text"
              value={pc.hotkey}
              placeholder="Ctrl+D"
              onChange={(e) => update({ hotkey: e.target.value }, true)}
            />
          </div>
        </div>
        <p className="hint">
          Path of Exile must run in Windowed or Windowed Fullscreen for the overlay to show over it.
        </p>
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
