// An item as the game and the trade site show it: its picture with sockets and links,
// and its tooltip. Shared by Searches (listings) and Builds (gear).

import { useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import type { HitItem, HitMod, HitProperty, SearchSort } from "./searches";

/** Shows `card` beside what's hovered, on top of the page: to its right, or its left
 *  where the right has no room, and kept inside the window. */
export function HoverCard({
  card,
  children,
  className,
  style,
  onClick,
}: {
  card: ReactNode;
  children: ReactNode;
  className?: string;
  style?: React.CSSProperties;
  onClick?: (e: React.MouseEvent) => void;
}) {
  const [anchor, setAnchor] = useState<DOMRect | null>(null);
  const cardRef = useRef<HTMLDivElement>(null);
  const [at, setAt] = useState<{ left: number; top: number } | null>(null);

  useLayoutEffect(() => {
    const el = cardRef.current;
    if (!anchor || !el) {
      setAt(null);
      return;
    }
    const gap = 8;
    const margin = 8;
    const { width, height } = el.getBoundingClientRect();
    const roomRight = window.innerWidth - anchor.right - gap - margin;
    let left = roomRight >= width ? anchor.right + gap : anchor.left - gap - width;
    left = Math.max(margin, Math.min(left, window.innerWidth - width - margin));
    let top = Math.min(anchor.top, window.innerHeight - height - margin);
    top = Math.max(margin, top);
    setAt({ left, top });
  }, [anchor]);

  return (
    <div
      className={className}
      style={style}
      onClick={onClick}
      onMouseEnter={(e) => setAnchor(e.currentTarget.getBoundingClientRect())}
      onMouseLeave={() => setAnchor(null)}
    >
      {children}
      {anchor &&
        createPortal(
          <div
            ref={cardRef}
            className="hover-card"
            style={at ? { left: at.left, top: at.top } : { left: 0, top: 0, visibility: "hidden" }}
          >
            {card}
          </div>,
          document.body,
        )}
    </div>
  );
}

/** Sorting by a clicked value, where the results can be sorted (an ordinary search). */
export interface Sorting {
  current: SearchSort | null;
  onSort: (key: string, label: string) => void;
}

/** A value results can be sorted by: click to sort, again to turn the order round. */
export function Sortable({
  sorting,
  sortKey,
  label,
  row,
  children,
}: {
  sorting?: Sorting;
  sortKey: string | null | undefined;
  label: string;
  /** Lay out as a row (around a block, such as the price), the arrow beside it. */
  row?: boolean;
  children: React.ReactNode;
}) {
  if (!sorting || !sortKey) return <>{children}</>;
  const on = sorting.current?.key === sortKey;
  return (
    <span
      className={`sortable ${on ? "sorted" : ""} ${row ? "row" : ""}`}
      title={`Sort by ${label}`}
      onClick={(e) => {
        e.stopPropagation();
        sorting.onSort(sortKey, label);
      }}
    >
      {children}
      {on && <span className="sort-arrow">{sorting.current!.dir === "desc" ? " ▼" : " ▲"}</span>}
    </span>
  );
}

/** The item as the game's tooltip shows it: a header, then sections with lines between,
 *  and the trade site's line under it (base percentile, defences at 20% quality, DPS).
 *  With `sorting`, values the site sorts by can be clicked to sort. */
export function ItemTooltip({
  item,
  note,
  sorting,
}: {
  item: HitItem;
  /** The price note at the bottom ("~b/o 1 alch"), when the item has none of its own. */
  note?: string | null;
  sorting?: Sorting;
}) {
  const named = item.name && item.rarity !== "magic" && item.rarity !== "normal";
  const mods = (kinds: HitMod["kind"][]) => item.mods.filter((m) => kinds.includes(m.kind));
  const value = (p: HitProperty) => (
    <span className={p.augmented ? "tt-augmented" : "tt-value"}>{p.value}</span>
  );

  const sections: React.ReactNode[] = [];
  if (item.properties.length > 0) {
    sections.push(
      item.properties.map((p, i) => (
        <div key={i}>
          {p.value == null ? (
            <span className="tt-label">{p.label}</span>
          ) : (
            <Sortable sorting={sorting} sortKey={p.sort} label={p.label}>
              <span className="tt-label">{p.label}: </span>
              {value(p)}
            </Sortable>
          )}
        </div>
      )),
    );
  }
  if (item.itemLevel != null || item.requirements.length > 0) {
    sections.push(
      <>
        {item.itemLevel != null && (
          <div>
            <Sortable sorting={sorting} sortKey="ilvl" label="Item Level">
              <span className="tt-label">Item Level: </span>
              <span className="tt-value">{item.itemLevel}</span>
            </Sortable>
          </div>
        )}
        {item.requirements.length > 0 && (
          <div>
            <span className="tt-label">Requires </span>
            {item.requirements.map((r, i) => (
              <span key={i}>
                {i > 0 && <span className="tt-label">, </span>}
                <Sortable sorting={sorting} sortKey={r.sort} label={r.name === "Level" ? "required level" : `required ${r.name}`}>
                  {r.name === "Level" ? (
                    <>
                      <span className="tt-label">Level </span>
                      <span className="tt-value">{r.value}</span>
                    </>
                  ) : (
                    <>
                      <span className="tt-value">{r.value}</span>
                      <span className="tt-label"> {r.name}</span>
                    </>
                  )}
                </Sortable>
              </span>
            ))}
          </div>
        )}
      </>,
    );
  }
  for (const kinds of [["enchant"], ["implicit"], ["fractured", "explicit", "crafted", "mutated"]] as HitMod["kind"][][]) {
    const lines = mods(kinds);
    if (lines.length > 0) {
      sections.push(
        lines.map((m, i) => (
          <div key={i} className={`tt-mod mod-${m.kind}`}>
            {/* As on the trade site: hover a mod to see what it can roll, at the tooltip's left edge. */}
            <Sortable sorting={sorting} sortKey={m.sort} label={m.text}>
              <span className="tt-line">
                {m.text}
                {m.range && <span className="mod-range">[{m.range.replace(/–/g, "—")}]</span>}
              </span>
            </Sortable>
          </div>
        )),
      );
    }
  }
  const flags = [
    item.unidentified && <div key="u" className="tt-unidentified">Unidentified</div>,
    item.mirrored && <div key="m" className="tt-mod">Mirrored</div>,
    item.corrupted && <div key="c" className="tt-corrupted">Corrupted</div>,
  ].filter(Boolean);
  if (flags.length > 0) sections.push(flags);
  const priceNote = item.note ?? note;
  if (priceNote) sections.push(<div className="tt-note">{priceNote}</div>);

  return (
    <div className="tooltip-wrap">
      <div className={`tooltip rarity-${item.rarity}`}>
        <header className={`tt-head ${named ? "double" : ""}`}>
          {named ? (
            <>
              <div>{item.name}</div>
              <div>{item.typeLine}</div>
            </>
          ) : (
            <div>{item.typeLine || item.name}</div>
          )}
        </header>
        <div className="tt-body">
          {sections.map((section, i) => (
            <div key={i} className="tt-section">
              {i > 0 && <hr className="tt-sep" />}
              {section}
            </div>
          ))}
        </div>
      </div>
      {item.extended.length > 0 && (
        <div className="tt-extended">
          {item.extended.map((p, i) => (
            <Sortable key={i} sorting={sorting} sortKey={p.sort} label={p.label}>
              <span className="tt-label">{p.label}: </span>
              {value(p)}
            </Sortable>
          ))}
        </div>
      )}
    </div>
  );
}

/** One inventory cell, in pixels, as the trade site draws them: the picture is drawn
 *  at its size in cells, with the sockets on it. */
const CELL = 47.4645;
/** A socket, and a link's long and short side (src/assets/socket.png). */
const SOCKET = 35;
const LINK_LONG = 38;
const LINK_SHORT = 15;

/** The item's picture with its sockets and links on it, as on the trade site. */
/** `sockets` false leaves them off, as the game does until the item's hovered. */
export function ItemPicture({ item, sockets = true }: { item: HitItem; sockets?: boolean }) {
  return (
    <div className="item-picture" style={{ width: item.width * CELL, height: item.height * CELL }}>
      {item.icon && <img src={item.icon} alt="" loading="lazy" />}
      {sockets && item.sockets && <Sockets sockets={item.sockets} width={item.width} />}
    </div>
  );
}

/** Where the game puts socket `i` of an item `width` cells wide: along the first row,
 *  back along the next, and so on. */
function socketCell(i: number, width: number): { col: number; row: number } {
  const row = Math.floor(i / width);
  const along = i % width;
  return { row, col: row % 2 === 0 ? along : width - 1 - along };
}

/** Sockets and links as the trade site draws them, from its own picture of them
 *  ("R-G-B B": linked groups joined by "-"). */
function Sockets({ sockets, width }: { sockets: string; width: number }) {
  const list = sockets
    .split(" ")
    .flatMap((group, g) => group.split("-").map((colour) => ({ colour, group: g })));

  return (
    <div className="sockets" title={`Sockets: ${sockets}`}>
      {list.map((s, i) => {
        const { col, row } = socketCell(i, width);
        return (
          <span
            key={i}
            className={`socket socket-${s.colour}`}
            style={{ left: col * CELL + (CELL - SOCKET) / 2, top: row * CELL + (CELL - SOCKET) / 2 }}
          />
        );
      })}
      {list.slice(1).map((s, j) => {
        if (s.group !== list[j].group) return null;
        const a = socketCell(j, width);
        const b = socketCell(j + 1, width);
        // Between two sockets side by side, or one above the other.
        const across = a.row === b.row;
        const style = across
          ? {
              left: Math.min(a.col, b.col) * CELL + CELL / 2 + (CELL - LINK_LONG) / 2,
              top: a.row * CELL + (CELL - LINK_SHORT) / 2,
            }
          : {
              left: a.col * CELL + (CELL - LINK_SHORT) / 2,
              top: Math.min(a.row, b.row) * CELL + CELL / 2 + (CELL - LINK_LONG) / 2,
            };
        return <span key={`l${j}`} className={`socket-link ${across ? "across" : "down"}`} style={style} />;
      })}
    </div>
  );
}
