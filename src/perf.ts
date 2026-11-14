// How long things take on the page, written to the app's performance log (perf.log)
// next to the backend's own timings.

import { invoke } from "@tauri-apps/api/core";

export function perfNote(line: string) {
  invoke("perf_note", { line }).catch(() => {});
}

/** Times a promise: "label ms=… extra". */
export async function timed<T>(label: string, work: Promise<T>, extra?: (value: T) => string): Promise<T> {
  const started = performance.now();
  const value = await work;
  perfNote(`${label} ms=${(performance.now() - started).toFixed(1)}${extra ? ` ${extra(value)}` : ""}`);
  return value;
}

/** Collects many short timings (frames drawn) and writes their count, average and
 *  worst once they've stopped for a second. */
export function batchTimer(label: string) {
  let times: number[] = [];
  let flush: ReturnType<typeof setTimeout> | undefined;
  return (ms: number) => {
    times.push(ms);
    clearTimeout(flush);
    flush = setTimeout(() => {
      const avg = times.reduce((a, b) => a + b, 0) / times.length;
      perfNote(`${label} n=${times.length} avg=${avg.toFixed(1)} max=${Math.max(...times).toFixed(1)}`);
      times = [];
    }, 1000);
  };
}
