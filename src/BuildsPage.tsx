import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  buildView,
  calcLoadouts,
  importBuild,
  listBuilds,
  removeBuild,
  updateBuild,
  type BuildView,
  type GearItem,
  type Gem,
  type GemGroup,
  type Loadout,
  type SavedBuild,
} from "./builds";
import { addGroup, listGroups, type HitItem, type SearchGroup } from "./searches";
import { HoverCard, ItemPicture, ItemTooltip } from "./ItemView";
import { PassiveTree } from "./PassiveTree";

function remembered(): string | null {
  try {
    return localStorage.getItem("builds.selected");
  } catch {
    return null;
  }
}

function remember(id: string) {
  try {
    localStorage.setItem("builds.selected", id);
  } catch {
    // Only a convenience.
  }
}

/** Builds from Path of Building: stats, gear, gems and tree. A build may use a group
 *  of searches from Searches; the two are apart otherwise. */
export function BuildsPage({ onOpenSearches }: { onOpenSearches: (groupId: string) => void }) {
  const [builds, setBuilds] = useState<SavedBuild[]>([]);
  const [groups, setGroups] = useState<SearchGroup[]>([]);
  const [selected, setSelected] = useState<string | null>(remembered);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const reload = () =>
    Promise.all([listBuilds(), listGroups()]).then(
      ([b, g]) => {
        setBuilds(b);
        setGroups(g);
      },
      (e) => setError(String(e)),
    );
  useEffect(() => {
    reload();
  }, []);

  function select(id: string) {
    setSelected(id);
    remember(id);
  }

  async function add() {
    setBusy(true);
    try {
      const id = await importBuild(input);
      setInput("");
      setError(null);
      await reload();
      select(id);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  const build = builds.find((b) => b.id === selected) ?? builds[0];

  return (
    <div className="searches">
      <aside className="builds">
        <form
          className="build-import"
          onSubmit={(e) => {
            e.preventDefault();
            if (input.trim()) add();
          }}
        >
          <input
            type="text"
            placeholder="pobb.in link or PoB code"
            value={input}
            onChange={(e) => setInput(e.target.value)}
          />
          <button className="btn primary" type="submit" disabled={!input.trim() || busy}>
            {busy ? "Importing…" : "Import build"}
          </button>
          {error && <p className="bad small">{error}</p>}
          <p className="hint">A pobb.in, pastebin or poe.ninja link, or a code from Path of Building's Export.</p>
        </form>
        <div className="builds-head">
          <span className="label">Builds</span>
        </div>
        <div className="builds-scroll">
          {builds.map((b) => (
            <button key={b.id} className={`build-item ${build?.id === b.id ? "active" : ""}`} onClick={() => select(b.id)}>
              <span className="build-name">{b.name}</span>
            </button>
          ))}
          {builds.length === 0 && <p className="muted small builds-empty">Import a build to start.</p>}
        </div>
      </aside>
      <section className="searches-main" key={build?.id ?? "none"}>
        {build ? (
          <Build build={build} searchGroups={groups} onChanged={reload} onOpenSearches={onOpenSearches} />
        ) : (
          <p className="muted">Import a build from Path of Building to see its gear, gems, tree and stats.</p>
        )}
      </section>
    </div>
  );
}

/** Which group of searches a build uses: none, one from Searches, or a new one
 *  named after it. */
function SearchesPick({
  build,
  groups,
  onChanged,
  onOpen,
}: {
  build: SavedBuild;
  groups: SearchGroup[];
  onChanged: () => void;
  onOpen: (groupId: string) => void;
}) {
  const linked = groups.find((g) => g.id === build.searches);
  async function pick(value: string) {
    if (value === "new") {
      const group = await addGroup(build.name);
      await updateBuild(build.id, { searches: group.id });
    } else {
      await updateBuild(build.id, { searches: value || null });
    }
    onChanged();
  }
  return (
    <span className="searches-pick">
      <select className="set-select" value={linked?.id ?? ""} onChange={(e) => pick(e.target.value)} title="The group of searches this build uses">
        <option value="">No searches</option>
        {groups.map((g) => (
          <option key={g.id} value={g.id}>
            Searches: {g.name} ({g.searches.length})
          </option>
        ))}
        <option value="new">New group of searches…</option>
      </select>
      {linked && (
        <button className="btn small-btn" onClick={() => onOpen(linked.id)}>
          Open searches
        </button>
      )}
    </span>
  );
}

function Build({
  build,
  searchGroups,
  onChanged,
  onOpenSearches,
}: {
  build: SavedBuild;
  searchGroups: SearchGroup[];
  onChanged: () => void;
  onOpenSearches: (groupId: string) => void;
}) {
  const [view, setView] = useState<BuildView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  // Which sets are shown: the exported loadout at first.
  const [sel, setSel] = useState<Loadout | null>(null);
  // Stats PoB calculated for other loadouts, by their sets.
  const [calcs, setCalcs] = useState<Record<string, Calc>>({});

  // Asks PoB for loadouts' stats, all in one go, and keeps them for this build.
  function calculate(picks: Loadout[]) {
    const wanted = picks.filter((p) => !calcs[setsKey(p)] && !calcCache.get(`${build.id}|${setsKey(p)}`));
    if (wanted.length === 0) return;
    const store = (key: string, calc: Calc) => {
      if (!calc.busy && !calc.error) calcCache.set(`${build.id}|${key}`, calc);
      setCalcs((c) => ({ ...c, [key]: calc }));
    };
    for (const p of wanted) store(setsKey(p), { busy: true });
    calcLoadouts(build.id, wanted).then(
      (results) =>
        wanted.forEach((p, i) => {
          const r = results[i];
          store(setsKey(p), r?.stats ? { busy: false, stats: r.stats, version: r.pobVersion } : { busy: false, error: r?.error ?? "No answer." });
        }),
      (e) => wanted.forEach((p) => store(setsKey(p), { busy: false, error: String(e) })),
    );
  }

  // The build's loadouts are calculated as soon as it's open, so switching is instant;
  // a mix of sets of its own when it's chosen.
  useEffect(() => {
    if (!view) return;
    const known: Record<string, Calc> = {};
    for (const l of view.loadouts) {
      const hit = calcCache.get(`${build.id}|${setsKey(l)}`);
      if (hit) known[setsKey(l)] = hit;
    }
    setCalcs((c) => ({ ...known, ...c }));
    calculate(view.loadouts.filter((l) => !sameSets(l, view.active) && !known[setsKey(l)]));
  }, [view]);

  useEffect(() => {
    if (!view || !sel || sameSets(sel, view.active)) return;
    if (!view.loadouts.some((l) => sameSets(l, sel))) calculate([sel]);
  }, [sel]);

  useEffect(() => {
    buildView(build.id).then(
      (v) => {
        setView(v);
        setSel(v.active);
      },
      (e) => setError(String(e)),
    );
  }, [build.id]);

  if (error) return <p className="bad">{error}</p>;
  if (!view || !sel) return <p className="muted">Reading the build…</p>;

  const gear = view.itemSets[sel.itemSet]?.gear ?? [];
  const tree = view.trees[sel.tree];
  const groups = view.skillSets[sel.skillSet]?.groups ?? [];
  const config = view.configs[sel.config];
  const loadoutAt = view.loadouts.findIndex(
    (l) => l.itemSet === sel.itemSet && l.skillSet === sel.skillSet && l.tree === sel.tree && l.config === sel.config,
  );
  const exported = sameSets(sel, view.active);
  const calc = exported ? undefined : (calcs[setsKey(sel)] ?? calcCache.get(`${build.id}|${setsKey(sel)}`));
  const stats = exported ? view.stats : (calc?.stats ?? view.stats);

  return (
    <div className="build">
      <header className="build-head">
        <div className="build-title-line">
          <h1>
            Level {view.level} {view.mainSkill ? `${view.mainSkill} ` : ""}
            {view.ascendancy ?? view.class}
          </h1>
          {view.version && <span className="build-version">{view.version.replace("_", ".")}</span>}
        </div>
        <BuildStats stats={stats} config={config} />
        {!exported && (
          <p className={`small build-stats-note ${calc?.error ? "bad" : "muted"}`}>
            {calc?.busy
              ? "Calculating this loadout with Path of Building…"
              : calc?.stats
                ? `Calculated by Path of Building${calc.version ? ` ${calc.version}` : ""} for these sets.`
                : `${calc?.error ?? ""} The stats are PoB's for ${view.active.title ? `"${view.active.title}"` : "the build as exported"}.`}
          </p>
        )}
        <div className="build-actions">
          <button
            className="btn small-btn"
            onClick={() => {
              navigator.clipboard.writeText(view.code);
              setCopied(true);
              setTimeout(() => setCopied(false), 1500);
            }}
          >
            {copied ? "Copied" : "Copy PoB code"}
          </button>
          {view.source && (
            <button className="btn small-btn" onClick={() => openUrl(view.source!)}>
              Open source ↗
            </button>
          )}
          <SearchesPick build={build} groups={searchGroups} onChanged={onChanged} onOpen={onOpenSearches} />
          <button
            className="btn small-btn"
            onClick={() =>
              confirm(`Remove ${build.name}?${build.searches ? " Its searches stay in Searches." : ""}`) &&
              removeBuild(build.id).then(onChanged, (e) => setError(String(e)))
            }
          >
            Remove build
          </button>
        </div>
      </header>

      {view.loadouts.length > 1 && (
        <section className="set-pick loadout-pick">
          <div className="label">Loadout</div>
          <select
            value={loadoutAt}
            onChange={(e) => {
              const l = view.loadouts[Number(e.target.value)];
              if (l) setSel(l);
            }}
          >
            {loadoutAt < 0 && <option value={-1}>Mixed</option>}
            {view.loadouts.map((l, i) => (
              <option key={i} value={i}>
                {l.title}
              </option>
            ))}
          </select>
        </section>
      )}

      <div className="build-columns">
        <section>
          <div className="section-head">
            <div className="label">Gear</div>
            <SetPick sets={view.itemSets} value={sel.itemSet} onPick={(itemSet) => setSel({ ...sel, itemSet })} />
          </div>
          <Gear gear={gear} jewels={tree?.jewels ?? []} />
        </section>
        <section>
          <div className="section-head">
            <div className="label">Gems</div>
            <SetPick sets={view.skillSets} value={sel.skillSet} onPick={(skillSet) => setSel({ ...sel, skillSet })} />
          </div>
          <div className="gem-groups">
            {groups
              .filter((g) => g.gems.length > 0)
              .map((g, i) => (
                <GemGroupCard key={i} group={g} />
              ))}
          </div>
        </section>
      </div>

      {tree && (
        <section className="build-tree">
          <div className="section-head">
            <div className="label">
              Tree · {tree.points} passives
              {tree.ascendancyPoints > 0 && `, ${tree.ascendancyPoints} ascendancy`}
            </div>
            <SetPick sets={view.trees} value={sel.tree} onPick={(t) => setSel({ ...sel, tree: t })} />
          </div>
          <PassiveTree tree={tree} className={view.class} ascendancy={view.ascendancy} />
          <NodeList title="Keystones" names={tree.keystones} kind="keystone" />
          <NodeList title={view.ascendancy ?? "Ascendancy"} names={tree.ascendancyNotables} kind="ascendancy" />
          <NodeList title="Notables" names={tree.notables} kind="notable" />
          <NodeList title="Masteries" names={tree.masteries} kind="mastery" />
        </section>
      )}

      {view.notes && (
        <section>
          <div className="label">Notes</div>
          <pre className="build-notes">
            <PobText text={view.notes} />
          </pre>
        </section>
      )}
    </div>
  );
}

const setsKey = (l: Loadout) => `${l.itemSet}-${l.skillSet}-${l.tree}-${l.config}`;
const sameSets = (a: Loadout, b: Loadout) => setsKey(a) === setsKey(b);
/** Loadouts PoB calculated, by build and sets, while the app runs. */
const calcCache = new Map<string, Calc>();

/** Stats PoB calculated for one combination of sets. */
interface Calc {
  busy: boolean;
  stats?: Record<string, number>;
  version?: string | null;
  error?: string;
}

/* Stats */

const num = (n: number, digits = 0) =>
  n.toLocaleString("en-US", { maximumFractionDigits: digits, minimumFractionDigits: 0 });

function BuildStats({ stats, config }: { stats: Record<string, number>; config: BuildView["configs"][number] | undefined }) {
  const s = stats;
  const has = (k: string) => s[k] != null && s[k] !== 0;
  const stat = (label: string, value: React.ReactNode, cls = "") => (
    <span className="bstat">
      <span className="bstat-label">{label}:</span> <span className={cls}>{value}</span>
    </span>
  );
  const res = (k: string, cls: string) => (
    <span className={cls}>
      {num(s[k] ?? 0)}%
      {(s[`${k}OverCap`] ?? 0) > 0 && <sup>{num(s[`${k}OverCap`])}</sup>}
    </span>
  );
  const dps = Math.max(s.FullDPS ?? 0, s.CombinedDPS ?? 0, s.TotalDPS ?? 0);

  return (
    <div className="build-stats">
      <div>
        {stat(
          "Life",
          // With life reserved (low life), what's left.
          s.LifeUnreserved != null && s.LifeUnreserved < (s.Life ?? 0) ? (
            <>
              {num(s.LifeUnreserved)} <span className="muted">(of {num(s.Life)})</span>
            </>
          ) : (
            num(s.Life ?? 0)
          ),
          "c-life",
        )}
        {has("EnergyShield") && stat("ES", num(s.EnergyShield), "c-es")}
        {stat("Mana", num(s.Mana ?? 0), "c-mana")}
        {has("Ward") && stat("Ward", num(s.Ward))}
        {stat("Str", num(s.Str ?? 0), "c-str")}
        {stat("Dex", num(s.Dex ?? 0), "c-dex")}
        {stat("Int", num(s.Int ?? 0), "c-int")}
        {has("TotalEHP") && stat("eHP", num(s.TotalEHP))}
      </div>
      <div>
        {stat(
          "Resistances",
          <>
            {res("FireResist", "c-fire")}/{res("ColdResist", "c-cold")}/{res("LightningResist", "c-light")}/
            {res("ChaosResist", "c-chaos")}
          </>,
        )}
        {has("MeleeEvadeChance") && stat("Evade", `${num(s.MeleeEvadeChance)}%`)}
        {has("Evasion") && stat("Evasion", num(s.Evasion))}
        {has("Armour") && stat("Armour", num(s.Armour))}
        {has("EffectiveBlockChance") && stat("Block", `${num(s.EffectiveBlockChance)}%`)}
        {has("EffectiveSpellBlockChance") && stat("Spell Block", `${num(s.EffectiveSpellBlockChance)}%`)}
        {has("EffectiveSpellSuppressionChance") && stat("Suppression", `${num(s.EffectiveSpellSuppressionChance)}%`)}
      </div>
      <div>
        {stat("DPS", num(dps), "c-dps")}
        {has("TotalDotDPS") && stat("DoT DPS", num(s.TotalDotDPS))}
        {has("Speed") && stat("Speed", num(s.Speed, 2))}
        {has("HitChance") && stat("Hit Chance", `${num(s.HitChance)}%`)}
        {has("CritChance") && stat("Crit Chance", `${num(s.CritChance, 2)}%`)}
        {has("CritMultiplier") && stat("Crit Multi", `${num(s.CritMultiplier * 100)}%`)}
        {has("EffectiveMovementSpeedMod") && stat("Movement", `${num(s.EffectiveMovementSpeedMod * 100)}%`)}
      </div>
      {config && config.config.length > 0 && <div>{stat("Config", config.config.join(", "))}</div>}
      {config && (config.bandit || config.pantheon.length > 0) && (
        <div>
          {config.bandit && stat("Bandit", config.bandit)}
          {config.pantheon.length > 0 && stat("Pantheon", config.pantheon.join(", "))}
        </div>
      )}
    </div>
  );
}

/* Gear in the inventory's layout */

/** Where each slot sits in the inventory (grid areas, cells of 47.46px). */
const SLOT_AREA: Record<string, string> = {
  "Weapon 1": "w1",
  "Weapon 2": "w2",
  Helmet: "helm",
  "Body Armour": "body",
  Gloves: "gloves",
  Boots: "boots",
  Belt: "belt",
  Amulet: "amulet",
  "Ring 1": "ring1",
  "Ring 2": "ring2",
};

function Gear({ gear, jewels }: { gear: GearItem[]; jewels: GearItem[] }) {
  const main = gear.filter((g) => SLOT_AREA[g.slot]);
  const flasks = gear.filter((g) => g.slot.startsWith("Flask") || g.slot.startsWith("Graft"));

  return (
    <div className="gear">
      <div className="inventory">
        {Object.values(SLOT_AREA).map((area) => (
          <div key={area} className="inv-slot" style={{ gridArea: area }} />
        ))}
        {main.map((g) => (
          <GearSlot key={g.slot} item={g.item} area={SLOT_AREA[g.slot]} />
        ))}
      </div>
      {flasks.length > 0 && (
        <div className="gear-row">
          {flasks
            .slice()
            .sort((a, b) => a.slot.localeCompare(b.slot))
            .map((g) => (
              <GearSlot key={g.slot} item={g.item} />
            ))}
        </div>
      )}
      {jewels.length > 0 && (
        <>
          <div className="label small-label">Jewels</div>
          <div className="gear-row">
            {jewels.map((g, i) => (
              <GearSlot key={i} item={g.item} />
            ))}
          </div>
        </>
      )}
    </div>
  );
}

/** An item in its slot; hovering shows its sockets and tooltip, as in the game. */
function GearSlot({ item, area }: { item: HitItem; area?: string }) {
  const [hover, setHover] = useState(false);
  return (
    <div onMouseEnter={() => setHover(true)} onMouseLeave={() => setHover(false)} style={area ? { gridArea: area } : undefined} className="gear-cell">
      <HoverCard
        className={`gear-slot ${area ? "" : "loose"}`}
        card={
          <div className="gear-pop">
            <ItemTooltip item={item} />
          </div>
        }
      >
        {item.icon ? (
          <ItemPicture item={item} sockets={hover} />
        ) : (
          <div className={`gear-noicon rarity-${item.rarity}`} style={{ width: item.width * 47.4645, height: item.height * 47.4645 }}>
            {item.typeLine}
          </div>
        )}
      </HoverCard>
    </div>
  );
}

/** A dropdown of a kind of set, when the build has more than one. */
function SetPick({
  sets,
  value,
  onPick,
}: {
  sets: { title: string | null }[];
  value: number;
  onPick: (at: number) => void;
}) {
  if (sets.length < 2) return null;
  return (
    <select className="set-select" value={value} onChange={(e) => onPick(Number(e.target.value))}>
      {sets.map((s, i) => (
        <option key={i} value={i}>
          {s.title ?? `Set ${i + 1}`}
        </option>
      ))}
    </select>
  );
}

/* Gems */

function GemGroupCard({ group }: { group: GemGroup }) {
  const actives = group.gems.filter((g) => !g.support);
  const supports = group.gems.filter((g) => g.support);
  const label = group.label ?? (group.source ? `From ${group.source}` : "");
  return (
    <div className={`gem-group ${group.main ? "main" : ""} ${group.enabled ? "" : "off"}`}>
      {(label || group.slot) && (
        <div className="gem-group-head muted small">
          <span>{label}</span>
          {group.slot && <SlotIcon slot={group.slot} />}
        </div>
      )}
      {actives.map((g, i) => (
        <GemLine key={`a${i}`} gem={g} />
      ))}
      {supports.map((g, i) => (
        <GemLine key={`s${i}`} gem={g} branch={i === supports.length - 1 ? "└" : "├"} />
      ))}
    </div>
  );
}

/** A gem in its group; hovering shows what the wiki says, Ctrl+Click opens its page. */
function GemLine({ gem, branch }: { gem: Gem; branch?: string }) {
  const line = (
    <>
      {branch && <span className="gem-branch">{branch}</span>}
      {gem.name} <span className="gem-lvl">{gem.level}/{gem.quality}</span>
    </>
  );
  const className = `gem ${branch ? "support" : ""} gem-${gem.colour ?? "white"} ${gem.enabled ? "" : "off"}`;
  const info = gem.info;
  if (!info) return <div className={className}>{line}</div>;
  return (
    <HoverCard
      className={className}
      onClick={(e) => {
        if (e.ctrlKey || e.metaKey) void openUrl(info.wiki);
      }}
      card={<GemTooltip gem={gem} />}
    >
      {line}
    </HoverCard>
  );
}

function GemTooltip({ gem }: { gem: Gem }) {
  const info = gem.info!;
  return (
    <div className="gem-tip">
      <div className="gem-tip-head">
        {info.icon && <img src={info.icon} alt="" />}
        <div>
          <div className={`gem-tip-name gem-${gem.colour ?? "white"}`}>{gem.name}</div>
          <div className="gem-tip-level">
            Level: {gem.level}
            {gem.quality > 0 && <> · Quality: +{gem.quality}%</>}
          </div>
        </div>
      </div>
      {info.requiredLevel != null && (
        <div className="gem-tip-row">
          <span className="muted">Minimum Level</span>
          <span>{info.requiredLevel}</span>
        </div>
      )}
      {info.rewards.length > 0 && (
        <table className="gem-tip-rewards">
          <thead>
            <tr>
              <th>Act</th>
              <th>NPC</th>
              <th>Quest</th>
            </tr>
          </thead>
          <tbody>
            {info.rewards.map((r, i) => (
              <tr key={i}>
                <td>{r.act}</td>
                <td>{r.npc ?? <span className="gem-tip-reward">Quest reward</span>}</td>
                <td>{r.quest}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      <div className="gem-tip-foot muted">Wiki: Ctrl+Click</div>
    </div>
  );
}

/** The kind of item a gem group is socketed in, as a small picture ("Helmet",
 *  "Weapon 1 Swap"); its name on hover. */
function SlotIcon({ slot }: { slot: string }) {
  const kind = slot.startsWith("Weapon 2")
    ? "offhand"
    : slot.startsWith("Weapon")
      ? "weapon"
      : slot.startsWith("Ring")
        ? "ring"
        : ({ Helmet: "helmet", "Body Armour": "body", Gloves: "gloves", Boots: "boots", Amulet: "amulet", Belt: "belt" } as Record<
            string,
            string
          >)[slot];
  const paths: Record<string, React.ReactNode> = {
    helmet: (
      <>
        <path d="M4 15c0-5 3.6-9 8-9s8 4 8 9v3H4z" />
        <path d="M9 18v-4h6v4M12 6V3" />
      </>
    ),
    body: <path d="M8 3l4 2 4-2 4 4-3 3v11H7V10L4 7z" />,
    gloves: <path d="M7 21v-7L5 9.5a1.5 1.5 0 0 1 2.6-1.5L9 10V4.5a1.5 1.5 0 0 1 3 0V9V3.5a1.5 1.5 0 0 1 3 0V9 5a1.5 1.5 0 0 1 3 0v9l-2 7z" />,
    boots: <path d="M7 3h7v10l6 3v5H5v-4l2-3z" />,
    weapon: (
      <>
        <path d="M20 4l-9 9M20 4h-4l-9 9 4 4 9-9z" />
        <path d="M6 14l4 4M4 20l3-3" />
      </>
    ),
    offhand: <path d="M12 3l8 3v6c0 5-3.5 8-8 9-4.5-1-8-4-8-9V6z" />,
    ring: (
      <>
        <circle cx="12" cy="14" r="6" />
        <path d="M10 8l2-3 2 3" />
      </>
    ),
    amulet: (
      <>
        <path d="M5 3c0 6 3 9 7 9s7-3 7-9" />
        <path d="M12 12l-3 4 3 5 3-5z" />
      </>
    ),
    belt: (
      <>
        <rect x="2" y="9" width="20" height="6" rx="1" />
        <rect x="9" y="8" width="6" height="8" rx="1" />
      </>
    ),
  };
  return (
    <span className="slot-icon" title={slot}>
      {kind && paths[kind] ? (
        <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinejoin="round" strokeLinecap="round">
          {paths[kind]}
        </svg>
      ) : (
        slot
      )}
      {slot.includes("Swap") && <span className="slot-swap">2</span>}
    </span>
  );
}

/** PoB's colours in text: ^1 to ^9 for its palette, ^xRRGGBB for any colour. */
const POB_COLOURS = ["#000000", "#ff0000", "#00ff00", "#0000ff", "#ffff00", "#ff00ff", "#00ffff", "#ffffff", "#b3b3b3", "#666666"];

function PobText({ text }: { text: string }) {
  const parts: { colour: string | null; text: string }[] = [];
  let colour: string | null = null;
  const re = /\^(x[0-9a-fA-F]{6}|[0-9])/g;
  let at = 0;
  for (let m = re.exec(text); m; m = re.exec(text)) {
    if (m.index > at) parts.push({ colour, text: text.slice(at, m.index) });
    colour = m[1].startsWith("x") ? `#${m[1].slice(1)}` : POB_COLOURS[Number(m[1])];
    // ^7 is PoB's default text colour.
    if (m[1] === "7") colour = null;
    at = m.index + m[0].length;
  }
  if (at < text.length) parts.push({ colour, text: text.slice(at) });
  return (
    <>
      {parts.map((p, i) => (
        <span key={i} style={p.colour ? { color: p.colour } : undefined}>
          {p.text}
        </span>
      ))}
    </>
  );
}

/* Tree */

function NodeList({ title, names, kind }: { title: string; names: string[]; kind: string }) {
  if (names.length === 0) return null;
  return (
    <div className="node-list">
      <span className="node-list-title muted small">{title}</span>
      <div className="chips">
        {names.map((n, i) => (
          <span key={i} className={`chip static node-${kind}`}>
            {n}
          </span>
        ))}
      </div>
    </div>
  );
}
