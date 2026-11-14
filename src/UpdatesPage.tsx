import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { checkForUpdate, installUpdate, useUpdates } from "./updates";

const RELEASE_PAGE = "https://github.com/alexvikman/poe-tool/releases/tag/nightly";

function ago(date: Date): string {
  const s = Math.round((Date.now() - date.getTime()) / 1000);
  if (s < 60) return "just now";
  if (s < 3600) return `${Math.round(s / 60)} min ago`;
  return `${Math.round(s / 3600)} h ago`;
}

export function UpdatesPage() {
  const { checking, update, lastChecked, progress, error } = useUpdates();
  const [version, setVersion] = useState<string | null>(null);
  const [, tick] = useState(0);

  useEffect(() => {
    getVersion().then(setVersion, () => setVersion(null));
    // Keep "checked … ago" current.
    const t = window.setInterval(() => tick((n) => n + 1), 30_000);
    return () => window.clearInterval(t);
  }, []);

  return (
    <div className="page">
      <header className="page-head">
        <div>
          <h1>Updates</h1>
          <p className="lede">
            PoE Tool looks for a newer build when it starts, every hour, and when you open its window. Installing one restarts the
            app; town mute puts the game's volume back first.
          </p>
        </div>
      </header>

      <section className={`state ${update ? "state-active" : "state-idle"}`}>
        <div className="state-main">
          <span className="state-dot" />
          <span className="state-label">
            {checking
              ? "Checking…"
              : update
                ? `Version ${update.version} is available`
                : error
                  ? "Couldn't check for updates"
                  : lastChecked
                    ? "Up to date"
                    : "Not checked yet"}
          </span>
        </div>
        <dl className="facts">
          <div>
            <dt>This version</dt>
            <dd>{version ? `v${version}` : "…"}</dd>
          </div>
          {update && (
            <div>
              <dt>New version</dt>
              <dd>
                v{update.version}
                {update.date && <span className="muted"> · {new Date(update.date).toLocaleString()}</span>}
              </dd>
            </div>
          )}
          {update?.body && (
            <div>
              <dt>What's new</dt>
              <dd>{update.body}</dd>
            </div>
          )}
          <div>
            <dt>Last checked</dt>
            <dd>{lastChecked ? ago(lastChecked) : "never"}</dd>
          </div>
          {error && (
            <div>
              <dt>Problem</dt>
              <dd className="bad">{error}</dd>
            </div>
          )}
        </dl>
      </section>

      <section className="card">
        <div className="update-actions">
          {update ? (
            <button className="btn primary" onClick={installUpdate} disabled={progress != null}>
              {progress ?? `Install v${update.version} and restart`}
            </button>
          ) : (
            <button className="btn primary" onClick={checkForUpdate} disabled={checking}>
              {checking ? "Checking…" : "Check for updates"}
            </button>
          )}
          <button className="btn" onClick={() => openUrl(RELEASE_PAGE)}>
            Release page ↗
          </button>
        </div>
      </section>
    </div>
  );
}
