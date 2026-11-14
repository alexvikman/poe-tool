import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getLeagues, priceCheck, type League, type PriceCheck } from "./price";
import { getSettings, setSettings } from "./townMute";
import { PriceResult } from "./PriceResult";
import { GemSearch } from "./GemSearch";

interface PriceRequest {
  text: string | null;
  error: string | null;
}

/** The window the hotkey opens beside the cursor. */
export function PriceOverlay() {
  const [check, setCheck] = useState<PriceCheck | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [leagues, setLeagues] = useState<League[]>([]);
  /** Typing a gem name, for the Divine Font, where the game can't copy items. */
  const [gems, setGems] = useState(false);
  const [gemsKey, setGemsKey] = useState(0);
  /** The league chosen in settings; null for the current league. */
  const [chosenLeague, setChosenLeague] = useState<string | null>(null);
  const lastText = useRef<string | null>(null);
  const request = useRef(0);

  async function run(text: string) {
    const id = ++request.current;
    setCheck(null);
    setError(null);
    setLoading(true);
    try {
      const result = await priceCheck(text);
      if (id === request.current) setCheck(result);
    } catch (e) {
      if (id === request.current) setError(String(e));
    } finally {
      if (id === request.current) setLoading(false);
    }
  }

  useEffect(() => {
    const win = getCurrentWindow();
    getLeagues().then(setLeagues, () => setLeagues([]));
    const unlisten = win.listen<PriceRequest>("price-request", ({ payload }) => {
      lastText.current = payload.text;
      setGemsKey((k) => k + 1);
      getSettings().then((s) => setChosenLeague(s.priceCheck.league));
      if (payload.text) {
        setGems(false);
        run(payload.text);
      } else {
        // Nothing copied: most likely the Labyrinth's Divine Font. Offer the gem search.
        request.current++;
        setCheck(null);
        setLoading(false);
        setError(payload.error);
        setGems(true);
      }
    });
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") win.hide();
    };
    window.addEventListener("keydown", onKey);
    return () => {
      unlisten.then((f) => f());
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  /** Saves the league for every price check from now on and checks this item again. */
  async function changeLeague(league: string) {
    const settings = await getSettings();
    await setSettings({ ...settings, priceCheck: { ...settings.priceCheck, league } });
    setChosenLeague(league);
    if (gems) setGemsKey((k) => k + 1);
    else if (lastText.current) run(lastText.current);
  }

  const current = check?.league ?? chosenLeague ?? leagues[0]?.id ?? "";
  return (
    <div className="overlay">
      <div className="overlay-bar" data-tauri-drag-region>
        <span data-tauri-drag-region>{gems ? "Lab gems" : "Price check"}</span>
        <div className="overlay-bar-right">
          <button
            className={`chip ${gems ? "on" : ""}`}
            onClick={() => setGems(!gems)}
            title="Prices of transfigured gems, for the Divine Font"
          >
            Lab gems
          </button>
          {leagues.length > 0 && (
            <select
              className="league-select"
              value={current}
              onChange={(e) => changeLeague(e.target.value)}
              title="League for prices (saved for later checks)"
            >
              {!leagues.some((l) => l.id === current) && current && <option value={current}>{current}</option>}
              {leagues.map((l) => (
                <option key={l.id} value={l.id}>
                  {l.id}
                </option>
              ))}
            </select>
          )}
          <button className="icon-btn" onClick={() => getCurrentWindow().hide()} title="Close (Esc)">
            ×
          </button>
        </div>
      </div>
      <div className="overlay-body">
        {gems ? (
          <>
            {error && (
              <p className="muted small">
                Nothing was copied. In the Labyrinth's Divine Font the game can't copy the gems:
                type the gem's name to see what each transfigured version is worth.
              </p>
            )}
            <GemSearch key={gemsKey} autoFocus />
          </>
        ) : (
          <>
            {loading && <p className="muted">Checking prices…</p>}
            {error && <p className="bad">{error}</p>}
            {check && <PriceResult check={check} />}
          </>
        )}
      </div>
    </div>
  );
}
