import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { ViewTransition } from "react";
import { UnitSchematic } from "@/components/unit-schematic";
import { UnitTile } from "@/components/unit-tile";
import { DOMAIN_COLOR, DomainTag, TechPips } from "@/components/unit-bits";
import {
  allUnits,
  bar,
  byKey,
  domainLabel,
  factionOf,
  fmt,
  getUnit,
  lineage,
  unitHref,
  unitId,
  units,
  type Unit,
  type UnitSummary,
  type Weapon,
} from "@/lib/units";

export const dynamicParams = false;

export function generateStaticParams() {
  return units.map((u) => ({ faction: u.faction, slug: u.slug }));
}

type Params = { params: Promise<{ faction: string; slug: string }> };

export async function generateMetadata({ params }: Params): Promise<Metadata> {
  const { faction, slug } = await params;
  const u = await getUnit(faction, slug);
  if (!u) return {};
  return { title: `${u.name}, ${u.role}`, description: u.lore ?? `${u.name}: tech ${u.tech} ${u.role.toLowerCase()}.` };
}

const STATS: { key: keyof Unit["stats"]; label: string; unit?: string; digits?: number }[] = [
  { key: "health", label: "Health" },
  { key: "shield", label: "Shield" },
  { key: "dps", label: "Damage", unit: "/s", digits: 1 },
  { key: "range", label: "Range", unit: "m" },
  { key: "speed", label: "Speed", unit: "m/s" },
  { key: "vision", label: "Vision", unit: "m" },
  { key: "radar", label: "Radar", unit: "m" },
  { key: "sonar", label: "Sonar", unit: "m" },
  { key: "buildPower", label: "Build power" },
];

export default async function UnitPage({ params }: Params) {
  const { faction: fslug, slug } = await params;
  const unit = await getUnit(fslug, slug);
  if (!unit) notFound();
  const faction = factionOf(unit.faction)!;
  const everyone = await allUnits();
  const peers = everyone.filter((u) => u.domain === unit.domain);
  const max = (k: keyof Unit["stats"]) => Math.max(1, ...peers.map((u) => u.stats[k] ?? 0));
  const { builtBy, from } = await lineage(unit);
  const id = unitId(unit);

  // Neighbours in directory order, for the prev/next stepper.
  const order = units.filter((u) => u.faction === unit.faction);
  const at = order.findIndex((u) => u.slug === unit.slug);
  const prev = order[at - 1];
  const next = order[at + 1];
  const builds = unit.builds.map(byKey).filter((u): u is UnitSummary => !!u);
  const upgrade = unit.upgradesTo ? byKey(unit.upgradesTo) : undefined;
  const regency = unit.faction === "regency";

  return (
    <>
      <article className="mx-auto max-w-[1480px] px-4 pb-24 pt-[calc(var(--header-h)+1.5rem)] sm:px-8">
        <nav className="flex items-center justify-between gap-4 text-sm">
          <Link
            href="/units"
            transitionTypes={["nav-back"]}
            className="group inline-flex items-center gap-2 font-display font-semibold text-dim transition hover:text-text"
          >
            <span className="transition-transform group-hover:-translate-x-1">←</span> Unit directory
          </Link>
          <div className="flex items-center gap-1">
            {prev && <Step href={unitHref(prev)} dir="back" label={prev.name} />}
            {next && <Step href={unitHref(next)} dir="forward" label={next.name} />}
          </div>
        </nav>

        <div className="mt-6 grid gap-8 lg:grid-cols-[minmax(0,1.1fr)_minmax(0,1fr)] lg:gap-14">
          {/* The schematic, flown in from the tile. */}
          <div className="relative lg:sticky lg:top-[calc(var(--header-h)+1.5rem)] lg:self-start">
            <div className="ticks relative aspect-square overflow-hidden border border-white/[0.08] bg-[radial-gradient(ellipse_at_center,rgb(255_255_255/0.035),transparent_70%)]">
              <ViewTransition name={`art-${id}`} share="morph" default="none">
                <div className="h-full w-full">
                  <UnitSchematic
                    slug={unit.slug}
                    icon={unit.icon}
                    domain={unit.domain}
                    tech={unit.tech}
                    faction={unit.faction}
                    className="h-full w-full"
                    detail
                  />
                </div>
              </ViewTransition>
              <span className="absolute left-4 top-4 font-display text-xs font-semibold text-faint">
                Schematic · capture pending
              </span>
              <span className="num absolute bottom-4 left-4 font-display text-xs text-faint">{unit.key}</span>
              <span className="absolute right-4 top-4 size-2" style={{ background: DOMAIN_COLOR[unit.domain] }} />
            </div>
          </div>

          <div>
            <div className="rise flex flex-wrap items-center gap-x-4 gap-y-2" style={{ ["--i" as string]: 0 }}>
              <Link
                href={`/factions/${faction.slug}`}
                className={`font-display text-sm font-semibold transition hover:underline ${regency ? "text-regency" : "text-accent"}`}
              >
                {faction.name}
              </Link>
              <DomainTag domain={unit.domain} label={domainLabel(unit.domain)} />
              <TechPips tech={unit.tech} />
            </div>
            <ViewTransition name={`name-${id}`} share="morph" default="none">
              <h1 className="mt-3 w-fit font-display text-6xl font-light leading-[0.95] sm:text-7xl">{unit.name}</h1>
            </ViewTransition>
            <p className="rise mt-3 font-display text-2xl text-dim" style={{ ["--i" as string]: 1 }}>
              {unit.role}
            </p>
            {unit.lore && (
              <blockquote
                className="rise mt-6 border-l-2 border-accent/70 pl-4 text-lg leading-relaxed text-text/90"
                style={{ ["--i" as string]: 2 }}
              >
                {unit.lore}
              </blockquote>
            )}

            {/* Cost, as the build card shows it. */}
            <div className="rise mt-8 grid grid-cols-3 border border-white/[0.08]" style={{ ["--i" as string]: 3 }}>
              <Cost label="Mass" value={fmt(unit.stats.mass)} color="var(--color-land)" />
              <Cost label="Energy" value={fmt(unit.stats.energy)} color="var(--color-warn)" />
              <Cost label="Build time" value={fmt(unit.stats.buildTime)} color="var(--color-dim)" />
            </div>

            <section className="rise mt-8" style={{ ["--i" as string]: 4 }}>
              <h2 className="font-display text-sm font-semibold text-faint">
                Performance, against every other {domainLabel(unit.domain).toLowerCase()} unit
              </h2>
              <dl className="mt-3 divide-y divide-white/[0.06] border-y border-white/[0.06]">
                {STATS.filter((s) => unit.stats[s.key]).map((s, i) => (
                  <div key={s.key} className="grid grid-cols-[7.5rem_1fr_auto] items-center gap-4 py-2.5">
                    <dt className="text-dim">{s.label}</dt>
                    <div className="h-1.5 bg-white/[0.06]">
                      <div
                        className="h-full origin-left animate-[grow_1s_var(--ease-out-quint)_both]"
                        style={{
                          width: `${bar(unit.stats[s.key], max(s.key)) * 100}%`,
                          background:
                            s.key === "dps"
                              ? regency
                                ? "var(--color-regency)"
                                : "var(--color-accent)"
                              : s.key === "shield"
                                ? "var(--color-space)"
                                : "var(--color-text)",
                          animationDelay: `${300 + i * 60}ms`,
                        }}
                      />
                    </div>
                    <dd className="num text-right font-display text-lg font-semibold">
                      {fmt(unit.stats[s.key], s.digits)}
                      {s.unit && <span className="ml-1 text-sm font-medium text-faint">{s.unit}</span>}
                    </dd>
                  </div>
                ))}
              </dl>
            </section>

            {unit.weapons.length > 0 && (
              <section className="mt-10">
                <h2 className="font-display text-3xl font-light">Armament</h2>
                <ul className="mt-4 space-y-3">
                  {unit.weapons.map((w, i) => (
                    <WeaponCard key={`${w.name}-${i}`} w={w} />
                  ))}
                </ul>
              </section>
            )}
          </div>
        </div>

        {(builds.length > 0 || builtBy.length > 0 || upgrade || from) && (
          <div className="mt-20 space-y-14">
            {(from || upgrade) && (
              <Row title="Upgrade path">
                <div className="flex flex-wrap items-center gap-3">
                  {from && <Chain u={byKey(from.key)!} />}
                  {from && <Arrow />}
                  <span className="border border-accent/60 bg-accent/10 px-4 py-3 font-display text-lg font-semibold">
                    {unit.name}
                  </span>
                  {upgrade && <Arrow />}
                  {upgrade && <Chain u={upgrade} />}
                </div>
              </Row>
            )}
            {builds.length > 0 && (
              <Row title="Builds" count={builds.length}>
                <ul className="grid grid-cols-2 gap-3 sm:grid-cols-4 lg:grid-cols-6">
                  {builds.map((b) => (
                    <li key={b.slug}>
                      <UnitTile unit={b} compact />
                    </li>
                  ))}
                </ul>
              </Row>
            )}
            {builtBy.length > 0 && (
              <Row title="Built by" count={builtBy.length}>
                <ul className="grid grid-cols-2 gap-3 sm:grid-cols-4 lg:grid-cols-6">
                  {builtBy.map((b) => (
                    <li key={b.key}>
                      <UnitTile unit={byKey(b.key)!} compact morph={!unit.builds.includes(b.key)} />
                    </li>
                  ))}
                </ul>
              </Row>
            )}
          </div>
        )}
      </article>
    </>
  );
}

function Step({ href, dir, label }: { href: string; dir: "back" | "forward"; label: string }) {
  return (
    <Link
      href={href}
      transitionTypes={[dir === "back" ? "nav-back" : "nav-forward"]}
      className="group flex items-center gap-2 border border-white/10 px-3 py-1.5 font-display font-semibold text-dim transition hover:border-white/25 hover:text-text"
      aria-label={`${dir === "back" ? "Previous" : "Next"}: ${label}`}
    >
      {dir === "back" && <span className="transition-transform group-hover:-translate-x-0.5">‹</span>}
      <span className="hidden max-w-[10rem] truncate sm:inline">{label}</span>
      {dir === "forward" && <span className="transition-transform group-hover:translate-x-0.5">›</span>}
    </Link>
  );
}

function Cost({ label, value, color }: { label: string; value: string; color: string }) {
  return (
    <div className="relative border-r border-white/[0.08] px-4 py-3 last:border-r-0">
      <span className="absolute inset-x-0 top-0 h-[2px]" style={{ background: color }} />
      <div className="text-sm text-faint">{label}</div>
      <div className="num font-display text-2xl font-semibold">{value}</div>
    </div>
  );
}

const KIND: Record<string, { label: string; color: string }> = {
  direct: { label: "Gun", color: "var(--color-accent)" },
  ballistic: { label: "Artillery", color: "var(--color-accent)" },
  missile: { label: "Missile", color: "var(--color-accent)" },
  torpedo: { label: "Torpedo", color: "var(--color-naval)" },
  flak: { label: "Flak", color: "var(--color-dim)" },
  rail: { label: "Rail", color: "var(--color-space)" },
  bore: { label: "Electric bore", color: "var(--color-air)" },
  beam: { label: "Beam", color: "var(--color-air)" },
  plasma: { label: "Plasma", color: "var(--color-regency)" },
  charged: { label: "Charged shell", color: "var(--color-air)" },
  bolt: { label: "Bolt", color: "var(--color-air)" },
};

function WeaponCard({ w }: { w: Weapon }) {
  const k = KIND[w.kind] ?? KIND.direct;
  return (
    <li className="group relative border border-white/[0.08] bg-white/[0.02] p-4 transition-colors hover:border-white/20">
      <span className="absolute inset-y-0 left-0 w-[3px]" style={{ background: k.color }} />
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <h3 className="font-display text-xl font-semibold">
          {w.name}
          {w.count > 1 && <span className="num ml-2 text-base text-faint">×{w.count}</span>}
        </h3>
        <span className="font-display text-sm font-semibold" style={{ color: k.color }}>
          {k.label} · {w.targets.join(", ")}
        </span>
      </div>
      {w.lore && <p className="mt-1.5 text-dim">{w.lore}</p>}
      <dl className="num mt-3 grid grid-cols-4 gap-3 font-display">
        <WStat label="Damage" v={fmt(w.damage)} />
        <WStat label="Range" v={fmt(w.range)} u="m" />
        <WStat label="Reload" v={fmt(w.reload, 2)} u="s" />
        <WStat label="DPS" v={fmt(w.dps, 1)} />
      </dl>
    </li>
  );
}

function WStat({ label, v, u }: { label: string; v: string; u?: string }) {
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

function Row({ title, count, children }: { title: string; count?: number; children: React.ReactNode }) {
  return (
    <section className="reveal">
      <header className="mb-4 flex items-baseline gap-3 border-b border-white/[0.07] pb-2">
        <h2 className="font-display text-3xl font-light">{title}</h2>
        {count != null && <span className="num font-display text-sm text-faint">{count}</span>}
      </header>
      {children}
    </section>
  );
}

function Chain({ u }: { u: UnitSummary }) {
  return (
    <Link
      href={unitHref(u)}
      transitionTypes={["nav-forward"]}
      className="border border-white/10 px-4 py-3 font-display text-lg font-semibold text-dim transition hover:border-white/30 hover:text-text"
    >
      {u.name}
      <span className="ml-2 text-sm text-faint">T{u.tech}</span>
    </Link>
  );
}

const Arrow = () => <span className="text-faint">→</span>;
