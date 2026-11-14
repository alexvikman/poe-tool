import { useState } from "react";

const MODIFIER_CODES = new Set([
  "ControlLeft",
  "ControlRight",
  "AltLeft",
  "AltRight",
  "ShiftLeft",
  "ShiftRight",
  "MetaLeft",
  "MetaRight",
]);

/** A key from a KeyboardEvent.code, as the hotkey parser reads it: KeyD → D, Digit1 → 1. */
function keyName(code: string): string {
  if (code.startsWith("Key")) return code.slice(3);
  if (code.startsWith("Digit")) return code.slice(5);
  return code;
}

/** Records a hotkey: click it, then press the keys. Esc cancels, Backspace clears. */
export function HotkeyInput({
  value,
  onChange,
}: {
  value: string;
  onChange: (hotkey: string) => void;
}) {
  const [recording, setRecording] = useState(false);

  function onKeyDown(e: React.KeyboardEvent) {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();
    if (MODIFIER_CODES.has(e.code)) return;
    if (e.code === "Escape") {
      setRecording(false);
      return;
    }
    if (e.code === "Backspace" || e.code === "Delete") {
      onChange("");
      setRecording(false);
      return;
    }
    const parts = [];
    if (e.ctrlKey) parts.push("Ctrl");
    if (e.altKey) parts.push("Alt");
    if (e.shiftKey) parts.push("Shift");
    parts.push(keyName(e.code));
    onChange(parts.join("+"));
    setRecording(false);
  }

  return (
    <button
      type="button"
      className={`hotkey-input ${recording ? "recording" : ""}`}
      onClick={() => setRecording(true)}
      onBlur={() => setRecording(false)}
      onKeyDown={onKeyDown}
      title="Click, then press the keys. Esc cancels, Backspace clears."
    >
      {recording ? "Press keys…" : value ? <kbd>{value}</kbd> : <span className="muted">None</span>}
    </button>
  );
}
