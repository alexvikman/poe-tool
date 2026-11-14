import { installUpdate, useUpdates } from "./updates";

/** A small notice in the menu when a newer build is out. */
export function UpdateNotice({ onOpen }: { onOpen: () => void }) {
  const { update, progress, error } = useUpdates();
  if (!update) return null;

  return (
    <div className="update">
      <button className="link-btn update-title" onClick={onOpen}>
        Update available
      </button>
      <small>v{update.version}</small>
      <button className="btn primary small-btn" onClick={installUpdate} disabled={progress != null}>
        {progress ?? "Install and restart"}
      </button>
      {error && <small className="bad">{error}</small>}
    </div>
  );
}
