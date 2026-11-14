import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

const CHECK_EVERY_MS = 60 * 60 * 1000;

/** Checks for a newer build at start and every hour, and installs it on request. */
export function UpdateNotice() {
  const [update, setUpdate] = useState<Update | null>(null);
  const [progress, setProgress] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const look = () =>
      check().then(
        (u) => setUpdate(u),
        () => {
          // Offline, or a build without updates: try again later.
        },
      );
    look();
    const timer = window.setInterval(look, CHECK_EVERY_MS);
    return () => window.clearInterval(timer);
  }, []);

  if (!update) return null;

  async function install() {
    if (!update) return;
    setError(null);
    setProgress("Downloading…");
    try {
      // Installing closes the app without its usual exit; put the game's volume back.
      await invoke("prepare_update");
      let total = 0;
      let done = 0;
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") total = event.data.contentLength ?? 0;
        if (event.event === "Progress") {
          done += event.data.chunkLength;
          if (total > 0) setProgress(`Downloading… ${Math.round((done / total) * 100)}%`);
        }
        if (event.event === "Finished") setProgress("Installing…");
      });
      await relaunch();
    } catch (e) {
      setProgress(null);
      setError(String(e));
    }
  }

  return (
    <div className="update">
      <div className="update-title">Update available</div>
      <small>v{update.version}</small>
      <button className="btn primary small-btn" onClick={install} disabled={progress != null}>
        {progress ?? "Install and restart"}
      </button>
      {error && <small className="bad">{error}</small>}
    </div>
  );
}
