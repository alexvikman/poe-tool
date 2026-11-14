import { useEffect, useRef, useState } from "react";
import { getSettings, setSettings, type ChatCommand, type Settings } from "./townMute";
import { HotkeyInput } from "./HotkeyInput";

const PRESETS = ["/hideout", "/menagerie", "/kingsmarch", "/delve", "/heist", "/exit", "/remaining"];

export function HotkeysPage() {
  const [settings, setLocal] = useState<Settings | null>(null);
  const [error, setError] = useState<string | null>(null);
  const saveTimer = useRef<number | undefined>(undefined);

  useEffect(() => {
    getSettings().then(setLocal, (e) => setError(`Couldn't load settings: ${e}`));
  }, []);

  if (!settings) return error ? <p className="bad">{error}</p> : null;

  function save(next: Settings, debounce = false) {
    setLocal(next);
    window.clearTimeout(saveTimer.current);
    const run = () =>
      setSettings(next).then(
        () => setError(null),
        (e) => setError(String(e)),
      );
    if (debounce) saveTimer.current = window.setTimeout(run, 400);
    else run();
  }

  function updateCommand(i: number, patch: Partial<ChatCommand>, debounce = false) {
    const chatCommands = settings!.chatCommands.map((c, j) => (j === i ? { ...c, ...patch } : c));
    save({ ...settings!, chatCommands }, debounce);
  }

  function addCommand(text = "") {
    const chatCommands = [...settings!.chatCommands, { enabled: true, hotkey: "", text, send: true }];
    save({ ...settings!, chatCommands });
  }

  function removeCommand(i: number) {
    save({ ...settings!, chatCommands: settings!.chatCommands.filter((_, j) => j !== i) });
  }

  return (
    <div className="page">
      <header className="page-head">
        <div>
          <h1>Hotkeys</h1>
          <p className="lede">
            Hotkeys only work while Path of Exile is in front, so other apps keep their keys. One key
            press does one thing in the game.
          </p>
        </div>
      </header>

      <section className="card">
        <div className="hotkey-row">
          <div>
            <div className="label">Price check</div>
            <small className="muted">Copies the item under the cursor and shows its price.</small>
          </div>
          <HotkeyInput
            value={settings.priceCheck.hotkey}
            onChange={(hotkey) => save({ ...settings, priceCheck: { ...settings.priceCheck, hotkey } })}
          />
        </div>
      </section>

      <section className="card">
        <div>
          <div className="label">Chat commands</div>
          <p className="hint">
            Opens the chat, types the command and sends it. Untick “Send” to leave it in the chat box
            to finish, such as a whisper.
          </p>
        </div>
        <div className="commands">
          {settings.chatCommands.map((c, i) => (
            <div key={i} className={`command ${c.enabled ? "" : "off"}`}>
              <input
                type="checkbox"
                checked={c.enabled}
                title="On"
                onChange={(e) => updateCommand(i, { enabled: e.target.checked })}
              />
              <HotkeyInput value={c.hotkey} onChange={(hotkey) => updateCommand(i, { hotkey })} />
              <input
                type="text"
                className="command-text"
                placeholder="/hideout"
                value={c.text}
                onChange={(e) => updateCommand(i, { text: e.target.value }, true)}
              />
              <label className="command-send" title="Press Enter after typing">
                <input
                  type="checkbox"
                  checked={c.send}
                  onChange={(e) => updateCommand(i, { send: e.target.checked })}
                />
                Send
              </label>
              <button className="icon-btn" onClick={() => removeCommand(i)} title="Remove">
                ×
              </button>
            </div>
          ))}
        </div>
        <div className="command-add">
          <button className="btn" onClick={() => addCommand()}>
            Add command
          </button>
          <div className="chips">
            {PRESETS.filter((p) => !settings.chatCommands.some((c) => c.text.trim() === p)).map((p) => (
              <button key={p} className="chip" onClick={() => addCommand(p)}>
                + {p}
              </button>
            ))}
          </div>
        </div>
      </section>

      {error && <p className="bad">Not saved: {error}</p>}
    </div>
  );
}
