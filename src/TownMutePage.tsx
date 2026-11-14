import { useEffect, useRef, useState } from "react";
import {
  getSettings,
  getStatus,
  onStatus,
  setSettings,
  type Settings,
  type Status,
  type TownMuteSettings,
} from "./townMute";

const KIND_LABEL = {
  town: "Town",
  hideout: "Hideout",
  other: "Zone",
  unknown: "Unknown",
} as const;

export function TownMutePage() {
  const [settings, setLocal] = useState<Settings | null>(null);
  const [status, setStatus] = useState<Status | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const saveTimer = useRef<number | undefined>(undefined);

  useEffect(() => {
    getSettings().then(setLocal, (e) => setSaveError(`Couldn't load settings: ${e}`));
    getStatus().then(setStatus);
    const unlisten = onStatus(setStatus);
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  if (!settings) return saveError ? <p className="bad">{saveError}</p> : null;
  const tm = settings.townMute;

  // Save right away for toggles; debounce typing and sliding.
  function update(patch: Partial<TownMuteSettings>, debounce = false) {
    const next = { ...settings!, townMute: { ...tm, ...patch } };
    setLocal(next);
    window.clearTimeout(saveTimer.current);
    const save = () =>
      setSettings(next).then(
        () => setSaveError(null),
        (e) => setSaveError(String(e)),
      );
    if (debounce) saveTimer.current = window.setTimeout(save, 300);
    else save();
  }

  const inTown =
    status?.areaKind === "town" || (status?.areaKind === "hideout" && tm.includeHideouts);
  const state = !tm.enabled
    ? { label: "Off", tone: "muted" }
    : status?.lowered && !inTown
      ? // The volume is only raised while the game is in front (it changes its own
        // volume when alt-tabbed).
        { label: "Back to normal when you return to the game", tone: "active" }
      : status?.lowered
        ? { label: tm.townVolume === 0 ? "Muted" : `Lowered to ${tm.townVolume}%`, tone: "active" }
        : { label: "Normal volume", tone: "idle" };

  return (
    <div className="page">
      <header className="page-head">
        <div>
          <h1>Town mute</h1>
          <p className="lede">
            Lowers Path of Exile's volume in the Windows volume mixer while you're in town, so
            Goblin Troupes and other noisy pets stay quiet. Reads <code>Client.txt</code> only and
            never touches the game.
          </p>
        </div>
        <label className="switch" title="Turn town mute on or off">
          <input
            type="checkbox"
            checked={tm.enabled}
            onChange={(e) => update({ enabled: e.target.checked })}
          />
          <span />
        </label>
      </header>

      <section className={`state state-${state.tone}`}>
        <div className="state-main">
          <span className="state-dot" />
          <span className="state-label">{state.label}</span>
        </div>
        <dl className="facts">
          <div>
            <dt>Area</dt>
            <dd>
              {status?.areaName ?? status?.areaId ?? "Waiting for a zone change"}
              {status && status.areaKind !== "unknown" && (
                <span className={`tag tag-${status.areaKind}`}>{KIND_LABEL[status.areaKind]}</span>
              )}
            </dd>
          </div>
          <div>
            <dt>Game</dt>
            <dd>{status?.gameRunning ? "Running" : "Not running"}</dd>
          </div>
          <div>
            <dt>Client.txt</dt>
            <dd className={status?.logError ? "bad" : ""}>
              {status?.logError ?? status?.logPath ?? "Looking…"}
            </dd>
          </div>
          {status?.audioError && (
            <div>
              <dt>Audio</dt>
              <dd className="bad">{status.audioError}</dd>
            </div>
          )}
        </dl>
      </section>

      <section className="card">
        <div className="field">
          <div className="field-head">
            <label htmlFor="vol">Volume in town</label>
            <output>{tm.townVolume === 0 ? "Muted" : `${tm.townVolume}%`}</output>
          </div>
          <input
            id="vol"
            type="range"
            min={0}
            max={100}
            step={5}
            value={tm.townVolume}
            onChange={(e) => update({ townVolume: Number(e.target.value) }, true)}
          />
          <p className="hint">Percent of the game's normal volume. It goes back as you leave town.</p>
        </div>

        <label className="check">
          <input
            type="checkbox"
            checked={tm.includeHideouts}
            onChange={(e) => update({ includeHideouts: e.target.checked })}
          />
          <span>
            Hideouts too
            <small>For when others visit your hideout with their pets.</small>
          </span>
        </label>
      </section>

      <details className="card advanced">
        <summary>Advanced</summary>
        <div className="field">
          <label htmlFor="log">Client.txt path</label>
          <input
            id="log"
            type="text"
            placeholder="Found automatically from the running game"
            value={tm.clientLogPath ?? ""}
            onChange={(e) => update({ clientLogPath: e.target.value || null }, true)}
          />
        </div>
        <div className="field">
          <label htmlFor="extra">More areas to treat as towns</label>
          <textarea
            id="extra"
            rows={3}
            placeholder={"One per line, area name or id, e.g.\nThe Menagerie"}
            value={tm.extraAreas.join("\n")}
            onChange={(e) => update({ extraAreas: e.target.value.split("\n") }, true)}
          />
        </div>
      </details>

      {saveError && <p className="bad">Couldn't save settings: {saveError}</p>}
    </div>
  );
}
