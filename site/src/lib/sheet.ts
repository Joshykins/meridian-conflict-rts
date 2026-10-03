// A unit's whole specification, laid out as sections of labelled figures: everything
// its data file says that changes how it plays. What a unit does not have is left out,
// so a wall's sheet is short and a titan's is long. The words follow the game's own
// cards (hud/armament.rs, hud/selection/details.rs).
import { fmt, type Economy, type RefitModule, type Shield, type Unit, type Weapon } from "./shared";

export type Row = {
  label: string;
  value: string;
  /** Printed small after the value: "m", "m/s". */
  unit?: string;
  /** A few words under the label, where the figure needs them. */
  note?: string;
};

export type Section = { title: string; rows: Row[] };

const row = (label: string, value: string, unit?: string, note?: string): Row => ({ label, value, unit, note });
/** A figure, when the unit has it. */
const num = (label: string, n: number | null | undefined, unit?: string, digits = 0, note?: string): Row[] =>
  n ? [row(label, fmt(n, digits), unit, note)] : [];
const flag = (label: string, on: boolean, note?: string): Row[] => (on ? [row(label, "Yes", undefined, note)] : []);
const seconds = (n: number) => fmt(n, n < 10 ? 1 : 0);

const LAYER: Record<string, string> = {
  Land: "Land",
  Amphibious: "Land and seabed",
  Naval: "Water",
  Hover: "Land and water",
  Air: "Air",
};

function shieldRows(s: Shield): Row[] {
  return [
    row("Shield", fmt(s.health), undefined, s.kind === "Hull" ? "A field round its own hull" : "A dome over everything under it"),
    ...num("Shield radius", s.radius, "m"),
    ...num("Shield recharge", s.regen, "/s", 1),
  ];
}

function economyRows(e: Economy): Row[] {
  return [
    ...num("Mass income", e.massIncome, "/s", 1),
    ...num("Energy income", e.energyIncome, "/s", 1),
    ...num("Energy upkeep", e.energyUpkeep, "/s", 1, "Drawn while it is active"),
    ...num("Mass storage", e.massStorage),
    ...num("Energy storage", e.energyStorage),
  ];
}

/** Who builds it and how long each takes: build time over build power. */
export type Maker = { name: string; power: number };

export function sheetOf(unit: Unit, makers: Maker[], droneName: (key: string) => string): Section[] {
  const { stats, hull, motion: m } = unit;
  const sections: Section[] = [];
  const add = (title: string, rows: Row[]) => {
    if (rows.length) sections.push({ title, rows });
  };

  add("Hull", [
    row("Health", fmt(stats.health)),
    ...num("Repairs itself", hull.regen, "/s", 1),
    ...(unit.shield ? shieldRows(unit.shield) : []),
    row("Size", `${fmt(hull.radius * 2, 1)} × ${fmt(hull.height, 1)}`, "m", "Across and tall"),
    ...(hull.footprint && hull.footprint[0] > 0 ? [row("Lot", `${hull.footprint[0]} × ${hull.footprint[1]}`, "cells")] : []),
    ...num("Wreck", hull.wreckMass, "mass", 0, `${fmt(hull.wreckShare * 100)}% of its cost left to reclaim`),
    ...num("Room in a hold", hull.cargoRoom, undefined, 0, "Of a lift ship's capacity"),
    ...flag("Built on water", hull.waterBuild && !hull.seabed),
    ...flag("Built on the seabed", hull.seabed, "Under the surface: only sonar finds it, only torpedoes reach it"),
    ...(unit.deathBlast
      ? [
          row(
            "Blast when destroyed",
            fmt(unit.deathBlast.damage),
            "damage",
            `Within ${fmt(unit.deathBlast.radius)} m, its owner's units too`,
          ),
        ]
      : []),
  ]);

  if (m) {
    add("Movement", [
      row("Moves on", LAYER[m.layer] ?? m.layer, undefined, m.hover && m.layer !== "Hover" ? "Hovers" : undefined),
      row("Speed", fmt(m.speed, 1), "m/s"),
      row("Acceleration", fmt(m.accel, 1), "m/s²"),
      row("Turn rate", fmt(m.turn), "°/s"),
      ...num("Cruise height", m.altitude, "m"),
      ...num("Orbit radius", m.orbit, "m", 0, "The circle it flies while it guards"),
      ...num("Deploy time", m.deploy, "s", 1, "To plant before it fires, and to pack before it moves"),
      ...num("Fights broadside", m.broadside, "°", 0, "Off the bow, once engaged and stopped"),
      ...flag("Strides over obstacles", m.stride, "Structures, steep ground and shallow water"),
      ...(unit.dive ? [row("Dives", fmt(unit.dive.depth), "m", `Under in ${seconds(unit.dive.time)} s`)] : []),
      ...(unit.stomp
        ? [
            row(
              "Footfall",
              fmt(unit.stomp.damage),
              "damage",
              `To ground units within ${fmt(unit.stomp.radius, 1)} m of each step`,
            ),
          ]
        : []),
    ]);
  }

  add("Sensors", [
    ...num("Vision", stats.vision, "m"),
    ...num("Radar", stats.radar, "m"),
    ...num("Sonar", stats.sonar, "m", 0, "Finds submerged hulls"),
  ]);

  add("Economy", [
    ...economyRows(unit.economy),
    ...(unit.mine
      ? [
          row(
            "Mine reach",
            fmt(unit.mine.reach),
            "m",
            unit.mine.seaReach !== unit.mine.reach ? `${fmt(unit.mine.seaReach)} m standing in the sea` : undefined,
          ),
          ...num("From the shaft", unit.mine.base, "/s", 2, "From the moment it is finished"),
          ...num("From open ground", unit.mine.ground, "/s per hectare", 3),
          ...num("From ore", unit.mine.ore, "/s per hectare", 3),
        ]
      : []),
  ]);

  add("Construction", [
    ...(unit.builder ? [row("Build power", fmt(unit.builder.power, 1)), row("Build range", fmt(unit.builder.range), "m")] : []),
    ...(unit.reclaimer
      ? [
          row("Reclaim power", fmt(unit.reclaimer.power, 1), "/s"),
          row("Reclaim range", fmt(unit.reclaimer.range), "m"),
          ...num("Reclaim heads", unit.reclaimer.heads > 1 ? unit.reclaimer.heads : 0),
          ...num("Beam charge", unit.reclaimer.charge, "s", 1, "On a target before the beam comes on"),
          ...flag("Reclaims on the move", unit.reclaimer.mobile),
        ]
      : []),
    ...(unit.drones
      ? [
          row(
            "Salvage drones",
            `${unit.drones.count} × ${droneName(unit.drones.key)}`,
            undefined,
            `Range out to ${fmt(unit.drones.radius)} m`,
          ),
        ]
      : []),
  ]);

  add("Production", [
    row("Build time", fmt(stats.buildTime), "work", "Seconds, at a build power of one"),
    ...makers.map((b) => row(`By ${b.name}`, seconds(stats.buildTime / b.power), "s", `Build power ${fmt(b.power, 1)}`)),
    ...(unit.upgradeCost
      ? [
          row(
            "As an upgrade",
            `${fmt(unit.upgradeCost.mass)} mass`,
            undefined,
            `and ${fmt(unit.upgradeCost.energy)} energy: only what it costs over the tier before`,
          ),
        ]
      : []),
    ...(unit.carried
      ? [row("Made by", "Its carrier", undefined, "It flies for the unit that carries it and takes no orders of its own")]
      : []),
  ]);

  const s = unit.strategic;
  if (s) {
    add(s.kind === "Nuke" ? "Warheads" : "Interceptors", [
      row("Holds", fmt(s.stock), s.kind === "Nuke" ? "warheads" : "interceptors"),
      row("Each costs", `${fmt(s.round.mass)} mass`, undefined, `and ${fmt(s.round.energy)} energy`),
      row("Assembled in", seconds(s.round.time / s.power), "s", `Its own line, build power ${fmt(s.power)}`),
      row("Missile speed", fmt(s.speed), "m/s"),
      ...num("Climbs to", s.apogee, "m"),
      ...num("Covers", s.coverage, "m", 0, "Warheads bound for inside it are shot at"),
      ...(s.blast
        ? [
            row("Blast", fmt(s.blast.damage), "damage", `Everything within ${fmt(s.blast.core)} m`),
            row(
              "Blast reach",
              fmt(s.blast.radius),
              "m",
              `${fmt(s.blast.edge)} damage at its edge, ${seconds(s.blast.front)} s after the middle`,
            ),
          ]
        : []),
    ]);
  }

  add("Systems", [
    ...(unit.antiMissile
      ? [
          row(
            "Missile defence",
            fmt(unit.antiMissile.range),
            "m",
            unit.antiMissile.lasers > 1 ? `${unit.antiMissile.lasers} lasers, a missile each` : "One laser",
          ),
        ]
      : []),
    ...(unit.transport
      ? [
          row("Hold", fmt(unit.transport.capacity), "room", "A land unit takes its size class plus one"),
          row("Climbs and lands at", fmt(unit.transport.descent, 1), "m/s"),
          row("Unloads a unit every", fmt(unit.transport.unload, 1), "s"),
        ]
      : []),
    ...(unit.warp
      ? [
          row("Warp charge", fmt(unit.warp.perKm), "energy a km", `At least one km; spools in ${seconds(unit.warp.spool)} s plus a tenth a km at full power`),
          row("Warp speed", fmt(unit.warp.speed), "m/s"),
          row("Warp cooldown", seconds(unit.warp.cooldown), "s"),
        ]
      : []),
    ...(unit.warpDamper
      ? [
          row("Dampens warps within", fmt(unit.warpDamper.radius), "m"),
          row(
            "A dampened jump",
            `${fmt(unit.warpDamper.drag, 1)}× longer`,
            undefined,
            `Costs the ship ${fmt(unit.warpDamper.damage * 100)}% of its health and stuns it ${seconds(unit.warpDamper.stun)} s`,
          ),
        ]
      : []),
  ]);

  return sections;
}

const FACING: [number, string][] = [
  [180, "aft"],
  [-180, "aft"],
  [90, "to port"],
  [-90, "to starboard"],
];

/** Every figure of a gun past the four on its card's first line. */
export function weaponRows(w: Weapon): Row[] {
  const centred =
    w.facings.length > 1
      ? "Each mount on its own side"
      : `Centred ${FACING.find(([deg]) => deg === w.facing)?.[1] ?? (w.facing ? `${fmt(w.facing)}° off the bow` : "forward")}`;
  return [
    ...num("Splash", w.splash, "m", 1),
    ...(w.hitscan ? [row("Velocity", "Instant")] : num("Velocity", w.speed, "m/s")),
    row("Trajectory", w.trajectory === "Ballistic" ? (w.flatFire ? "Ballistic, laid flat" : "Ballistic") : "Direct"),
    ...(w.salvo > 1
      ? [
          row(
            "Salvo",
            `${w.salvo} shots`,
            undefined,
            w.salvoDelay
              ? `${w.salvoBatch > 1 ? `${w.salvoBatch} at a time, ` : ""}${fmt(w.salvoDelay, 1)} s apart`
              : "All at once",
          ),
        ]
      : []),
    w.turretTurn ? row("Traverse", fmt(w.turretTurn), "°/s") : row("Traverse", "Fixed", undefined, "It aims with the hull"),
    ...(w.arc < 360 ? [row("Firing arc", fmt(w.arc), "°", centred)] : []),
    ...num("Aim error", w.spread, "°", 2),
    ...num("Sweep", w.sweep, "°", 0, "Fires down its barrel at anything this near its line"),
    ...num("Spin-up", w.spinUp, "s", 1, w.spinRamp ? "Opens fire early, slower" : "Before it fires"),
    ...num("Stays up", w.loft, "s", 1, "Longer in flight, for a higher arc"),
    ...(w.missile
      ? [
          row(
            "Guidance",
            w.guided ? "Guided" : "Unguided",
            undefined,
            w.split ? "A salvo spreads over the targets in range" : undefined,
          ),
        ]
      : []),
    ...flag(
      "Vertical launch",
      w.verticalLaunch,
      w.coldLaunch ? `Thrown clear, lights after ${fmt(w.coldLaunch, 1)} s` : undefined,
    ),
    ...num("Hatches open in", w.hatch, "s", 1),
    ...num("Cruises at", w.skim, "m", 0, "Over ground and water"),
    ...num("Climbs to", w.apogee, "m"),
    ...num("Casing", w.casing, "hp", 0, "What a defence laser must burn through"),
    ...num("Proximity fuse", w.proximity, "m", 1),
    ...(w.burn ? [row("Sets fire", fmt(w.burn.dps), "damage/s", `For ${seconds(w.burn.seconds)} s where it lands`)] : []),
    ...(w.bore
      ? [
          ...num(
            "Bore discharge",
            w.bore.damage,
            "damage",
            0,
            w.bore.width ? `To everything within ${fmt(w.bore.width, 1)} m of its channel` : undefined,
          ),
          ...(w.bore.storm
            ? [
                row(
                  "Storm",
                  fmt(w.bore.storm.dps),
                  "damage/s",
                  `Grows to ${fmt(w.bore.storm.radius)} m over ${seconds(w.bore.storm.seconds)} s`,
                ),
              ]
            : []),
        ]
      : []),
    ...(w.sabot
      ? [row("Spent casing", fmt(w.sabot.damage), "damage", `Within ${fmt(w.sabot.splash, 1)} m of where it falls`)]
      : []),
    ...num("Bombards", w.bombard, "m", 0, "The widest circle it will spread its shots over"),
    ...num("Lowest aim", w.depression, "°", 0, "Below its deck: nearer than that is a dead zone"),
    ...(w.prefer.length ? [row("Shoots first", w.prefer.join(", "))] : []),
    ...flag("Slant range", w.slant, "Measured along the line of sight to the ground"),
    ...flag("Surfaced only", w.surfaced),
    ...flag("Holds its aim", w.keepsAim, "Stays laid where it last fired"),
  ];
}

/** What a refit module adds, as rows. */
export function moduleRows(m: RefitModule): Row[] {
  return [
    ...num("Health", m.health, undefined, 0).map((r) => ({ ...r, value: `+${r.value}` })),
    ...num("Repairs itself", m.regen, "/s", 1).map((r) => ({ ...r, value: `+${r.value}` })),
    ...num("Vision", m.vision, "m").map((r) => ({ ...r, value: `+${r.value}` })),
    ...num("Build power", m.buildPower, undefined, 1).map((r) => ({ ...r, value: `+${r.value}` })),
    ...num("Build range", m.buildRange, "m").map((r) => ({ ...r, value: `+${r.value}` })),
    ...economyRows(m.economy),
    ...(m.shield ? shieldRows(m.shield) : []),
    ...(m.drones ? [row("Salvage drones", String(m.drones.count), undefined, `Range out to ${fmt(m.drones.radius)} m`)] : []),
    ...num("Counts as tech", m.tech),
  ];
}
