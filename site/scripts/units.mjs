// Turns the game's unit data (data/factions/*/units/*.ron, lore.ron, codex.ron)
// into the JSON the site reads (content/units/*.json, content/factions.json).
// The game data stays the source of truth: run `pnpm sync` after it changes
// and commit the JSON, so the site builds on its own.
import { readFileSync, writeFileSync, mkdirSync, readdirSync, rmSync, existsSync } from "node:fs";
import { join, dirname, basename } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repo = join(here, "..", "..");
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

const FACTIONS = {
  aster: { slug: "arc", short: "ARC" },
  regency: { slug: "regency", short: "Regency" },
};

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

const round = (n, d = 1) => (typeof n === "number" ? Math.round(n * 10 ** d) / 10 ** d : null);

function weaponOf(w, loreWeapons, factionKey) {
  const dps = w.reload ? round((w.damage * (w.salvo ?? 1)) / w.reload) : null;
  return {
    name: w.name,
    damage: w.damage ?? 0,
    range: w.range ?? 0,
    reload: w.reload ?? null,
    dps,
    splash: w.splash ?? 0,
    targets: w.targets ?? [],
    kind: w.flak
      ? "flak"
      : w.rail
        ? "rail"
        : w.bore
          ? "bore"
          : w.hitscan
            ? "beam"
            : w.discharge
              ? "charged"
              : w.plasma
                ? factionKey === "regency"
                  ? "plasma"
                  : "bolt"
                : w.torpedo
                  ? "torpedo"
                  : w.missile
                    ? "missile"
                    : w.trajectory === "Ballistic"
                      ? "ballistic"
                      : "direct",
    color: w.color ?? "Orange",
    lore: loreWeapons?.[w.name] ?? null,
  };
}

function groupWeapons(ws) {
  // Mounts of the same gun read as one row, "x2".
  const out = [];
  for (const w of ws) {
    const same = out.find((o) => o.name === w.name && o.damage === w.damage && o.range === w.range);
    if (same) same.count++;
    else out.push({ ...w, count: 1 });
  }
  return out;
}

function unitOf(u, factionKey, lore) {
  const l = lore[u.key] ?? {};
  const weapons = groupWeapons((u.weapons ?? []).map((w) => weaponOf(w, l.weapons, factionKey)));
  const dps = round(weapons.reduce((s, w) => s + (w.dps ?? 0) * w.count, 0));
  return {
    key: u.key,
    slug: u.key.replace(/^(aster|regency)_/, "").replace(/_/g, "-"),
    faction: FACTIONS[factionKey].slug,
    name: u.name,
    role: u.role,
    tech: u.tech,
    domain: domainOf(u),
    categories: u.categories ?? [],
    lore: l.lore ?? null,
    stats: {
      health: u.health ?? 0,
      shield: u.shield?.health ?? null,
      mass: u.cost?.mass ?? 0,
      energy: u.cost?.energy ?? 0,
      buildTime: u.cost?.time ?? 0,
      speed: u.motion?.speed ?? null,
      vision: u.vision ?? null,
      radar: u.radar ?? null,
      sonar: u.sonar ?? null,
      range: weapons.length ? Math.max(...weapons.map((w) => w.range)) : null,
      dps: dps || null,
      buildPower: u.builder?.power ?? null,
    },
    weapons,
    builds: u.builder?.builds ?? [],
    upgradesTo: u.upgrades_to ?? null,
    mesh: typeof u.mesh === "string" ? u.mesh : null,
    icon: typeof u.icon === "string" ? u.icon : null,
  };
}

// ---- Run ---------------------------------------------------------------------

const unitsDir = join(out, "units");
if (existsSync(unitsDir)) rmSync(unitsDir, { recursive: true });
mkdirSync(unitsDir, { recursive: true });

const factions = [];
const index = [];
for (const [factionKey, meta] of Object.entries(FACTIONS)) {
  const dir = join(repo, "data", "factions", factionKey);
  const faction = readRon(join(dir, "faction.ron"));
  const codex = readRon(join(dir, "codex.ron"));
  const lorePath = join(dir, "lore.ron");
  const lore = existsSync(lorePath) ? readRon(lorePath) : {};
  const all = [];
  for (const f of readdirSync(join(dir, "units")).sort()) {
    if (!f.endsWith(".ron") || f === "survival.ron") continue;
    for (const u of readRon(join(dir, "units", f))) {
      // Spent casings are scrap props, not units anyone builds.
      if (!u.key || !u.name || u.role === "Scrap") continue;
      all.push(unitOf(u, factionKey, lore));
    }
  }
  // Names that are placeholders for another faction's roster are left out.
  for (const unit of all) {
    unit.builds = unit.builds.filter((k) => all.some((o) => o.key === k));
    if (unit.upgradesTo && !all.some((o) => o.key === unit.upgradesTo)) unit.upgradesTo = null;
    writeFileSync(join(unitsDir, `${unit.slug}.${unit.faction}.json`), JSON.stringify(unit, null, 2) + "\n");
    index.push({
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
    });
  }
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
  });
  console.log(`${meta.short}: ${all.length} units`);
}

const order = { command: 0, land: 1, air: 2, naval: 3, space: 4, experimental: 5, structure: 6 };
index.sort(
  (a, b) =>
    a.faction.localeCompare(b.faction) || order[a.domain] - order[b.domain] || a.tech - b.tech || a.name.localeCompare(b.name),
);
writeFileSync(join(out, "units.index.json"), JSON.stringify(index, null, 2) + "\n");
writeFileSync(join(out, "factions.json"), JSON.stringify(factions, null, 2) + "\n");
console.log(`wrote ${index.length} units to ${basename(out)}/`);
