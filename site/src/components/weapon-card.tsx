import { fmt, fmtDps, type Weapon } from "@/lib/shared";
import { weaponRows } from "@/lib/sheet";
import { SpecRows } from "./unit-sheet";

// One gun, or several mounts of the same gun, as the game's build card lists it: what
// it is and what it hits, the four figures that matter most, then everything else.

const GRADE: Record<string, string> = { Plasmeric: "Plasmeric", Pinched: "Pinched-plasmeric", PinchFusion: "Pinch-fusion" };

const KIND: Record<string, { label: string; color: string }> = {
  direct: { label: "Direct", color: "var(--color-accent)" },
  ballistic: { label: "Ballistic", color: "var(--color-accent)" },
  rocket: { label: "Rocket", color: "var(--color-accent)" },
  guided: { label: "Guided", color: "var(--color-accent)" },
  torpedo: { label: "Torpedo", color: "var(--color-naval)" },
  interceptor: { label: "Interceptor", color: "var(--color-naval)" },
  flak: { label: "Flak", color: "var(--color-dim)" },
  rail: { label: "Rail", color: "var(--color-space)" },
  bore: { label: "Bore", color: "var(--color-air)" },
  beam: { label: "Beam", color: "var(--color-air)" },
  plasma: { label: "Plasma", color: "var(--color-regency)" },
  charged: { label: "Charged shell", color: "var(--color-air)" },
  bolt: { label: "Bolt", color: "var(--color-air)" },
};

/** The domains a gun shoots at, in their colours (`hud/armament.rs` `DOMAINS`). */
const DOMAINS: { on: string[]; label: string; color: string }[] = [
  { on: ["Land", "Structure"], label: "Land", color: "var(--color-land)" },
  { on: ["Naval"], label: "Naval", color: "var(--color-naval)" },
  { on: ["Air"], label: "Air", color: "var(--color-air)" },
  { on: ["Space"], label: "Space", color: "var(--color-space)" },
];

export function WeaponCard({ w }: { w: Weapon }) {
  const k = KIND[w.kind] ?? KIND.direct;
  const label = w.kind === "plasma" && w.plasmaGrade ? (GRADE[w.plasmaGrade] ?? k.label) : k.label;
  const hits = w.intercepts
    ? [{ label: "Torpedoes", color: "var(--color-dim)" }]
    : DOMAINS.filter((d) => d.on.some((t) => w.targets.includes(t)));
  return (
    <li className="relative border border-white/[0.08] bg-white/[0.02] p-4">
      <span className="absolute inset-y-0 left-0 w-[3px]" style={{ background: k.color }} />
      <div className="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-2">
        <h3 className="font-display text-xl font-semibold">
          {w.name}
          {w.count > 1 && <span className="num ml-2 text-base text-faint">×{w.count}</span>}
        </h3>
        <div className="flex flex-wrap gap-1.5 font-display text-xs font-semibold">
          <Chip color={k.color}>{label}</Chip>
          {hits.map((d) => (
            <Chip key={d.label} color={d.color}>
              {d.label}
            </Chip>
          ))}
        </div>
      </div>
      {w.lore && <p className="mt-1.5 text-dim">{w.lore}</p>}
      <dl className="num mt-3 grid grid-cols-2 gap-3 font-display sm:grid-cols-4">
        <Figure label="Per shot" v={w.salvo > 1 ? `${fmt(w.damage)} × ${w.salvo}` : fmt(w.damage)} />
        <Figure label="Range" v={w.rangeMin ? `${fmt(w.rangeMin)}–${fmt(w.range)}` : fmt(w.range)} u="m" />
        <Figure label="Reload" v={fmt(w.reload, 1)} u="s" />
        <Figure label={w.count > 1 ? "DPS, all mounts" : "DPS"} v={w.dps == null ? "–" : fmtDps(w.dps * w.count)} />
      </dl>
      <div className="mt-3 border-t border-white/[0.06] pt-1">
        <SpecRows rows={weaponRows(w)} dense />
      </div>
    </li>
  );
}

function Chip({ color, children }: { color: string; children: React.ReactNode }) {
  return (
    <span className="px-1.5 py-0.5" style={{ color, background: `color-mix(in srgb, ${color} 16%, transparent)` }}>
      {children}
    </span>
  );
}

function Figure({ label, v, u }: { label: string; v: string; u?: string }) {
  return (
    <div>
      <dt className="text-xs font-medium text-faint">{label}</dt>
      <dd className="text-lg font-semibold">
        {v}
        {u && <span className="ml-0.5 text-sm font-medium text-faint">{u}</span>}
      </dd>
    </div>
  );
}
