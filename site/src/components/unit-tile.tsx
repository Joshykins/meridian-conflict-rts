"use client";

import Link, { useLinkStatus } from "next/link";
import { useState, ViewTransition } from "react";
import { UnitArt } from "./unit-art";
import { DOMAIN_COLOR, TechPips } from "./unit-bits";
import { fmt, unitHref, unitId, type UnitSummary } from "@/lib/shared";

// A unit in the directory, drawn like an in-game build tile: a stripe in its
// domain colour down the left, its portrait, name over role.

// The morph from tile to page is armed only on the tile being pressed, and on
// the tile of the unit last opened (so Back flies it home). Naming every tile
// would make the browser snapshot all of them on every navigation.
let lastOpened: string | null = null;

function Pending() {
  const { pending } = useLinkStatus();
  return (
    <span
      aria-hidden
      className={`pointer-events-none absolute inset-0 overflow-hidden transition-opacity duration-150 ${pending ? "opacity-100" : "opacity-0"}`}
    >
      <span className="absolute inset-0 bg-accent/[0.06]" />
      <span className="absolute inset-y-0 left-0 w-1/2 animate-[sweep_0.9s_linear_infinite] bg-gradient-to-r from-transparent via-accent/25 to-transparent" />
    </span>
  );
}

export function UnitTile({ unit, compact = false, morph = true }: { unit: UnitSummary; compact?: boolean; morph?: boolean }) {
  // A view-transition name must be unique on the page: a tile shown twice morphs from one place only.
  const id = unitId(unit);
  const [pressed, setPressed] = useState(false);
  const armed = morph && (pressed || lastOpened === id);
  const arm = () => {
    if (!morph) return;
    lastOpened = id;
    setPressed(true);
  };
  const art = (
    <div className="h-full w-full opacity-90 transition-[opacity,scale] duration-500 ease-out group-hover:scale-[1.04] group-hover:opacity-100">
      <UnitArt portrait={unit.portrait} faction={unit.faction} className="h-full w-full" />
    </div>
  );
  const name = <h3 className="w-fit font-display text-xl font-semibold leading-tight">{unit.name}</h3>;
  return (
    <Link
      href={unitHref(unit)}
      transitionTypes={["nav-forward"]}
      onPointerDown={arm}
      onKeyDown={(e) => e.key === "Enter" && arm()}
      className="group relative flex h-full flex-col overflow-hidden border border-white/[0.08] bg-white/[0.025] transition-[background-color,border-color,translate] duration-300 ease-out hover:-translate-y-0.5 hover:border-white/20 hover:bg-white/[0.05] active:translate-y-0 active:duration-75"
    >
      <span
        className="absolute inset-y-0 left-0 w-[3px] origin-top transition-[width] duration-300 group-hover:w-[5px]"
        style={{ background: DOMAIN_COLOR[unit.domain] }}
      />
      <div className={`relative ${compact ? "aspect-[4/3]" : "aspect-[5/4]"} overflow-hidden`}>
        <ViewTransition name={armed ? `art-${id}` : undefined} share={armed ? "morph" : "none"} default="none">
          {art}
        </ViewTransition>
        <span className="absolute right-3 top-3">
          <TechPips tech={unit.tech} />
        </span>
        {unit.faction === "regency" && (
          <span className="absolute left-4 top-3 font-display text-xs font-semibold text-regency">Regency</span>
        )}
      </div>
      <div className="relative flex flex-1 flex-col gap-1 border-t border-white/[0.06] px-4 pb-3.5 pt-3">
        <ViewTransition name={armed ? `name-${id}` : undefined} share={armed ? "morph" : "none"} default="none">
          {name}
        </ViewTransition>
        <p className="text-sm text-dim">{unit.role}</p>
        {!compact && (
          <dl className="num mt-auto grid grid-cols-3 gap-2 pt-3 font-display text-sm">
            <Stat label="Health" value={fmt(unit.health)} />
            <Stat label="Mass" value={fmt(unit.mass)} />
            <Stat label="DPS" value={fmt(unit.dps)} hot={!!unit.dps} />
          </dl>
        )}
      </div>
      <Pending />
    </Link>
  );
}

function Stat({ label, value, hot }: { label: string; value: string; hot?: boolean }) {
  return (
    <div>
      <dt className="text-[0.7rem] font-medium text-faint">{label}</dt>
      <dd className={hot ? "text-text" : "text-dim"}>{value}</dd>
    </div>
  );
}
