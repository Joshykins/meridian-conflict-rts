import Link from "next/link";
import { HeroMedia } from "@/components/hero-media";
import { ArcCrest, RegencyCrest } from "@/components/crests";
import { UnitTile } from "@/components/unit-tile";
import { UnitArt } from "@/components/unit-art";
import { factions, units, type UnitSummary } from "@/lib/units";
import site from "../../content/site.json";

const find = (path: string) => {
  const [faction, slug] = path.split("/");
  return units.find((u) => u.faction === faction && u.slug === slug);
};

// What the player does, each shown by a unit that stands for it.
const PILLARS = [
  {
    unit: "arc/commander",
    title: "Build from nothing",
    text: "Your Commander lands alone and prints a base out of the ground: mines, reactors, factories and the guns to hold them.",
  },
  {
    unit: "arc/t3-battleship",
    title: "Fight on every front",
    text: "Tanks and walkers, fighters and gunships, submarines and battleships, and warships that fly. Every domain can reach every other.",
  },
  {
    unit: "arc/t5-titan",
    title: "Raise a titan",
    text: "Tech 4 and 5 machines too big for any factory, built on a lot of their own and walked off it into the fight.",
  },
  {
    unit: "arc/t4-nuke-silo",
    title: "Strike across the map",
    text: "Strategic missiles, map-range artillery and the interceptor networks that stand against them.",
  },
];

const TIMELINE = [
  {
    when: "0 AC",
    title: "The Crossing",
    text: "Humanity's own drive makes the first jump. Survey ships find Asteria, a garden world.",
  },
  {
    when: "Unification",
    title: "Reach Command",
    text: "Earth and Asteria join as the Asterian Reach, and fold their armies into one command.",
  },
  { when: "150 AC", title: "Meridia", text: "The Reach settles a frontier ore world, rich enough to fight over." },
  {
    when: "175 AC",
    title: "The Arrival",
    text: "Envoys of the Regency come to Meridia. The first battle is a massacre.",
    hot: true,
  },
];

export default function Home() {
  const featured = site.featured.map(find).filter((u): u is UnitSummary => !!u);
  const tiers = new Set(units.map((u) => u.tech)).size;

  return (
    <>
      {/* ---- Hero --------------------------------------------------------- */}
      <section className="relative h-[100svh] min-h-[620px] overflow-hidden">
        <HeroMedia />
        <div className="pointer-events-none absolute inset-0 bg-[linear-gradient(90deg,rgb(5_5_6/0.75)_0%,rgb(5_5_6/0.2)_45%,transparent_70%)]" />
        <div className="pointer-events-none absolute inset-0 bg-[linear-gradient(0deg,rgb(5_5_6/0.92)_0%,rgb(5_5_6/0.6)_45%,transparent_70%)] sm:hidden" />

        <div className="relative mx-auto flex h-full max-w-[1480px] flex-col justify-end px-4 pb-16 sm:px-8 sm:pb-20">
          <p
            className="rise flex items-center gap-3 font-display text-base font-semibold text-dim"
            style={{ ["--i" as string]: 0 }}
          >
            <span className="h-px w-10 bg-accent" />A real-time strategy game · In development
          </p>
          <h1 className="mt-4 font-display text-[clamp(4rem,13vw,11.5rem)] leading-[0.82] tracking-[-0.01em]">
            <span className="rise block font-semibold" style={{ ["--i" as string]: 1 }}>
              Meridian
            </span>
            <span className="rise block font-extralight text-text/75" style={{ ["--i" as string]: 2 }}>
              Conflict
            </span>
          </h1>
          <p className="rise mt-6 max-w-xl text-lg leading-relaxed text-text/85 sm:text-xl" style={{ ["--i" as string]: 3 }}>
            {site.description}
          </p>
          <div className="rise mt-8 flex flex-wrap items-center gap-3" style={{ ["--i" as string]: 4 }}>
            <Link
              href="/units"
              transitionTypes={["nav-forward"]}
              className="group relative inline-flex items-center gap-3 overflow-hidden bg-accent px-6 py-3.5 font-display text-lg font-semibold text-black transition hover:brightness-110 active:scale-[0.98]"
            >
              Browse the units
              <span className="transition-transform duration-300 group-hover:translate-x-1">→</span>
            </Link>
            <a
              href={site.discord}
              target="_blank"
              rel="noreferrer"
              className="glass inline-flex items-center gap-3 px-6 py-3.5 font-display text-lg font-semibold transition hover:border-white/30"
            >
              Join the Discord
            </a>
            <span
              className="inline-flex items-center gap-2.5 px-3 py-3.5 font-display font-semibold text-faint"
              title="The trailer is being cut"
            >
              <span className="grid size-7 place-items-center rounded-full border border-white/20">
                <svg viewBox="0 0 10 10" className="ml-0.5 size-2.5" aria-hidden>
                  <path d="M1 0L9 5L1 10Z" fill="currentColor" />
                </svg>
              </span>
              Trailer soon
            </span>
          </div>
        </div>

        {/* Instrument readout, top right. */}
        <div
          className="rise pointer-events-none absolute right-4 top-[calc(var(--header-h)+1.5rem)] hidden text-right font-display text-sm text-faint sm:right-8 md:block"
          style={{ ["--i" as string]: 6 }}
        >
          <p className="num">Meridia · 175 AC</p>
          <p className="mt-1 text-accent/80">The meridian, pole to pole</p>
        </div>
        <div className="pointer-events-none absolute bottom-6 right-4 hidden flex-col items-center gap-2 font-display text-xs text-faint sm:right-8 md:flex">
          <span>Scroll</span>
          <span className="h-10 w-px animate-[blink_1.8s_ease-in-out_infinite] bg-gradient-to-b from-white/50 to-transparent" />
        </div>
      </section>

      {/* ---- Figures ------------------------------------------------------ */}
      <section className="border-y border-white/[0.08] bg-black/40">
        <dl className="mx-auto grid max-w-[1480px] grid-cols-2 md:grid-cols-4">
          {[
            { v: units.length, l: "Units and structures on file" },
            { v: factions.length, l: "Factions so far" },
            { v: tiers, l: "Tech tiers, up to titans" },
            { v: 4, l: "Domains: land, sea, air, orbit" },
          ].map((f, i) => (
            <div
              key={f.l}
              className="reveal border-white/[0.08] px-4 py-7 sm:px-8 [&:not(:last-child)]:border-r max-md:[&:nth-child(2)]:border-r-0 max-md:[&:nth-child(-n+2)]:border-b"
              style={{ ["--i" as string]: i }}
            >
              <dd className="num font-display text-5xl font-extralight sm:text-6xl">{f.v}</dd>
              <dt className="mt-1 text-sm text-dim">{f.l}</dt>
            </div>
          ))}
        </dl>
      </section>

      {/* ---- The war ------------------------------------------------------ */}
      <section id="war" className="relative scroll-mt-[var(--header-h)] overflow-hidden">
        <div className="hairline-grid pointer-events-none absolute inset-0 [mask-image:radial-gradient(70%_60%_at_30%_40%,black,transparent)] opacity-60" />
        <div className="relative mx-auto grid max-w-[1480px] gap-14 px-4 py-24 sm:px-8 md:py-32 lg:grid-cols-[1.15fr_1fr]">
          <div className="reveal">
            <p className="font-display text-sm font-semibold text-accent">The war</p>
            <blockquote className="mt-4 font-display text-4xl font-light leading-[1.08] sm:text-5xl lg:text-6xl">
              They came out of the dark in 175&nbsp;AC, and the first battle was a massacre.
            </blockquote>
            <p className="mt-6 max-w-xl text-lg leading-relaxed text-dim">
              For a century and a half Reach Command was the strongest force humanity had ever fielded. It had never had to learn
              what losing looks like. Then the Regency came to Meridia: black machines lit red from inside, and shields that hold
              against almost everything Command fires at them.
            </p>
          </div>
          <ol className="relative self-center">
            <span className="absolute bottom-3 left-[5px] top-3 w-px bg-gradient-to-b from-white/5 via-white/20 to-accent" />
            {TIMELINE.map((t, i) => (
              <li
                key={t.title}
                className="reveal relative grid grid-cols-[auto_1fr] gap-x-6 pb-9 last:pb-0"
                style={{ ["--i" as string]: i }}
              >
                <span
                  className={`relative z-10 mt-2 size-[11px] border ${t.hot ? "border-accent bg-accent shadow-[0_0_14px_2px_rgb(255_90_36/0.7)]" : "border-white/40 bg-void"}`}
                />
                <div>
                  <p className={`num font-display text-sm font-semibold ${t.hot ? "text-accent" : "text-faint"}`}>{t.when}</p>
                  <h3 className="mt-0.5 font-display text-2xl font-semibold">{t.title}</h3>
                  <p className="mt-1 text-dim">{t.text}</p>
                </div>
              </li>
            ))}
          </ol>
        </div>
      </section>

      {/* ---- How you fight ------------------------------------------------ */}
      <section className="mx-auto max-w-[1480px] px-4 py-20 sm:px-8">
        <header className="reveal flex flex-wrap items-end justify-between gap-4">
          <h2 className="font-display text-5xl font-light leading-none sm:text-6xl">How you fight</h2>
          <p className="max-w-md text-dim">
            Big battles, a real economy behind them, and a war you can see from a single tank up to the whole map.
          </p>
        </header>
        <ol className="mt-10 grid gap-3 md:grid-cols-2 xl:grid-cols-4">
          {PILLARS.map((p, i) => {
            const u = find(p.unit);
            return (
              <li
                key={p.title}
                className="reveal group relative overflow-hidden border border-white/[0.08] bg-white/[0.02] transition-colors duration-300 hover:border-white/20"
                style={{ ["--i" as string]: i }}
              >
                <span className="absolute inset-y-0 left-0 w-[3px] bg-white/20 transition-colors duration-300 group-hover:bg-accent" />
                <div className="relative aspect-[16/10] overflow-hidden border-b border-white/[0.06]">
                  {u && (
                    <UnitArt
                      portrait={u.portrait}
                      faction={u.faction}
                      className="h-full w-full opacity-85 transition-[scale,opacity] duration-700 ease-out group-hover:scale-110 group-hover:opacity-100"
                    />
                  )}
                  <kbd className="absolute left-4 top-4 grid size-7 place-items-center border border-white/25 bg-black/60 font-display text-sm font-semibold">
                    {i + 1}
                  </kbd>
                  {u && <span className="absolute bottom-3 right-4 font-display text-sm text-faint">{u.name}</span>}
                </div>
                <div className="p-5">
                  <h3 className="font-display text-2xl font-semibold">{p.title}</h3>
                  <p className="mt-2 leading-relaxed text-dim">{p.text}</p>
                </div>
              </li>
            );
          })}
        </ol>
      </section>

      {/* ---- Factions ----------------------------------------------------- */}
      <section className="mx-auto max-w-[1480px] px-4 py-20 sm:px-8">
        <div className="grid gap-3 lg:grid-cols-2">
          {factions.map((f) => {
            const regency = f.slug === "regency";
            return (
              <Link
                key={f.slug}
                href={`/factions/${f.slug}`}
                transitionTypes={["nav-forward"]}
                className="reveal group relative grid min-h-[420px] overflow-hidden border border-white/[0.08] p-7 transition-colors duration-500 hover:border-white/20 sm:grid-cols-[1fr_auto] sm:p-10"
              >
                <div
                  aria-hidden
                  className="absolute inset-0 opacity-60 transition-opacity duration-700 group-hover:opacity-100"
                  style={{
                    background: regency
                      ? "radial-gradient(80% 70% at 100% 0%, rgb(224 52 43 / 0.2), transparent 60%)"
                      : "radial-gradient(80% 70% at 100% 0%, rgb(255 90 36 / 0.14), transparent 60%)",
                  }}
                />
                <div className="relative flex flex-col">
                  <p
                    className="font-display text-sm font-semibold"
                    style={{ color: regency ? "var(--color-regency)" : "var(--color-accent)" }}
                  >
                    {regency ? "The enemy" : "Your side"}
                  </p>
                  <h3 className="mt-2 font-display text-4xl font-light leading-tight sm:text-5xl">{f.name}</h3>
                  {f.motto && <p className="mt-2 font-display text-xl text-dim">“{f.motto}”</p>}
                  <p className="mt-5 max-w-lg leading-relaxed text-dim">{f.about}</p>
                  <span className="mt-auto inline-flex items-center gap-2 pt-8 font-display text-lg font-semibold">
                    Read the file <span className="transition-transform duration-300 group-hover:translate-x-1.5">→</span>
                  </span>
                </div>
                <div
                  className="relative hidden w-44 self-center transition-transform duration-700 ease-out group-hover:scale-105 sm:block"
                  style={{ color: regency ? "var(--color-bronze)" : "var(--color-text)" }}
                >
                  {regency ? <RegencyCrest /> : <ArcCrest />}
                </div>
              </Link>
            );
          })}
        </div>
      </section>

      {/* ---- Featured units ----------------------------------------------- */}
      <section className="py-20">
        <header className="reveal mx-auto flex max-w-[1480px] flex-wrap items-end justify-between gap-4 px-4 sm:px-8">
          <div>
            <p className="font-display text-sm font-semibold text-accent">Unit directory</p>
            <h2 className="mt-2 font-display text-5xl font-light leading-none sm:text-6xl">From brick to titan</h2>
          </div>
          <Link
            href="/units"
            transitionTypes={["nav-forward"]}
            className="group font-display text-lg font-semibold text-dim transition hover:text-text"
          >
            All {units.length} units <span className="inline-block transition-transform group-hover:translate-x-1">→</span>
          </Link>
        </header>
        <ul className="rail mt-8 flex snap-x snap-mandatory gap-3 overflow-x-auto pb-4 [scrollbar-width:thin]">
          {featured.map((u) => (
            <li key={`${u.faction}-${u.slug}`} className="w-[250px] shrink-0 snap-start sm:w-[280px]">
              <UnitTile unit={u} />
            </li>
          ))}
        </ul>
      </section>

      {/* ---- Join --------------------------------------------------------- */}
      <section className="relative overflow-hidden border-t border-white/[0.08]">
        <div aria-hidden className="absolute inset-y-0 left-1/2 w-px bg-accent shadow-[0_0_40px_6px_rgb(255_90_36/0.35)]" />
        <div className="relative mx-auto grid max-w-[1480px] gap-10 px-4 py-24 sm:px-8 md:grid-cols-2 md:py-32">
          <h2 className="reveal font-display text-5xl font-light leading-[1] sm:text-6xl">
            Help test
            <br />
            what comes next
          </h2>
          <div className="reveal md:pl-16">
            <p className="text-lg leading-relaxed text-dim">
              Meridian Conflict is in development. Meet players, share ideas and tell us what breaks. Insiders help test early
              builds.
            </p>
            <a
              href={site.discord}
              target="_blank"
              rel="noreferrer"
              className="group mt-8 inline-flex items-center gap-3 bg-text px-6 py-3.5 font-display text-lg font-semibold text-black transition hover:bg-white active:scale-[0.98]"
            >
              Join the Discord <span className="transition-transform duration-300 group-hover:translate-x-1">↗</span>
            </a>
          </div>
        </div>
      </section>
    </>
  );
}
