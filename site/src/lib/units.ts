// The unit directory's data: JSON written by scripts/generate.mjs from the game's
// own unit files and models before every dev start and build. Read at build time
// only; every page is static.
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import indexJson from "../../content/units.index.json";
import factionsJson from "../../content/factions.json";
import teamsJson from "../../content/teams.json";

import type { Faction, Rgb, Unit, UnitSummary } from "./shared";

export * from "./shared";

export const units = indexJson as UnitSummary[];
export const factions = factionsJson as Faction[];
/** The game's player colours, linear RGB: what a unit can be shown wearing. */
export const teams = teamsJson as Rgb[];

export const factionOf = (slug: string) => factions.find((f) => f.slug === slug);

export async function getUnit(faction: string, slug: string): Promise<Unit | null> {
  if (!units.some((u) => u.faction === faction && u.slug === slug)) return null;
  const file = join(process.cwd(), "content", "units", `${slug}.${faction}.json`);
  return JSON.parse(await readFile(file, "utf8")) as Unit;
}

/** The game key (`aster_t1_tank`) back to a directory entry. */
export function byKey(key: string): UnitSummary | undefined {
  const faction = key.startsWith("regency_") ? "regency" : "arc";
  const slug = key.replace(/^(aster|regency)_/, "").replace(/_/g, "-");
  return units.find((u) => u.faction === faction && u.slug === slug);
}

let all: Promise<Unit[]> | null = null;
/** Every unit's full record, read once per build. */
export function allUnits(): Promise<Unit[]> {
  all ??= Promise.all(units.map((u) => getUnit(u.faction, u.slug))).then((l) => l.filter((u): u is Unit => !!u));
  return all;
}

/**
 * Who can build this unit, each with the build power it brings (a refit module that
 * adds the unit to what its carrier builds brings its carrier's power and its own),
 * and what it was upgraded from.
 */
export async function lineage(unit: Unit) {
  const everyone = await allUnits();
  const builtBy = everyone.filter(
    (u) => u.builds.includes(unit.key) || u.refits.some((s) => s.modules.some((m) => m.builds.includes(unit.key))),
  );
  const makers = builtBy.flatMap((u) => {
    const base = u.builder?.power ?? 0;
    if (u.builds.includes(unit.key)) return base ? [{ name: u.name, power: base }] : [];
    return u.refits
      .flatMap((s) => s.modules)
      .filter((m) => m.builds.includes(unit.key) && base + m.buildPower > 0)
      .map((m) => ({ name: `${u.name}, ${m.name}`, power: base + m.buildPower }));
  });
  const from = everyone.find((u) => u.upgradesTo === unit.key) ?? null;
  return { builtBy, makers, from };
}
