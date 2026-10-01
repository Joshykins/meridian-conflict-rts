import type { Metadata } from "next";
import { UnitBrowser } from "@/components/unit-browser";
import { factions, units } from "@/lib/units";

export const metadata: Metadata = {
  title: "Unit directory",
  description: "Every unit and structure in Meridian Conflict: what it does, what it costs and what it carries.",
};

export default function UnitsPage() {
  return (
    <>
      <div className="mx-auto max-w-[1480px] px-4 pb-28 pt-[calc(var(--header-h)+2.5rem)] sm:px-8">
        <header className="grid gap-6 pb-8 md:grid-cols-[1fr_auto] md:items-end">
          <div>
            <p className="rise font-display text-sm font-semibold text-accent">Unit directory</p>
            <h1 className="rise mt-2 font-display text-5xl font-light leading-none sm:text-7xl" style={{ ["--i" as string]: 1 }}>
              Everything on the field
            </h1>
            <p className="rise mt-4 max-w-2xl text-lg text-dim" style={{ ["--i" as string]: 2 }}>
              Every unit and structure in the game, its figures read straight from its data and its picture drawn from its own
              model: what it does, what it costs and what it carries.
            </p>
          </div>
          <dl className="rise num flex gap-8 font-display" style={{ ["--i" as string]: 3 }}>
            <Figure label="On file" value={units.length} />
            {factions.map((f) => (
              <Figure key={f.slug} label={f.short} value={f.units} />
            ))}
          </dl>
        </header>
        <UnitBrowser units={units} />
      </div>
    </>
  );
}

function Figure({ label, value }: { label: string; value: number }) {
  return (
    <div>
      <dt className="text-sm text-faint">{label}</dt>
      <dd className="text-4xl font-light">{value}</dd>
    </div>
  );
}
