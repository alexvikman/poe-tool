import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getLeagues, priceCheck, type League, type PriceCheck } from "./price";
import { getSettings, setSettings } from "./townMute";
import { PriceResult } from "./PriceResult";

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
      if (payload.text) run(payload.text);
      else {
        request.current++;
        setCheck(null);
        setLoading(false);
        setError(payload.error);
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
    if (lastText.current) run(lastText.current);
  }

  const current = check?.league ?? "";
  return (
    <div className="overlay">
      <div className="overlay-bar" data-tauri-drag-region>
        <span data-tauri-drag-region>Price check</span>
        <div className="overlay-bar-right">
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
        {loading && <p className="muted">Checking prices…</p>}
        {error && <p className="bad">{error}</p>}
        {check && <PriceResult check={check} />}
      </div>
    </div>
  );
}
