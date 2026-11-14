import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { priceCheck, type PriceCheck } from "./price";
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

  useEffect(() => {
    const win = getCurrentWindow();
    let request = 0;
    const unlisten = win.listen<PriceRequest>("price-request", async ({ payload }) => {
      const id = ++request;
      setCheck(null);
      setError(payload.error);
      if (!payload.text) return;
      setLoading(true);
      try {
        const result = await priceCheck(payload.text);
        if (id === request) setCheck(result);
      } catch (e) {
        if (id === request) setError(String(e));
      } finally {
        if (id === request) setLoading(false);
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

  return (
    <div className="overlay">
      <div className="overlay-bar" data-tauri-drag-region>
        <span data-tauri-drag-region>Price check</span>
        <button className="icon-btn" onClick={() => getCurrentWindow().hide()} title="Close (Esc)">
          ×
        </button>
      </div>
      <div className="overlay-body">
        {loading && <p className="muted">Checking prices…</p>}
        {error && <p className="bad">{error}</p>}
        {check && <PriceResult check={check} />}
      </div>
    </div>
  );
}
