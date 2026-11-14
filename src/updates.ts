import { useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

const CHECK_EVERY_MS = 60 * 60 * 1000;
/** Opening the window checks again when the last check is older than this. */
const RECHECK_ON_FOCUS_MS = 5 * 60 * 1000;

export interface UpdateState {
  checking: boolean;
  /** A newer build, when there is one. */
  update: Update | null;
  lastChecked: Date | null;
  /** "Downloading… 40%", "Installing…" while it installs. */
  progress: string | null;
  error: string | null;
}

let state: UpdateState = { checking: false, update: null, lastChecked: null, progress: null, error: null };
const listeners = new Set<() => void>();

function set(patch: Partial<UpdateState>) {
  state = { ...state, ...patch };
  listeners.forEach((l) => l());
}

/** Looks for a newer build now. */
export async function checkForUpdate(): Promise<void> {
  if (state.checking || state.progress) return;
  set({ checking: true, error: null });
  try {
    // Always the newest answer, never one kept along the way.
    const update = await check({ headers: { "Cache-Control": "no-cache", Pragma: "no-cache" } });
    set({ update, lastChecked: new Date() });
  } catch (e) {
    set({ error: String(e), lastChecked: new Date() });
  } finally {
    set({ checking: false });
  }
}

/** Downloads and installs the newer build, then restarts the app. */
export async function installUpdate(): Promise<void> {
  const update = state.update;
  if (!update || state.progress) return;
  set({ error: null, progress: "Downloading…" });
  try {
    // Installing closes the app without its usual exit; put the game's volume back.
    await invoke("prepare_update");
    let total = 0;
    let done = 0;
    await update.downloadAndInstall((event) => {
      if (event.event === "Started") total = event.data.contentLength ?? 0;
      if (event.event === "Progress") {
        done += event.data.chunkLength;
        if (total > 0) set({ progress: `Downloading… ${Math.round((done / total) * 100)}%` });
      }
      if (event.event === "Finished") set({ progress: "Installing…" });
    });
    await relaunch();
  } catch (e) {
    set({ progress: null, error: String(e) });
  }
}

let started = false;

/** Checks at start, every hour after, and when the window is opened again. Call once. */
export function startUpdateChecks() {
  if (started) return;
  started = true;
  checkForUpdate();
  window.setInterval(checkForUpdate, CHECK_EVERY_MS);
  window.addEventListener("focus", () => {
    const last = state.lastChecked?.getTime() ?? 0;
    if (Date.now() - last > RECHECK_ON_FOCUS_MS) checkForUpdate();
  });
}

export function useUpdates(): UpdateState {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => state,
  );
}
