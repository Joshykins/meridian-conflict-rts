import { fmt, fmtDps, type Unit } from "@/lib/shared";
import type { Row, Section } from "@/lib/sheet";

// A unit's figures: sections of labelled values (lib/sheet.ts), and its firepower
// against each domain as the game's build card sums it.

export function SpecRows({ rows, dense = false }: { rows: Row[]; dense?: boolean }) {
  return (
    <dl className={dense ? "grid gap-x-6 sm:grid-cols-2" : "divide-y divide-white/[0.06]"}>
      {rows.map((r) => (
        <div
          key={r.label}
          className={`flex items-baseline justify-between gap-4 ${dense ? "border-b border-white/[0.04] py-1.5 text-sm" : "py-2.5"}`}
        >
          <dt className="min-w-0 text-dim">
            {r.label}
            {r.note && <span className={`block text-faint ${dense ? "text-xs" : "text-sm"}`}>{r.note}</span>}
          </dt>
          <dd className={`num shrink-0 text-right font-display font-semibold ${dense ? "" : "text-lg"}`}>
            {r.value}
            {r.unit && <span className={`ml-1 font-medium text-faint ${dense ? "text-xs" : "text-sm"}`}>{r.unit}</span>}
          </dd>
        </div>
      ))}
    </dl>
  );
}

export function SpecSections({ sections }: { sections: Section[] }) {
  return (
    <div className="space-y-8">
      {sections.map((s) => (
        <section key={s.title}>
          <h3 className="border-b border-white/[0.12] pb-2 font-display text-sm font-semibold text-faint">{s.title}</h3>
          <SpecRows rows={s.rows} />
        </section>
      ))}
    </div>
  );
}

/** Damage a second against each domain, and the longest reach. */
export function Firepower({ unit }: { unit: Unit }) {
  const blocks = [
    { label: "Total DPS", value: unit.firepower.total, color: "var(--color-text)" },
    { label: "vs Land", value: unit.firepower.land, color: "var(--color-land)" },
    { label: "vs Naval", value: unit.firepower.naval, color: "var(--color-naval)" },
    { label: "vs Air", value: unit.firepower.air, color: "var(--color-air)" },
  ].filter((b) => b.value);
  return (
    <dl className="num grid grid-cols-2 gap-x-4 gap-y-3 font-display sm:grid-cols-5">
      {blocks.map((b) => (
        <div key={b.label} className="border-l-2 pl-3" style={{ borderColor: b.color }}>
          <dt className="text-xs font-medium text-faint">{b.label}</dt>
          <dd className="text-xl font-semibold" style={{ color: b.color }}>
            {fmtDps(b.value)}
          </dd>
        </div>
      ))}
      <div className="border-l-2 border-white/40 pl-3">
        <dt className="text-xs font-medium text-faint">Reach</dt>
        <dd className="text-xl font-semibold">
          {fmt(unit.stats.range)}
          <span className="ml-1 text-sm font-medium text-faint">m</span>
        </dd>
      </div>
    </dl>
  );
}
