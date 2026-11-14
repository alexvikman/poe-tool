// The passive tree, drawn as the game draws it from GGG's own pictures: the build's
// allocated nodes lit, a node's text on hover, zoom with the wheel and drag to move.

import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { treeLayout, type GearItem, type TreeLayout, type TreeNode, type TreeView } from "./builds";
import { ItemTooltip } from "./ItemView";

const MIN_SCALE = 0.02;
const MAX_SCALE = 1.2;

interface Camera {
  /** The tree point in the middle of the view. */
  x: number;
  y: number;
  /** Screen pixels per tree unit. */
  s: number;
}

const images = new Map<string, HTMLImageElement>();
/** A picture, loaded once; `onLoad` runs when it arrives. */
function image(url: string, onLoad: () => void): HTMLImageElement | null {
  let img = images.get(url);
  if (!img) {
    img = new Image();
    img.src = url;
    images.set(url, img);
  }
  if (img.complete && img.naturalWidth > 0) return img;
  img.addEventListener("load", onLoad, { once: true });
  return null;
}

export function PassiveTree({
  tree,
  className,
  ascendancy,
}: {
  tree: TreeView;
  className: string;
  ascendancy: string | null;
}) {
  const [layout, setLayout] = useState<TreeLayout | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [hover, setHover] = useState<{ node: TreeNode; x: number; y: number } | null>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const box = useRef<HTMLDivElement>(null);
  const camera = useRef<Camera | null>(null);
  const frame = useRef(0);

  useEffect(() => {
    treeLayout().then(setLayout, (e) => setError(String(e)));
  }, []);

  const allocated = useMemo(() => new Set(tree.allocated), [tree]);

  // What's drawn: the main tree, and only the ascendancies the build uses.
  const shown = useMemo(() => {
    if (!layout) return null;
    const ascendancies = new Set<string>(ascendancy ? [ascendancy] : []);
    for (const n of layout.nodes) if (n.ascendancy && allocated.has(n.id)) ascendancies.add(n.ascendancy);
    const nodes = layout.nodes.filter((n) => !n.ascendancy || ascendancies.has(n.ascendancy));
    const byId = new Map(nodes.map((n) => [n.id, n]));
    const lit = (n: TreeNode) => allocated.has(n.id) || (n.kind === "classStart" && n.class === className);
    const edges = layout.edges.filter((e) => byId.has(e.a) && byId.has(e.b));
    return { nodes, byId, lit, edges };
  }, [layout, allocated, ascendancy, className]);

  // The view starts on the allocated part of the main tree.
  function fit() {
    const el = canvas.current;
    if (!shown || !el) return;
    const picked = shown.nodes.filter((n) => !n.ascendancy && shown.lit(n));
    const pts = picked.length > 0 ? picked : shown.nodes;
    const xs = pts.map((n) => n.x);
    const ys = pts.map((n) => n.y);
    const [x0, x1, y0, y1] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
    const pad = 400;
    const s = Math.min(el.clientWidth / (x1 - x0 + pad * 2), el.clientHeight / (y1 - y0 + pad * 2));
    camera.current = { x: (x0 + x1) / 2, y: (y0 + y1) / 2, s: Math.max(MIN_SCALE, Math.min(MAX_SCALE, s)) };
    redraw();
  }

  useLayoutEffect(() => {
    fit();
  }, [shown]);

  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const resize = new ResizeObserver(() => redraw());
    resize.observe(el);
    return () => resize.disconnect();
  }, [shown]);

  function redraw() {
    cancelAnimationFrame(frame.current);
    frame.current = requestAnimationFrame(draw);
  }

  function draw() {
    const el = canvas.current;
    const cam = camera.current;
    if (!el || !cam || !shown || !layout) return;
    const dpr = window.devicePixelRatio || 1;
    const w = el.clientWidth;
    const h = el.clientHeight;
    if (el.width !== Math.round(w * dpr) || el.height !== Math.round(h * dpr)) {
      el.width = Math.round(w * dpr);
      el.height = Math.round(h * dpr);
    }
    const ctx = el.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.fillStyle = "#07080a";
    ctx.fillRect(0, 0, el.width, el.height);
    ctx.setTransform(dpr * cam.s, 0, 0, dpr * cam.s, dpr * (w / 2 - cam.x * cam.s), dpr * (h / 2 - cam.y * cam.s));

    const zoom = layout.zoom;
    // A picture from one of GGG's sheets, centred on (x, y) at its size on the tree.
    const sprite = (sheet: string, name: string | null | undefined, x: number, y: number, scale = 1, clip = false) => {
      if (!name) return false;
      const s = layout.sprites[sheet];
      const at = s?.coords[name];
      if (!s || !at) return false;
      const img = image(s.url, redraw);
      if (!img) return true;
      const [sx, sy, sw, sh] = at;
      const dw = (sw / zoom) * scale;
      const dh = (sh / zoom) * scale;
      if (clip) {
        ctx.save();
        ctx.beginPath();
        ctx.arc(x, y, Math.min(dw, dh) / 2, 0, Math.PI * 2);
        ctx.clip();
      }
      ctx.drawImage(img, sx, sy, sw, sh, x - dw / 2, y - dh / 2, dw, dh);
      if (clip) ctx.restore();
      return true;
    };

    // Backgrounds: the build's class start and its ascendancy.
    for (const n of shown.nodes) {
      if (n.kind === "classStart") {
        const mine = n.class === className;
        sprite("startNode", mine ? `center${n.class?.toLowerCase()}` : "PSStartNodeBackgroundInactive", n.x, n.y);
      } else if (n.kind === "ascendancyStart" && n.ascendancy) {
        sprite("ascendancy", `Classes${n.ascendancy}`, n.x, n.y);
      }
    }

    // Lines: lit where both ends are allocated.
    const minWidth = 1.2 / cam.s;
    for (const lit of [false, true]) {
      ctx.strokeStyle = lit ? "#d8b56a" : "#3b3a35";
      ctx.lineWidth = Math.max(lit ? 16 : 10, minWidth);
      ctx.beginPath();
      for (const e of shown.edges) {
        const a = shown.byId.get(e.a)!;
        const b = shown.byId.get(e.b)!;
        if ((shown.lit(a) && shown.lit(b)) !== lit) continue;
        if (e.arc) {
          const [cx, cy, r] = e.arc;
          const a1 = Math.atan2(a.y - cy, a.x - cx);
          const a2 = Math.atan2(b.y - cy, b.x - cx);
          let d = a2 - a1;
          while (d > Math.PI) d -= Math.PI * 2;
          while (d < -Math.PI) d += Math.PI * 2;
          ctx.moveTo(a.x, a.y);
          ctx.arc(cx, cy, r, a1, a1 + d, d < 0);
        } else {
          ctx.moveTo(a.x, a.y);
          ctx.lineTo(b.x, b.y);
        }
      }
      ctx.stroke();
    }

    // Nodes: the picture, then its frame.
    for (const n of shown.nodes) {
      const lit = shown.lit(n);
      const asc = !!n.ascendancy;
      switch (n.kind) {
        case "normal":
          sprite(lit ? "normalActive" : "normalInactive", n.icon, n.x, n.y, 1, true);
          if (asc) sprite("ascendancy", lit ? "AscendancyFrameSmallAllocated" : "AscendancyFrameSmallNormal", n.x, n.y);
          else sprite("frame", lit ? "PSSkillFrameActive" : "PSSkillFrame", n.x, n.y);
          break;
        case "notable":
          sprite(lit ? "notableActive" : "notableInactive", n.icon, n.x, n.y, 1, true);
          if (asc) sprite("ascendancy", lit ? "AscendancyFrameLargeAllocated" : "AscendancyFrameLargeNormal", n.x, n.y);
          else sprite("frame", lit ? "NotableFrameAllocated" : "NotableFrameUnallocated", n.x, n.y);
          break;
        case "keystone":
          sprite(lit ? "keystoneActive" : "keystoneInactive", n.icon, n.x, n.y, 1, true);
          sprite("frame", lit ? "KeystoneFrameAllocated" : "KeystoneFrameUnallocated", n.x, n.y);
          break;
        case "mastery":
          if (lit) sprite("masteryActiveSelected", n.activeIcon, n.x, n.y) || sprite("mastery", n.icon, n.x, n.y);
          else sprite("masteryInactive", n.inactiveIcon, n.x, n.y) || sprite("mastery", n.icon, n.x, n.y);
          break;
        case "jewel": {
          sprite("frame", lit ? "JewelFrameAllocated" : "JewelFrameUnallocated", n.x, n.y);
          const jewel = jewelIn(tree, n.id);
          const img = jewel?.item.icon ? image(jewel.item.icon, redraw) : null;
          if (img) {
            const size = 58 / zoom;
            ctx.drawImage(img, n.x - size / 2, n.y - size / 2, size, size);
          }
          break;
        }
        case "ascendancyStart":
          sprite("ascendancy", "AscendancyMiddle", n.x, n.y);
          break;
      }
    }
  }

  /** The node under a point on the canvas, if any. */
  function nodeAt(px: number, py: number): TreeNode | null {
    const el = canvas.current;
    const cam = camera.current;
    if (!el || !cam || !shown) return null;
    const x = cam.x + (px - el.clientWidth / 2) / cam.s;
    const y = cam.y + (py - el.clientHeight / 2) / cam.s;
    let best: TreeNode | null = null;
    let bestD = Infinity;
    for (const n of shown.nodes) {
      if (n.kind === "classStart" || n.kind === "ascendancyStart") continue;
      const r = n.kind === "keystone" ? 85 : n.kind === "mastery" ? 70 : n.kind === "normal" ? 45 : 65;
      const d = Math.hypot(n.x - x, n.y - y);
      // At least a few pixels, so small nodes can be hit when zoomed out.
      if (d < Math.max(r, 7 / cam.s) && d < bestD) {
        best = n;
        bestD = d;
      }
    }
    return best;
  }

  // Dragging moves the tree; the wheel zooms round the pointer.
  const drag = useRef<{ x: number; y: number; cx: number; cy: number } | null>(null);
  useEffect(() => {
    const el = canvas.current;
    if (!el) return;
    const wheel = (e: WheelEvent) => {
      e.preventDefault();
      const cam = camera.current;
      if (!cam) return;
      const r = el.getBoundingClientRect();
      zoomAt(e.clientX - r.left, e.clientY - r.top, Math.exp(-e.deltaY * 0.0015));
    };
    el.addEventListener("wheel", wheel, { passive: false });
    return () => el.removeEventListener("wheel", wheel);
  }, [shown]);

  function zoomAt(px: number, py: number, factor: number) {
    const el = canvas.current;
    const cam = camera.current;
    if (!el || !cam) return;
    const s = Math.max(MIN_SCALE, Math.min(MAX_SCALE, cam.s * factor));
    // The tree point under the pointer stays under it.
    const wx = cam.x + (px - el.clientWidth / 2) / cam.s;
    const wy = cam.y + (py - el.clientHeight / 2) / cam.s;
    camera.current = { s, x: wx - (px - el.clientWidth / 2) / s, y: wy - (py - el.clientHeight / 2) / s };
    setHover(null);
    redraw();
  }

  if (error) return <p className="bad small">The passive tree couldn't be loaded: {error}</p>;
  if (!layout) return <div className="passive-tree loading muted small">Loading the passive tree…</div>;

  const jewel = hover?.node.kind === "jewel" ? jewelIn(tree, hover.node.id) : undefined;

  return (
    <div className="passive-tree" ref={box}>
      <canvas
        ref={canvas}
        className={drag.current ? "dragging" : ""}
        onMouseDown={(e) => {
          const cam = camera.current;
          if (!cam) return;
          drag.current = { x: e.clientX, y: e.clientY, cx: cam.x, cy: cam.y };
          setHover(null);
        }}
        onMouseMove={(e) => {
          const cam = camera.current;
          if (drag.current && cam && e.buttons & 1) {
            camera.current = {
              ...cam,
              x: drag.current.cx - (e.clientX - drag.current.x) / cam.s,
              y: drag.current.cy - (e.clientY - drag.current.y) / cam.s,
            };
            redraw();
            return;
          }
          drag.current = null;
          const r = e.currentTarget.getBoundingClientRect();
          const node = nodeAt(e.clientX - r.left, e.clientY - r.top);
          setHover(node ? { node, x: e.clientX, y: e.clientY } : null);
        }}
        onMouseUp={() => (drag.current = null)}
        onMouseLeave={() => {
          drag.current = null;
          setHover(null);
        }}
      />
      <div className="tree-controls">
        <button className="btn small-btn" title="Zoom in" onClick={() => zoomAt(box.current!.clientWidth / 2, box.current!.clientHeight / 2, 1.4)}>
          +
        </button>
        <button className="btn small-btn" title="Zoom out" onClick={() => zoomAt(box.current!.clientWidth / 2, box.current!.clientHeight / 2, 1 / 1.4)}>
          −
        </button>
        <button className="btn small-btn" title="Show the allocated passives" onClick={fit}>
          Fit
        </button>
      </div>
      {hover && (
        <NodeTip x={hover.x} y={hover.y}>
          {jewel ? (
            <ItemTooltip item={jewel.item} />
          ) : (
            <NodeText node={hover.node} pick={tree.masteryPicks[hover.node.id]} allocated={allocated.has(hover.node.id)} />
          )}
        </NodeTip>
      )}
    </div>
  );
}

function jewelIn(tree: TreeView, node: number): GearItem | undefined {
  const at = tree.sockets[node];
  return at == null ? undefined : tree.jewels[at];
}

/** A card beside the pointer, kept inside the window. */
function NodeTip({ x, y, children }: { x: number; y: number; children: React.ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  const [at, setAt] = useState<{ left: number; top: number } | null>(null);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const { width, height } = el.getBoundingClientRect();
    const gap = 16;
    let left = x + gap;
    if (left + width > window.innerWidth - 8) left = x - gap - width;
    const top = Math.max(8, Math.min(y + gap, window.innerHeight - height - 8));
    setAt({ left: Math.max(8, left), top });
  }, [x, y, children]);
  return (
    <div ref={ref} className="hover-card" style={at ?? { left: 0, top: 0, visibility: "hidden" }}>
      {children}
    </div>
  );
}

function NodeText({ node, pick, allocated }: { node: TreeNode; pick: string | undefined; allocated: boolean }) {
  const kind = node.ascendancy ? "ascendancy" : node.kind;
  return (
    <div className={`node-tip node-tip-${kind}`}>
      <div className="node-tip-name">{node.name || "Jewel Socket"}</div>
      {node.kind === "mastery" ? (
        pick ? (
          <div className="node-tip-stat">{pick}</div>
        ) : (
          <>
            <div className="node-tip-note muted">{allocated ? "Allocated" : "Not allocated"}: one of</div>
            {node.options.map((o, i) => (
              <div key={i} className="node-tip-stat muted">
                {o}
              </div>
            ))}
          </>
        )
      ) : (
        node.stats.map((s, i) => (
          <div key={i} className="node-tip-stat">
            {s}
          </div>
        ))
      )}
    </div>
  );
}
