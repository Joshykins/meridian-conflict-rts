// Types and helpers safe for client components (no file system, no data).

export type Domain = "command" | "land" | "air" | "naval" | "space" | "experimental" | "structure";
export type FactionSlug = "arc" | "regency";

export type UnitSummary = {
  key: string;
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
  /** Its portrait's address (`Art::portrait`), or null for a unit with no model. */
  portrait: string | null;
};

/** A unit's pictures, made from the game's model of it (`scripts/models.mjs`). */
export type Art = {
  /** The mesh file the viewer draws (`lib/mesh.ts`). */
  mesh: string;
  /** A cut-out portrait on a transparent ground, for tiles and before the viewer is up. */
  portrait: string;
  triangles: number;
};

/** A colour in linear RGB, as the game's data gives it. */
export type Rgb = [number, number, number];

/** A faction's paint (`mc_models::site::Paint`). */
export type Paint = { plating: Rgb; accent: Rgb; glow: Rgb; shield: Rgb; team: Rgb; bronze: boolean };

/** A gun, or several mounts of the same gun. Seconds, metres and degrees, as the unit files give them. */
export type Weapon = {
  name: string;
  kind: string;
  plasmaGrade: string | null;
  damage: number;
  salvo: number;
  salvoBatch: number;
  salvoDelay: number;
  reload: number;
  /** One mount's damage a second; null for an interceptor, which shoots torpedoes. */
  dps: number | null;
  splash: number;
  rangeMin: number;
  range: number;
  speed: number | null;
  trajectory: string;
  flatFire: boolean;
  loft: number;
  targets: string[];
  prefer: string[];
  turretTurn: number;
  arc: number;
  facing: number;
  spread: number;
  sweep: number;
  depression: number;
  keepsAim: boolean;
  slant: boolean;
  hitscan: boolean;
  beam: boolean;
  rail: boolean;
  flak: boolean;
  missile: boolean;
  guided: boolean;
  verticalLaunch: boolean;
  coldLaunch: number;
  hatch: number;
  split: boolean;
  skim: number;
  apogee: number;
  casing: number;
  proximity: number;
  burn: { seconds: number; dps: number } | null;
  spinUp: number;
  spinRamp: number;
  torpedo: boolean;
  intercepts: boolean;
  surfaced: boolean;
  bore: { width: number; damage: number; storm: { radius: number; seconds: number; dps: number } | null } | null;
  sabot: { damage: number; splash: number } | null;
  bombard: number;
  color: string;
  lore: string | null;
  mounts: string[];
  /** Where each mount's arc is centred, degrees off the bow: more than one when they differ. */
  facings: number[];
  count: number;
};

export type Economy = {
  massIncome: number;
  energyIncome: number;
  energyUpkeep: number;
  massStorage: number;
  energyStorage: number;
};

export type Shield = { kind: "Dome" | "Hull"; radius: number | null; health: number; regen: number };
export type Drones = { key: string; count: number; radius: number };
export type Cost = { mass: number; energy: number; time: number };

/** A part fitted onto the unit where it stands, and what it adds. */
export type RefitModule = {
  name: string;
  summary: string | null;
  after: string | null;
  tech: number | null;
  cost: Cost;
  health: number;
  regen: number;
  vision: number;
  buildPower: number;
  buildRange: number;
  builds: string[];
  economy: Economy;
  shield: Shield | null;
  drones: Drones | null;
  weapons: Weapon[];
};

export type Unit = {
  key: string;
  slug: string;
  faction: FactionSlug;
  name: string;
  title: string | null;
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
  firepower: { total: number | null; land: number | null; naval: number | null; air: number | null };
  hull: {
    regen: number;
    radius: number;
    height: number;
    footprint: [number, number] | null;
    wreckShare: number;
    wreckMass: number;
    cargoRoom: number | null;
    waterBuild: boolean;
    seabed: boolean;
  };
  motion: {
    layer: string;
    hover: boolean;
    speed: number;
    accel: number;
    turn: number;
    altitude: number | null;
    deploy: number | null;
    broadside: number | null;
    stride: boolean;
    orbit: number | null;
  } | null;
  dive: { depth: number; time: number } | null;
  economy: Economy;
  mine: { reach: number; seaReach: number; ground: number; ore: number; base: number } | null;
  builder: { power: number; range: number } | null;
  reclaimer: { power: number; range: number; charge: number; mobile: boolean; heads: number } | null;
  drones: Drones | null;
  shield: Shield | null;
  antiMissile: { range: number; lasers: number } | null;
  transport: { capacity: number; descent: number; unload: number } | null;
  warp: { perKm: number; spool: number; cooldown: number; speed: number } | null;
  warpDamper: { radius: number; drag: number; damage: number; stun: number } | null;
  strategic: {
    kind: "Nuke" | "Interceptor";
    round: Cost;
    power: number;
    stock: number;
    speed: number;
    apogee: number | null;
    coverage: number | null;
    blast: { radius: number; core: number; damage: number; edge: number; front: number } | null;
  } | null;
  deathBlast: { radius: number; damage: number } | null;
  stomp: { radius: number; damage: number; pace: number } | null;
  weapons: Weapon[];
  refits: { name: string; modules: RefitModule[] }[];
  builds: string[];
  upgradesTo: string | null;
  upgradeCost: { mass: number; energy: number } | null;
  /** A drone that flies for its carrier: never built or ordered on its own. */
  carried: boolean;
  mesh: string | null;
  icon: string | null;
  art: Art | null;
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
  paint: Paint;
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

/** Damage a second: whole once it is in the hundreds, as the game's cards print it. */
export const fmtDps = (n: number | null | undefined) => fmt(n, n != null && n >= 100 ? 0 : 1);

/** Map a stat onto a 0-1 bar. Logarithmic, since a titan has 16000 times a scout's health. */
export const bar = (v: number | null, max: number) => (v ? Math.max(0.04, Math.log1p(v) / Math.log1p(max)) : 0);
