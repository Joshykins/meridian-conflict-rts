// The unit directory's data: JSON written by scripts/sync-units.mjs from the
// game's own unit files. Read at build time only; every page is static.
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import indexJson from "../../content/units.index.json";
import factionsJson from "../../content/factions.json";

import type { Faction, Unit, UnitSummary } from "./shared";

export * from "./shared";

export const units = indexJson as UnitSummary[];
export const factions = factionsJson as Faction[];

export const factionOf = (slug: string) => factions.find((f) => f.slug === slug);

export async function getUnit(faction: string, slug: string): Promise<Unit | null> {
  if (!units.some((u) => u.faction === faction && u.slug === slug)) return null;
  const file = join(process.cwd(), "content", "units", `${slug}.${faction}.json`);
  return JSON.parse(await readFile(file, "utf8")) as Unit;
}

/** The game key (`aster_t1_tank`) back to a directory entry. */
export function byKey(key: string): UnitSummary | undefined {
  const faction = key.startsWith("naga_") ? "regency" : "arc";
  const slug = key.replace(/^(aster|naga)_/, "").replace(/_/g, "-");
  return units.find((u) => u.faction === faction && u.slug === slug);
}

let all: Promise<Unit[]> | null = null;
/** Every unit's full record, read once per build. */
export function allUnits(): Promise<Unit[]> {
  all ??= Promise.all(units.map((u) => getUnit(u.faction, u.slug))).then((l) => l.filter((u): u is Unit => !!u));
  return all;
}

/** Who can build this unit, and what it was upgraded from. */
export async function lineage(unit: Unit) {
  const everyone = await allUnits();
  const builtBy = everyone.filter((u) => u.builds.includes(unit.key));
  const from = everyone.find((u) => u.upgradesTo === unit.key) ?? null;
  return { builtBy, from };
}
