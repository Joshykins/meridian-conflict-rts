import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { ViewTransition } from "react";
import { ArcCrest, RegencyCrest } from "@/components/crests";
import { UnitTile } from "@/components/unit-tile";
import { factionOf, factions, units } from "@/lib/units";

export const dynamicParams = false;

export function generateStaticParams() {
  return factions.map((f) => ({ faction: f.slug }));
}

type Params = { params: Promise<{ faction: string }> };

export async function generateMetadata({ params }: Params): Promise<Metadata> {
  const f = factionOf((await params).faction);
  return f ? { title: f.name, description: f.about } : {};
}

export default async function FactionPage({ params }: Params) {
  const { faction: slug } = await params;
  const f = factionOf(slug);
  if (!f) notFound();
  const regency = f.slug === "regency";
  const hot = regency ? "var(--color-regency)" : "var(--color-accent)";
  // The heaviest of each kind: the faction's top tier in every domain it fields.
  const roster = units.filter((u) => u.faction === f.slug && u.domain !== "structure");
  const best = [...roster].sort((a, b) => b.tech - a.tech || b.health - a.health).slice(0, 8);

  return (
    <>
      <div className="relative overflow-hidden">
        <div
          aria-hidden
          className="pointer-events-none absolute inset-x-0 top-0 h-[80vh]"
          style={{
            background: regency
              ? "radial-gradient(60% 50% at 75% 20%, rgb(224 52 43 / 0.16), transparent 70%), radial-gradient(40% 40% at 20% 0%, rgb(154 116 71 / 0.12), transparent 70%)"
              : "radial-gradient(60% 50% at 75% 20%, rgb(255 90 36 / 0.12), transparent 70%), radial-gradient(40% 40% at 20% 0%, rgb(128 205 245 / 0.08), transparent 70%)",
          }}
        />
        <div className="relative mx-auto max-w-[1480px] px-4 pb-24 pt-[calc(var(--header-h)+2rem)] sm:px-8">
          {/* Tabs between the factions: same place, different content. */}
          <nav className="flex gap-1 border-b border-white/10">
            {factions.map((x) => (
              <Link
                key={x.slug}
                href={`/factions/${x.slug}`}
                transitionTypes={[x.slug === "arc" ? "nav-back" : "nav-forward"]}
                className={`relative px-4 py-3 font-display text-lg font-semibold transition-colors ${x.slug === f.slug ? "text-text" : "text-faint hover:text-dim"}`}
              >
                {x.short === "ARC" ? "Asterian Reach Command" : x.name}
                {x.slug === f.slug && (
                  <ViewTransition name="faction-tab" share="morph" default="none">
                    <span
                      className="absolute inset-x-4 -bottom-px h-[2px]"
                      style={{ background: x.slug === "regency" ? "var(--color-regency)" : "var(--color-accent)" }}
                    />
                  </ViewTransition>
                )}
              </Link>
            ))}
          </nav>

          <section className="grid items-center gap-10 py-14 md:grid-cols-[1.3fr_1fr]">
            <div>
              <p className="rise font-display text-sm font-semibold" style={{ color: hot }}>
                {regency ? "Command's file on the enemy" : "The Reach's own army"}
              </p>
              <h1
                className="rise mt-3 font-display text-5xl font-light leading-[0.95] sm:text-7xl"
                style={{ ["--i" as string]: 1 }}
              >
                {f.name}
              </h1>
              {f.motto && (
                <p className="rise mt-4 font-display text-2xl font-light text-dim" style={{ ["--i" as string]: 2 }}>
                  “{f.motto}”
                </p>
              )}
              <p className="rise mt-6 max-w-2xl text-lg leading-relaxed text-text/90" style={{ ["--i" as string]: 3 }}>
                {f.about}
              </p>
              <p className="rise mt-4 max-w-2xl leading-relaxed text-dim" style={{ ["--i" as string]: 4 }}>
                {f.meaning}
              </p>
              <div className="rise mt-8 flex flex-wrap gap-3" style={{ ["--i" as string]: 5 }}>
                <Link
                  href={`/units?faction=${f.slug}`}
                  transitionTypes={["nav-forward"]}
                  className="border px-5 py-2.5 font-display text-lg font-semibold text-black transition hover:brightness-110"
                  style={{ background: hot, borderColor: hot }}
                >
                  All {f.units} {f.short} units
                </Link>
              </div>
            </div>
            <div
              className="rise relative mx-auto w-full max-w-sm"
              style={{ ["--i" as string]: 2, color: regency ? "var(--color-bronze)" : "var(--color-text)" }}
            >
              <div className="ticks aspect-square p-8">
                {regency ? <RegencyCrest className="h-full w-full" /> : <ArcCrest className="h-full w-full" />}
              </div>
            </div>
          </section>

          <section
            className={`grid gap-px border border-white/[0.08] bg-white/[0.08] sm:grid-cols-2 ${f.crest.length === 5 ? "lg:grid-cols-5" : "lg:grid-cols-4"}`}
          >
            {f.crest.map((c, i) => (
              <div key={c.title} className="reveal bg-void p-5" style={{ ["--i" as string]: i }}>
                <h3 className="font-display text-lg font-semibold">{c.title}</h3>
                <p className="mt-2 text-dim">{c.text}</p>
              </div>
            ))}
          </section>

          <section className="mt-6 border-l-2 pl-5" style={{ borderColor: hot }}>
            <h2 className="font-display text-sm font-semibold text-faint">In the field</h2>
            <p className="mt-1 max-w-3xl text-lg">{f.field}</p>
          </section>

          <section className="mt-16">
            <header className="mb-4 flex items-baseline justify-between gap-3 border-b border-white/[0.07] pb-2">
              <h2 className="font-display text-3xl font-light">Heaviest in the line</h2>
              <Link
                href={`/units?faction=${f.slug}`}
                transitionTypes={["nav-forward"]}
                className="font-display font-semibold text-dim transition hover:text-text"
              >
                Full roster →
              </Link>
            </header>
            <ul className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
              {best.map((u) => (
                <li key={u.slug} className="reveal">
                  <UnitTile unit={u} />
                </li>
              ))}
            </ul>
          </section>
        </div>
      </div>
    </>
  );
}
