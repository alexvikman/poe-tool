import { invoke } from "@tauri-apps/api/core";

/**
 * Calls a command that only reads, trying again for a few seconds if it fails: right
 * after start the page can be faster than the app behind it.
 */
export async function read<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const deadline = Date.now() + 5000;
  for (;;) {
    try {
      return await invoke<T>(cmd, args);
    } catch (e) {
      if (Date.now() > deadline) throw e;
      await new Promise((r) => setTimeout(r, 150));
    }
  }
}
