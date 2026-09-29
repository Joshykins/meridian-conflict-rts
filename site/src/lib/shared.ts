// Types and helpers safe for client components (no file system, no data).

export type Domain = "command" | "land" | "air" | "naval" | "space" | "experimental" | "structure";
export type FactionSlug = "arc" | "regency";

export type UnitSummary = {
  slug: string;
  faction: FactionSlug;
  name: string;
  role: string;
  tech: number;
  domain: Domain;
  icon: string | null;
  health: number;
  mass: number;
  dps: number | null;
  range: number | null;
  lore: string | null;
};

export type Weapon = {
  name: string;
  damage: number;
  range: number;
  reload: number | null;
  dps: number | null;
  splash: number;
  targets: string[];
  kind: string;
  color: string;
  lore: string | null;
  count: number;
};

export type Unit = {
  key: string;
  slug: string;
  faction: FactionSlug;
  name: string;
  role: string;
  tech: number;
  domain: Domain;
  categories: string[];
  lore: string | null;
  stats: {
    health: number;
    shield: number | null;
    mass: number;
    energy: number;
    buildTime: number;
    speed: number | null;
    vision: number | null;
    radar: number | null;
    sonar: number | null;
    range: number | null;
    dps: number | null;
    buildPower: number | null;
  };
  weapons: Weapon[];
  builds: string[];
  upgradesTo: string | null;
  mesh: string | null;
  icon: string | null;
};

export type Faction = {
  slug: FactionSlug;
  key: string;
  name: string;
  short: string;
  motto: string | null;
  meaning: string;
  about: string;
  field: string;
  crest: { title: string; text: string }[];
  units: number;
};

export const DOMAINS: { key: Domain; label: string; plural: string }[] = [
  { key: "command", label: "Command", plural: "Command" },
  { key: "land", label: "Land", plural: "Land" },
  { key: "air", label: "Air", plural: "Air" },
  { key: "naval", label: "Naval", plural: "Navy" },
  { key: "space", label: "Space", plural: "Spacecraft" },
  { key: "experimental", label: "Titan", plural: "Titans" },
  { key: "structure", label: "Structure", plural: "Structures" },
];

export const domainLabel = (d: Domain) => DOMAINS.find((x) => x.key === d)!.label;

export const unitHref = (u: { faction: string; slug: string }) => `/units/${u.faction}/${u.slug}`;

export const unitId = (u: { faction: string; slug: string }) => `${u.faction}-${u.slug}`;

export const fmt = (n: number | null | undefined, digits = 0) =>
  n == null ? "–" : n.toLocaleString("en-US", { maximumFractionDigits: digits });

/** Map a stat onto a 0-1 bar. Logarithmic, since a titan has 16000 times a scout's health. */
export const bar = (v: number | null, max: number) => (v ? Math.max(0.04, Math.log1p(v) / Math.log1p(max)) : 0);
