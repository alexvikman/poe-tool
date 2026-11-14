import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { buildView, calcLoadout, importBuild, removeBuild, type BuildView, type GearItem, type GemGroup, type Loadout } from "./builds";
import { getSettings, setSettings, type Settings } from "./townMute";
import { listGroups, type HitItem, type SearchGroup } from "./searches";
import { ItemPicture, ItemTooltip } from "./ItemView";

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

/** Builds from Path of Building, shown like pobb.in shows them. A build is the same
 *  build as in Searches: its searches belong to it. */
export function BuildsPage({ onOpenSearches }: { onOpenSearches: (groupId: string) => void }) {
  const [groups, setGroups] = useState<SearchGroup[]>([]);
  const [selected, setSelected] = useState<string | null>(remembered);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // A saved service address reopens the build, so it's used.
  const [serviceSaves, setServiceSaves] = useState(0);

  const reload = () => listGroups().then(setGroups, (e) => setError(String(e)));
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
      const id = await importBuild(input, null);
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

  const group = groups.find((g) => g.id === selected) ?? groups.find((g) => g.build) ?? groups[0];

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
          {groups.map((g) => (
            <button
              key={g.id}
              className={`build-item ${group?.id === g.id ? "active" : ""}`}
              onClick={() => select(g.id)}
            >
              <span className="build-name">{g.name}</span>
              <span className="build-meta">
                {!g.build && <span className="muted small">no PoB</span>}
                {g.searches.length > 0 && <span className="muted">{g.searches.length}</span>}
              </span>
            </button>
          ))}
          {groups.length === 0 && <p className="muted small builds-empty">Import a build to start.</p>}
        </div>
        <PobServiceSettings onSaved={() => setServiceSaves((n) => n + 1)} />
      </aside>
      <section className="searches-main" key={`${group?.id ?? "none"}-${serviceSaves}`}>
        {group &&
          (group.build ? (
            <Build group={group} onChanged={reload} onOpenSearches={() => onOpenSearches(group.id)} />
          ) : (
            <ImportInto group={group} onDone={reload} onOpenSearches={() => onOpenSearches(group.id)} />
          ))}
      </section>
    </div>
  );
}

/** A build from Searches without a PoB build yet. */
function ImportInto({
  group,
  onDone,
  onOpenSearches,
}: {
  group: SearchGroup;
  onDone: () => void;
  onOpenSearches: () => void;
}) {
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function go() {
    setBusy(true);
    try {
      await importBuild(input, group.id);
      onDone();
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  }

  return (
    <>
      <header className="main-head">
        <div>
          <h1>{group.name}</h1>
          <p className="lede">
            No Path of Building build here yet. Import one to see its gear, gems, tree and stats beside its{" "}
            <button className="link-btn" onClick={onOpenSearches}>
              {group.searches.length} searches
            </button>
            .
          </p>
        </div>
      </header>
      <form
        className="inline-form"
        onSubmit={(e) => {
          e.preventDefault();
          if (input.trim()) go();
        }}
      >
        <input type="text" placeholder="pobb.in link or PoB code" value={input} onChange={(e) => setInput(e.target.value)} />
        <button className="btn primary" type="submit" disabled={!input.trim() || busy}>
          {busy ? "Importing…" : "Import"}
        </button>
      </form>
      {error && <p className="bad small">{error}</p>}
    </>
  );
}

function Build({
  group,
  onChanged,
  onOpenSearches,
}: {
  group: SearchGroup;
  onChanged: () => void;
  onOpenSearches: () => void;
}) {
  const [view, setView] = useState<BuildView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  // Which sets are shown: the exported loadout at first.
  const [sel, setSel] = useState<Loadout | null>(null);
  // Stats PoB calculated for other loadouts, by their sets.
  const [calcs, setCalcs] = useState<Record<string, Calc>>({});

  // Another loadout: ask PoB for its stats (once per combination of sets).
  useEffect(() => {
    if (!view || !sel) return;
    const a = view.active;
    if (sel.itemSet === a.itemSet && sel.skillSet === a.skillSet && sel.tree === a.tree && sel.config === a.config) return;
    const key = `${sel.itemSet}-${sel.skillSet}-${sel.tree}-${sel.config}`;
    if (calcs[key]) return;
    setCalcs((c) => ({ ...c, [key]: { busy: true } }));
    calcLoadout(group.id, sel).then(
      (r) => setCalcs((c) => ({ ...c, [key]: { busy: false, stats: r.stats, version: r.pobVersion } })),
      (e) => setCalcs((c) => ({ ...c, [key]: { busy: false, error: String(e) } })),
    );
  }, [view, sel]);

  useEffect(() => {
    buildView(group.id).then(
      (v) => {
        setView(v);
        setSel(v.active);
      },
      (e) => setError(String(e)),
    );
  }, [group.id, group.build?.code]);

  if (error) return <p className="bad">{error}</p>;
  if (!view || !sel) return <p className="muted">Reading the build…</p>;

  const gear = view.itemSets[sel.itemSet]?.gear ?? [];
  const tree = view.trees[sel.tree];
  const groups = view.skillSets[sel.skillSet]?.groups ?? [];
  const config = view.configs[sel.config]?.config ?? [];
  const loadoutAt = view.loadouts.findIndex(
    (l) => l.itemSet === sel.itemSet && l.skillSet === sel.skillSet && l.tree === sel.tree && l.config === sel.config,
  );
  const exported =
    sel.itemSet === view.active.itemSet &&
    sel.skillSet === view.active.skillSet &&
    sel.tree === view.active.tree &&
    sel.config === view.active.config;
  const key = `${sel.itemSet}-${sel.skillSet}-${sel.tree}-${sel.config}`;
  const calc = exported ? undefined : calcs[key];
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
        <BuildStats stats={stats} view={view} config={config} />
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
          <button className="btn small-btn" onClick={onOpenSearches}>
            Searches ({group.searches.length})
          </button>
          <button
            className="btn small-btn"
            onClick={() => confirm(`Take the PoB build off ${group.name}? Its searches stay.`) && removeBuild(group.id).then(onChanged)}
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

/** Stats PoB calculated for one combination of sets. */
interface Calc {
  busy: boolean;
  stats?: Record<string, number>;
  version?: string | null;
  error?: string;
}

/** Another Path of Building service than PoE Tool's own, such as a local copy for
 *  testing. */
function PobServiceSettings({ onSaved }: { onSaved: () => void }) {
  const [settings, setLocal] = useState<Settings | null>(null);
  const [url, setUrl] = useState("");
  const [key, setKey] = useState("");
  const [open, setOpen] = useState(false);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    getSettings().then((s) => {
      setLocal(s);
      setUrl(s.pobService?.url ?? "");
      setKey(s.pobService?.apiKey ?? "");
    });
  }, []);

  if (!settings) return null;
  const on = !!settings.pobService?.url?.trim();

  async function save() {
    const next = { ...settings!, pobService: { url: url.trim(), apiKey: key.trim() } };
    await setSettings(next);
    setLocal(next);
    onSaved();
    setSaved(true);
    setTimeout(() => setSaved(false), 1500);
  }

  return (
    <div className="pob-service">
      <button className="link-btn" onClick={() => setOpen((o) => !o)}>
        Path of Building service: {on ? settings.pobService.url : "PoE Tool's"}
      </button>
      {open && (
        <form
          onSubmit={(e) => {
            e.preventDefault();
            save();
          }}
        >
          <input type="text" placeholder="Empty: PoE Tool's own" value={url} onChange={(e) => setUrl(e.target.value)} />
          <input type="password" placeholder="Its key (if another)" value={key} onChange={(e) => setKey(e.target.value)} />
          <button className="btn small-btn" type="submit">
            {saved ? "Saved" : "Save"}
          </button>
          <p className="hint">
            Path of Building calculates a build's other loadouts on PoE Tool's service; the build is sent there. Point it
            at another copy here, for example one running locally.
          </p>
        </form>
      )}
    </div>
  );
}

/* Stats, as pobb.in lists them */

const num = (n: number, digits = 0) =>
  n.toLocaleString("en-US", { maximumFractionDigits: digits, minimumFractionDigits: 0 });

function BuildStats({ stats, view, config }: { stats: Record<string, number>; view: BuildView; config: string[] }) {
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
          // With life reserved (low life), what's left, as pobb.in shows it.
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
      {config.length > 0 && <div>{stat("Config", config.join(", "))}</div>}
      {(view.bandit || view.pantheon.length > 0) && (
        <div>
          {view.bandit && stat("Bandit", view.bandit)}
          {view.pantheon.length > 0 && stat("Pantheon", view.pantheon.join(", "))}
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
          <GearSlot key={g.slot} item={g.item} area={SLOT_AREA[g.slot]} right={g.slot === "Weapon 2" || g.slot === "Boots" || g.slot === "Ring 2"} />
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

/** An item in its slot; hovering shows its tooltip. */
function GearSlot({ item, area, right }: { item: HitItem; area?: string; right?: boolean }) {
  return (
    <div className={`gear-slot ${area ? "" : "loose"}`} style={area ? { gridArea: area } : undefined}>
      {item.icon ? (
        <ItemPicture item={item} />
      ) : (
        <div className={`gear-noicon rarity-${item.rarity}`} style={{ width: item.width * 47.4645, height: item.height * 47.4645 }}>
          {item.typeLine}
        </div>
      )}
      <div className={`gear-pop ${right ? "left" : ""}`}>
        <ItemTooltip item={item} />
      </div>
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
  return (
    <div className={`gem-group ${group.main ? "main" : ""} ${group.enabled ? "" : "off"}`}>
      {(group.label || group.slot) && (
        <div className="gem-group-head muted small">
          <span>{group.label ?? ""}</span>
          {group.slot && <SlotIcon slot={group.slot} />}
        </div>
      )}
      {actives.map((g, i) => (
        <div key={`a${i}`} className={`gem gem-${g.colour ?? "white"} ${g.enabled ? "" : "off"}`}>
          {g.name} <span className="gem-lvl">{g.level}/{g.quality}</span>
        </div>
      ))}
      {supports.map((g, i) => (
        <div key={`s${i}`} className={`gem support gem-${g.colour ?? "white"} ${g.enabled ? "" : "off"}`}>
          <span className="gem-branch">{i === supports.length - 1 ? "└" : "├"}</span>
          {g.name} <span className="gem-lvl">{g.level}/{g.quality}</span>
        </div>
      ))}
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
