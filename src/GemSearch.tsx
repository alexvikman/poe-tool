import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { formatChaos } from "./price";

interface GemVariantPrice {
  variant: string;
  chaos: number;
  listingCount: number | null;
}

interface GemOption {
  name: string;
  transfigured: boolean;
  prices: GemVariantPrice[];
}

interface GemFamily {
  base: string;
  gems: GemOption[];
}

interface GemSearchResult {
  league: string;
  divineChaos: number;
  families: GemFamily[];
}

const gemSearch = (query: string) => invoke<GemSearchResult>("gem_search", { query });

/** "1/20" → "lvl 1 · 20%" */
function variantLabel(v: string): string {
  const [level, quality] = v.replace(/c$/, "").split("/");
  return quality ? `${level}/${quality}%` : `lvl ${level}`;
}

/**
 * Prices of the gems the Divine Font can make from a base gem. The game doesn't let
 * you copy the choices in the Labyrinth, so you type the gem's name instead.
 */
export function GemSearch({ autoFocus = false }: { autoFocus?: boolean }) {
  const [query, setQuery] = useState("");
  const [result, setResult] = useState<GemSearchResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const timer = useRef<number | undefined>(undefined);
  const request = useRef(0);

  useEffect(() => () => window.clearTimeout(timer.current), []);

  function change(q: string) {
    setQuery(q);
    window.clearTimeout(timer.current);
    if (q.trim().length < 2) {
      setResult(null);
      return;
    }
    timer.current = window.setTimeout(async () => {
      const id = ++request.current;
      try {
        const r = await gemSearch(q);
        if (id === request.current) {
          setResult(r);
          setError(null);
        }
      } catch (e) {
        if (id === request.current) setError(String(e));
      }
    }, 200);
  }

  return (
    <div className="gem-search">
      <input
        type="text"
        className="gem-input"
        placeholder="Gem name, e.g. double strike"
        value={query}
        autoFocus={autoFocus}
        onChange={(e) => change(e.target.value)}
      />
      {error && <p className="bad small">{error}</p>}
      {result && result.families.length === 0 && (
        <p className="muted small">No transfigured gems match “{query}”.</p>
      )}
      {result?.families.map((f) => (
        <section key={f.base} className="gem-family">
          <div className="section-label">
            {f.base} · poe.ninja · {result.league}
          </div>
          {f.gems.map((g, i) => (
            <div key={g.name} className={`gem-row ${g.transfigured ? "" : "base"} ${i === 0 && g.transfigured ? "best" : ""}`}>
              <div className="gem-name">
                {g.name}
                {!g.transfigured && <small>base gem</small>}
              </div>
              <div className="gem-prices">
                {g.prices.length === 0 && <span className="muted">no price</span>}
                {g.prices.map((p) => (
                  <span key={p.variant} className="gem-price" title={p.listingCount != null ? `${p.listingCount} listed` : undefined}>
                    <small>{variantLabel(p.variant)}</small>
                    {formatChaos(p.chaos, result.divineChaos)}
                  </span>
                ))}
              </div>
            </div>
          ))}
        </section>
      ))}
    </div>
  );
}
