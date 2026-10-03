// Turns the game's unit data (data/factions/*/units/*.ron, lore.ron, codex.ron,
// faction.ron) into the JSON the site reads (content/units/*.json, units.index.json,
// factions.json). The game data is the only source: the JSON is written afresh by
// `scripts/generate.mjs` before every dev start and build, and is not checked in.
import { readFileSync, writeFileSync, mkdirSync, readdirSync, rmSync, existsSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
export const repo = join(here, "..", "..");
const out = join(here, "..", "content");

// ---- A small, tolerant RON reader ---------------------------------------

function parseRon(src) {
  let i = 0;
  const err = (m) => {
    const line = src.slice(0, i).split("\n").length;
    throw new Error(`${m} at line ${line}`);
  };
  const ws = () => {
    for (;;) {
      while (i < src.length && /\s/.test(src[i])) i++;
      if (src.startsWith("//", i)) {
        while (i < src.length && src[i] !== "\n") i++;
      } else if (src.startsWith("/*", i)) {
        const end = src.indexOf("*/", i + 2);
        i = end < 0 ? src.length : end + 2;
      } else return;
    }
  };
  const ident = () => {
    const m = /^[A-Za-z_][A-Za-z0-9_]*/.exec(src.slice(i, i + 128));
    if (!m) return null;
    i += m[0].length;
    return m[0];
  };
  const peekKey = () => {
    // `ident :` (not `::`) starts a struct field.
    const save = i;
    const id = ident();
    ws();
    const ok = id && src[i] === ":" && src[i + 1] !== ":";
    i = save;
    return ok;
  };
  const list = (close) => {
    const items = [];
    for (;;) {
      ws();
      if (src[i] === close) {
        i++;
        return items;
      }
      items.push(value());
      ws();
      if (src[i] === ",") i++;
      else if (src[i] !== close) err(`expected , or ${close}`);
    }
  };
  const fields = (close) => {
    const obj = {};
    for (;;) {
      ws();
      if (src[i] === close) {
        i++;
        return obj;
      }
      let key;
      if (src[i] === '"') key = string();
      else key = ident();
      if (key == null) err("expected a field name");
      ws();
      if (src[i] !== ":") err("expected :");
      i++;
      obj[key] = value();
      ws();
      if (src[i] === ",") i++;
      else if (src[i] !== close) err(`expected , or ${close}`);
    }
  };
  const paren = () => {
    i++; // (
    ws();
    if (src[i] === ")") {
      i++;
      return [];
    }
    return peekKey() ? fields(")") : list(")");
  };
  const string = () => {
    i++;
    let s = "";
    while (src[i] !== '"') {
      if (i >= src.length) err("unterminated string");
      if (src[i] === "\\") {
        const c = src[i + 1];
        s += c === "n" ? "\n" : c === "t" ? "\t" : c;
        i += 2;
      } else s += src[i++];
    }
    i++;
    return s;
  };
  const value = () => {
    ws();
    const c = src[i];
    if (c === "(") return paren();
    if (c === "[") {
      i++;
      return list("]");
    }
    if (c === "{") {
      i++;
      return fields("}");
    }
    if (c === '"') return string();
    if (/[-+0-9.]/.test(c)) {
      const m = /^[-+]?(\d[\d_]*)?(\.\d*)?([eE][-+]?\d+)?/.exec(src.slice(i, i + 64));
      i += m[0].length;
      return Number(m[0].replace(/_/g, ""));
    }
    const id = ident();
    if (id == null) err(`unexpected ${JSON.stringify(c)}`);
    if (id === "true") return true;
    if (id === "false") return false;
    if (id === "None") return null;
    ws();
    if (src[i] === "(") {
      const inner = paren();
      if (id === "Some") return Array.isArray(inner) && inner.length === 1 ? inner[0] : inner;
      if (Array.isArray(inner)) return inner.length === 1 ? { $: id, value: inner[0] } : { $: id, args: inner };
      return { $: id, ...inner };
    }
    return id;
  };
  const v = value();
  ws();
  return v;
}

const readRon = (p) => parseRon(readFileSync(p, "utf8"));

// ---- Shaping ---------------------------------------------------------------
//
// The figures follow the game's own rules, so the site reads as the build card
// does. Where a rule lives in the engine, the comment names it.

const FACTIONS = {
  aster: { slug: "arc", short: "ARC" },
  regency: { slug: "regency", short: "Regency" },
};

/** Seconds to sim ticks, ten a second (`mc_core::TICKS_PER_SECOND`, `raw.rs` `ticks`). */
const ticks = (seconds) => Math.max(0, Math.round((seconds ?? 0) * 10));
const round = (n, d = 1) => (typeof n === "number" ? Math.round(n * 10 ** d) / 10 ** d : null);
/** A figure the data leaves at zero is one the unit does not have. */
const some = (n) => (typeof n === "number" && n !== 0 ? n : null);
const slugOf = (key) => key.replace(/^(aster|regency)_/, "").replace(/_/g, "-");

function domainOf(u) {
  const layer = u.motion?.layer;
  const cats = u.categories ?? [];
  if (!u.motion) return "structure";
  if (cats.includes("Experimental")) return "experimental";
  if (layer === "Air" || layer === "Space" || cats.includes("Space")) return cats.includes("Space") ? "space" : "air";
  if (layer === "Water" || layer === "Sub" || layer === "Naval" || cats.includes("Naval")) return "naval";
  if (u.key.endsWith("commander") || cats.includes("Commander")) return "command";
  return "land";
}

/** What kind of gun it is, in the order the build card decides it (`hud/armament.rs` `kind`). */
function kindOf(w, factionKey) {
  if (w.intercepts) return "interceptor";
  if (w.torpedo) return "torpedo";
  if (w.bore) return "bore";
  if (w.missile && w.guided) return "guided";
  if (w.missile) return "rocket";
  if (w.plasma_grade) return "plasma";
  if (w.hitscan) return "beam";
  if (w.rail) return "rail";
  if (w.flak) return "flak";
  if (w.discharge) return "charged";
  if (w.plasma && factionKey !== "regency") return "bolt";
  return w.trajectory === "Ballistic" ? "ballistic" : "direct";
}

function weaponOf(w, loreWeapons, factionKey) {
  // `hud/selection.rs` `weapon_dps`: a salvo's damage over the reload, in whole ticks.
  const reload = Math.max(1, ticks(w.reload)) / 10;
  const salvo = Math.max(1, w.salvo ?? 1);
  const intercepts = !!w.intercepts;
  return {
    name: w.name,
    kind: kindOf(w, factionKey),
    plasmaGrade: w.plasma_grade ?? null,
    damage: w.damage ?? 0,
    salvo,
    salvoBatch: Math.max(1, w.salvo_batch ?? 1),
    salvoDelay: ticks(w.salvo_delay) / 10,
    reload,
    // Interceptors shoot torpedoes, not units: they are no part of the firepower.
    dps: intercepts ? null : round((w.damage * salvo) / reload),
    splash: w.splash ?? 0,
    rangeMin: w.range_min ?? 0,
    range: w.range ?? 0,
    // A hitscan shot crosses its range in the tick it is fired.
    speed: w.hitscan ? null : some(w.speed),
    trajectory: w.trajectory,
    flatFire: !!w.flat_fire,
    loft: w.loft ?? 0,
    targets: w.targets ?? [],
    prefer: w.prefer ?? [],
    // Degrees a second; zero is a gun fixed to the hull.
    turretTurn: w.turret_turn ?? 0,
    arc: w.arc ?? 360,
    facing: w.rear ? 180 : (w.facing ?? 0),
    spread: w.spread ?? 0,
    sweep: w.sweep ?? 0,
    depression: w.depression ?? 0,
    keepsAim: !!w.keeps_aim,
    slant: !!w.slant,
    hitscan: !!w.hitscan,
    beam: !!w.beam,
    rail: !!w.rail,
    flak: !!w.flak,
    missile: !!w.missile,
    guided: !!w.guided,
    verticalLaunch: !!w.vertical_launch,
    coldLaunch: w.cold_launch ?? 0,
    hatch: w.hatch ?? 0,
    split: !!w.split,
    skim: w.skim ?? 0,
    apogee: w.apogee ?? 0,
    // `Weapon::casing_hp`: what an intercept laser has to burn through. A missile that
    // does not say is a light casing; a shell can be taken only when it says how much.
    casing: w.intercept > 0 ? w.intercept : w.missile ? 10 : 0,
    proximity: w.proximity ?? 0,
    burn: w.burn > 0 ? { seconds: w.burn, dps: w.burn_dps ?? 30 } : null,
    spinUp: w.spin_up ?? 0,
    spinRamp: w.spin_ramp ?? 0,
    torpedo: !!w.torpedo,
    intercepts,
    surfaced: !!w.surfaced,
    bore: w.bore
      ? {
          width: w.bore.width ?? 0,
          damage: w.bore.damage ?? 0,
          storm: w.bore.storm ? { radius: w.bore.storm.radius, seconds: w.bore.storm.seconds, dps: w.bore.storm.damage } : null,
        }
      : null,
    sabot: w.sabot ? { damage: w.sabot.damage, splash: w.sabot.splash } : null,
    // `weapon.rs` `BOMBARD_RADIUS`: only a gun given more than the usual circle says so.
    bombard: w.bombard > 250 ? w.bombard : 0,
    color: w.color ?? "Orange",
    lore: loreWeapons?.[w.name] ?? null,
  };
}

// Mounts that differ only in where they sit and what they are called read as one
// row, "Battery x3" (`hud/armament.rs` `same`, `shared_name`, `groups`).
const SAME = [
  "damage",
  "splash",
  "rangeMin",
  "range",
  "reload",
  "salvo",
  "salvoBatch",
  "speed",
  "trajectory",
  "missile",
  "guided",
  "torpedo",
  "intercepts",
  "hitscan",
];
const same = (a, b) => SAME.every((k) => a[k] === b[k]) && a.targets.join() === b.targets.join();

function sharedName(names) {
  if (names.length === 1) return names[0];
  const words = names.map((n) => n.split(/\s+/));
  const shortest = Math.min(...words.map((w) => w.length));
  let tail = 0;
  while (tail < shortest && words.every((w) => w[w.length - 1 - tail] === words[0][words[0].length - 1 - tail])) tail++;
  if (tail) return words[0].slice(words[0].length - tail).join(" ");
  let head = 0;
  while (head < shortest && words.every((w) => w[head] === words[0][head])) head++;
  return head ? words[0].slice(0, head).join(" ") : names[0];
}

function groupWeapons(ws) {
  const sets = [];
  for (const w of ws) {
    const set = sets.find((s) => same(s[0], w));
    if (set) set.push(w);
    else sets.push([w]);
  }
  return sets
    .map((s) => ({
      ...s[0],
      name: sharedName(s.map((w) => w.name)),
      mounts: s.map((w) => w.name),
      // Where each mount's arc is centred: a broadside's are to port and to starboard.
      facings: [...new Set(s.map((w) => w.facing))],
      count: s.length,
      // The first mount's text stands for the group when they share it or it is alone.
      lore: s.find((w) => w.lore)?.lore ?? null,
    }))
    .sort((a, b) => (b.dps ?? 0) * b.count - (a.dps ?? 0) * a.count);
}

/** Firepower against each domain, as the build card sums it (`hud/armament.rs` `DOMAINS`). */
function firepower(weapons) {
  const against = (...cats) =>
    round(weapons.filter((w) => w.targets.some((t) => cats.includes(t))).reduce((s, w) => s + (w.dps ?? 0) * w.count, 0)) || null;
  return {
    total: round(weapons.reduce((s, w) => s + (w.dps ?? 0) * w.count, 0)) || null,
    land: against("Land", "Structure"),
    naval: against("Naval"),
    air: against("Air"),
  };
}

const economyOf = (e = {}) => ({
  massIncome: e.mass_income ?? 0,
  energyIncome: e.energy_income ?? 0,
  energyUpkeep: e.energy_upkeep ?? 0,
  massStorage: e.mass_storage ?? 0,
  energyStorage: e.energy_storage ?? 0,
});

const shieldOf = (s) => (s ? { kind: s.kind ?? "Dome", radius: some(s.radius), health: s.health, regen: s.regen } : null);

/** `raw.rs` `default_wreck`: the share of its mass a wreck keeps, more the higher the tier. */
const wreckShare = (u) => u.wreck_fraction ?? (u.tech <= 1 ? 0.81 : u.tech === 2 ? 0.85 : 0.9);

/** `UnitBlueprint::cargo_room`: the room a land unit takes in a lift ship's hold. */
function cargoRoom(u) {
  const m = u.motion;
  if (!m || u.transport || !["Land", "Amphibious", "Hover"].includes(m.layer)) return null;
  return (u.categories ?? []).includes("Commander") ? 8 : m.size + 1;
}

function refitsOf(u, loreWeapons, factionKey) {
  return (u.refits ?? []).map((slot) => ({
    name: slot.name,
    modules: slot.modules.map((m) => ({
      name: m.name,
      summary: m.summary || null,
      // The module it goes on over: its next tier. Every other module in the slot is an alternative.
      after: m.after ? (slot.modules.find((o) => o.key === m.after)?.name ?? null) : null,
      tech: some(m.tech),
      cost: { mass: m.cost.mass, energy: m.cost.energy, time: m.cost.time },
      health: m.health ?? 0,
      regen: m.regen ?? 0,
      vision: m.vision ?? 0,
      buildPower: m.build_power ?? 0,
      buildRange: m.build_range ?? 0,
      builds: m.builds ?? [],
      economy: economyOf(m.economy),
      shield: shieldOf(m.shield),
      drones: m.drone ? { key: m.drone, count: (m.drone_sockets ?? []).length, radius: m.drone_radius ?? 0 } : null,
      weapons: groupWeapons((m.weapons ?? []).map((w) => weaponOf(w, loreWeapons, factionKey))),
    })),
  }));
}

function unitOf(u, factionKey, lore) {
  const l = lore[u.key] ?? {};
  const weapons = groupWeapons((u.weapons ?? []).map((w) => weaponOf(w, l.weapons, factionKey)));
  const power = firepower(weapons);
  const m = u.motion;
  const share = wreckShare(u);
  return {
    key: u.key,
    slug: slugOf(u.key),
    faction: FACTIONS[factionKey].slug,
    name: u.name,
    title: u.title ?? null,
    role: u.role,
    tech: u.tech,
    domain: domainOf(u),
    categories: u.categories ?? [],
    lore: l.lore ?? null,
    // The headline figures, one per bar on the unit page.
    stats: {
      health: u.health ?? 0,
      shield: u.shield?.health ?? null,
      mass: u.cost?.mass ?? 0,
      energy: u.cost?.energy ?? 0,
      buildTime: u.cost?.time ?? 0,
      speed: m?.speed ?? null,
      vision: u.vision ?? null,
      radar: some(u.radar),
      sonar: some(u.sonar),
      range: weapons.length ? Math.max(...weapons.map((w) => w.range)) : null,
      dps: power.total,
      buildPower: u.builder?.power ?? null,
    },
    firepower: power,
    hull: {
      regen: u.regen ?? 0,
      radius: u.radius,
      height: u.height,
      footprint: u.footprint ?? null,
      wreckShare: share,
      wreckMass: round((u.cost?.mass ?? 0) * share, 0),
      cargoRoom: cargoRoom(u),
      waterBuild: !!u.water_build,
      seabed: !!u.seabed,
    },
    motion: m
      ? {
          layer: m.layer,
          hover: !!m.hover,
          speed: m.speed,
          accel: m.accel,
          turn: m.turn,
          altitude: some(m.altitude),
          deploy: some(m.deploy),
          broadside: some(m.broadside),
          stride: !!m.stride,
          orbit: u.orbit ?? null,
        }
      : null,
    dive: u.dive ? { depth: u.dive.depth, time: u.dive.time } : null,
    economy: economyOf(u.economy),
    mine: u.mine
      ? {
          reach: u.mine.reach,
          seaReach: u.mine.sea_reach,
          ground: u.mine.ground,
          ore: u.mine.per_hectare,
          base: u.mine.base ?? 0,
        }
      : null,
    builder: u.builder ? { power: u.builder.power, range: u.builder.range } : null,
    reclaimer: u.reclaimer
      ? {
          power: u.reclaimer.power,
          range: u.reclaimer.range,
          charge: u.reclaimer.charge ?? 0,
          mobile: !!u.reclaimer.mobile,
          heads: u.reclaimer.heads.length,
        }
      : null,
    drones: u.drone ? { key: u.drone, count: (u.drone_sockets ?? []).length, radius: u.drone_radius ?? 0 } : null,
    shield: shieldOf(u.shield),
    // `anti_missile` is the lasers' reach; they burn `anti_missile_lasers` missiles at once (at least one).
    antiMissile: u.anti_missile ? { range: u.anti_missile, lasers: Math.max(1, u.anti_missile_lasers ?? 1) } : null,
    transport: u.transport ? { capacity: u.transport.capacity, descent: u.transport.descent, unload: u.transport.unload } : null,
    warp: u.warp ? { perKm: u.warp.per_km, spool: u.warp.spool, cooldown: u.warp.cooldown, speed: u.warp.speed } : null,
    warpDamper: u.warp_damper
      ? { radius: u.warp_damper.radius, drag: u.warp_damper.drag, damage: u.warp_damper.damage, stun: u.warp_damper.stun }
      : null,
    strategic: u.strategic
      ? {
          kind: u.strategic.kind,
          round: { mass: u.strategic.missile.mass, energy: u.strategic.missile.energy, time: u.strategic.missile.time },
          power: u.strategic.power,
          stock: u.strategic.stock,
          speed: u.strategic.speed,
          apogee: some(u.strategic.apogee),
          coverage: some(u.strategic.coverage),
          blast: u.strategic.blast
            ? {
                radius: u.strategic.blast.radius,
                core: u.strategic.blast.core,
                damage: u.strategic.blast.damage,
                edge: u.strategic.blast.edge,
                front: u.strategic.blast.front,
              }
            : null,
        }
      : null,
    deathBlast: u.death_blast ? { radius: u.death_blast.radius, damage: u.death_blast.damage } : null,
    stomp: u.stomp ? { radius: u.stomp.radius, damage: u.stomp.damage, pace: u.stomp.pace } : null,
    weapons,
    refits: refitsOf(u, l.weapons, factionKey),
    builds: u.builder?.builds ?? [],
    upgradesTo: u.upgrades_to ?? null,
    // Set below, once every unit of the faction is read.
    upgradeCost: null,
    mesh: typeof u.mesh === "string" ? u.mesh : null,
    icon: typeof u.icon === "string" ? u.icon : null,
    // Its mesh and portrait (`scripts/models.mjs`), set as the content is written.
    art: null,
  };
}

// ---- Run ---------------------------------------------------------------------

/**
 * The game's player colours, the first eight, linear RGB: read out of its source
 * (`mc-game/src/setup.rs` `TEAM_COLORS`), so the site never keeps a list of its own.
 */
function teamColors() {
  const path = join(repo, "crates", "mc-game", "src", "setup.rs");
  const list = /pub const TEAM_COLORS: Palette = \[([\s\S]*?)\n\];/.exec(readFileSync(path, "utf8"));
  const colors = [...(list?.[1] ?? "").matchAll(/\[([\d.]+), ([\d.]+), ([\d.]+)\]/g)].map((m) => m.slice(1, 4).map(Number));
  if (colors.length < 8) throw new Error(`${path}: TEAM_COLORS was not found as a list of [r, g, b]`);
  return colors.slice(0, 8);
}

// The owner's colour a faction's units wear on the site: the first two player colours.
const TEAM_SLOT = { aster: 0, regency: 1 };

/** A faction's paint, for the model exporter and the page's viewer (`mc_models::site::Paint`). */
function paintOf(faction, factionKey, teams) {
  return {
    plating: faction.plating_color,
    accent: faction.accent_color,
    glow: faction.highlight_color,
    // `raw.rs` `default_shield_color`.
    shield: faction.shield_color ?? [0.35, 0.75, 1.0],
    team: teams[TEAM_SLOT[factionKey]],
    // Regency machinery is dark bronze, not gunmetal (`regency.wgsl`).
    bronze: factionKey === "regency",
  };
}

/**
 * Reads the game data and shapes it. `art(unit)` gives each unit's pictures (its mesh
 * and portrait, `scripts/models.mjs`), or null while there are none.
 */
export function readUnits() {
  const teams = teamColors();
  const factions = [];
  const units = [];
  for (const [factionKey, meta] of Object.entries(FACTIONS)) {
    const dir = join(repo, "data", "factions", factionKey);
    const faction = readRon(join(dir, "faction.ron"));
    const codex = readRon(join(dir, "codex.ron"));
    const lorePath = join(dir, "lore.ron");
    const lore = existsSync(lorePath) ? readRon(lorePath) : {};
    const all = [];
    // Units no one builds or picks: a salvage drone flies for its carrier, never on its own.
    const carried = new Set();
    const raw = [];
    for (const f of readdirSync(join(dir, "units")).sort()) {
      if (!f.endsWith(".ron") || f === "survival.ron") continue;
      for (const u of readRon(join(dir, "units", f))) {
        // Spent casings are scrap props, not units anyone builds.
        if (!u.key || !u.name || u.role === "Scrap") continue;
        raw.push(u);
        if (u.drone) carried.add(u.drone);
        for (const slot of u.refits ?? []) for (const m of slot.modules) if (m.drone) carried.add(m.drone);
      }
    }
    for (const u of raw) all.push(unitOf(u, factionKey, lore));
    const known = (k) => all.some((o) => o.key === k);
    for (const unit of all) {
      // Names that are placeholders for another faction's roster are left out.
      unit.builds = unit.builds.filter(known);
      for (const slot of unit.refits) for (const m of slot.modules) m.builds = m.builds.filter(known);
      if (unit.upgradesTo && !known(unit.upgradesTo)) unit.upgradesTo = null;
      unit.carried = carried.has(unit.key);
      // `Blueprints::upgrade_cost`: an upgrade is paid only what the new tier costs over the one it is.
      const from = all.find((o) => o.upgradesTo === unit.key);
      if (from) {
        unit.upgradeCost = {
          mass: Math.max(0, unit.stats.mass - from.stats.mass),
          energy: Math.max(0, unit.stats.energy - from.stats.energy),
        };
      }
    }
    units.push(...all);
    factions.push({
      slug: meta.slug,
      key: faction.key,
      name: faction.name,
      short: faction.abbreviation ?? meta.short,
      motto: codex.motto || null,
      meaning: codex.meaning,
      about: codex.about,
      field: codex.field,
      crest: (codex.crest ?? []).map(([title, text]) => ({ title, text })),
      units: all.length,
      paint: paintOf(faction, factionKey, teams),
    });
  }
  return { factions, units, teams };
}

const ORDER = { command: 0, land: 1, air: 2, naval: 3, space: 4, experimental: 5, structure: 6 };

/** Writes `content/` from what `readUnits` gave, each unit with its pictures from `art`. */
export function writeContent({ factions, units, teams }, art) {
  const unitsDir = join(out, "units");
  if (existsSync(unitsDir)) rmSync(unitsDir, { recursive: true });
  mkdirSync(unitsDir, { recursive: true });
  const index = [];
  for (const unit of units) {
    unit.art = art(unit);
    writeFileSync(join(unitsDir, `${unit.slug}.${unit.faction}.json`), JSON.stringify(unit, null, 2) + "\n");
    index.push({
      key: unit.key,
      slug: unit.slug,
      faction: unit.faction,
      name: unit.name,
      role: unit.role,
      tech: unit.tech,
      domain: unit.domain,
      icon: unit.icon,
      health: unit.stats.health,
      mass: unit.stats.mass,
      dps: unit.stats.dps,
      range: unit.stats.range,
      lore: unit.lore,
      portrait: unit.art?.portrait ?? null,
    });
  }
  index.sort(
    (a, b) =>
      a.faction.localeCompare(b.faction) || ORDER[a.domain] - ORDER[b.domain] || a.tech - b.tech || a.name.localeCompare(b.name),
  );
  writeFileSync(join(out, "units.index.json"), JSON.stringify(index, null, 2) + "\n");
  writeFileSync(join(out, "factions.json"), JSON.stringify(factions, null, 2) + "\n");
  writeFileSync(join(out, "teams.json"), JSON.stringify(teams) + "\n");
  return index.length;
}
