// What a tree takes, as cards: its tattoos and runegrafts, keystones and chosen
// masteries, each with the game's picture.

import { useEffect, useState } from "react";
import { treeLayout, type TreeLayout, type TreeView } from "./builds";

/** The passive tree's layout once it has arrived (shared, fetched once). */
export function useTreeLayout(): TreeLayout | null {
  const [layout, setLayout] = useState<TreeLayout | null>(null);
  useEffect(() => {
    let live = true;
    treeLayout().then(
      (l) => live && setLayout(l),
      () => {},
    );
    return () => {
      live = false;
    };
  }, []);
  return layout;
}

/** One picture from GGG's sheets, `size` pixels across. */
function Sprite({ layout, sheet, name, size }: { layout: TreeLayout; sheet: string; name: string | null | undefined; size: number }) {
  const s = layout.sprites[sheet];
  const at = name ? s?.coords[name] : undefined;
  if (!s || !at) return <span className="tree-card-icon" style={{ width: size, height: size }} />;
  const [x, y, w, h] = at;
  const scale = size / Math.max(w, h);
  return (
    <span className="tree-card-icon" style={{ width: size, height: size }}>
      <span
        style={{
          width: w,
          height: h,
          background: `url("${s.url}") -${x}px -${y}px no-repeat`,
          transform: `scale(${scale})`,
          transformOrigin: "top left",
          marginLeft: (size - w * scale) / 2,
          marginTop: (size - h * scale) / 2,
        }}
      />
    </span>
  );
}

interface Card {
  key: string;
  name: string;
  kind: "override" | "keystone" | "mastery";
  sheet?: string;
  icon?: string | null;
  lines: string[];
}

function cards(tree: TreeView, layout: TreeLayout | null): Card[] {
  const node = (name: string, kind: string) => layout?.nodes.find((n) => n.name === name && n.kind === kind);

  // The same tattoo on many passives is one card.
  const overrides = new Map<string, { count: number; stats: string[] }>();
  for (const o of Object.values(tree.overrides ?? {})) {
    const seen = overrides.get(o.name);
    if (seen) seen.count++;
    else overrides.set(o.name, { count: 1, stats: o.stats });
  }

  // "Life Mastery: Skills Cost Life…": one card a mastery, with each effect taken.
  const masteries = new Map<string, string[]>();
  for (const m of tree.masteries) {
    const at = m.indexOf(": ");
    const [name, effect] = at < 0 ? [m, ""] : [m.slice(0, at), m.slice(at + 2)];
    masteries.set(name, [...(masteries.get(name) ?? []), ...(effect ? [effect] : [])]);
  }

  const byName = (a: Card, b: Card) => a.name.localeCompare(b.name);
  return [
    ...[...overrides]
      .map(([name, o]): Card => ({ key: `o${name}`, name: o.count > 1 ? `${name} ×${o.count}` : name, kind: "override", lines: o.stats }))
      .sort(byName),
    ...tree.keystones
      .map((name): Card => ({ key: `k${name}`, name, kind: "keystone", sheet: "keystoneActive", icon: node(name, "keystone")?.icon, lines: [] }))
      .sort(byName),
    ...[...masteries]
      .map(([name, lines]): Card => {
        // The lit picture, as the game shows a mastery that's taken.
        const n = node(name, "mastery");
        const lit = n?.activeIcon && layout?.sprites.masteryActiveSelected?.coords[n.activeIcon];
        return { key: `m${name}`, name, kind: "mastery", sheet: lit ? "masteryActiveSelected" : "mastery", icon: lit ? n.activeIcon : n?.icon, lines };
      })
      .sort(byName),
  ];
}

export function TreeSummary({ tree, layout }: { tree: TreeView; layout: TreeLayout | null }) {
  const list = cards(tree, layout);
  if (list.length === 0) return null;
  return (
    <div className="tree-cards">
      {list.map((c) => (
        <div key={c.key} className={`tree-card tree-card-${c.kind}`}>
          <div className="tree-card-name">
            {layout && c.sheet && <Sprite layout={layout} sheet={c.sheet} name={c.icon} size={24} />}
            {c.name}
          </div>
          {c.lines.map((l, i) => (
            <div key={i} className="tree-card-line">
              {l}
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}
