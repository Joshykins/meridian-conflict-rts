import type { Metadata } from "next";
import { PlaytestBadge, SetupDownload } from "@/components/download-link";
import site from "../../../content/site.json";

export const metadata: Metadata = {
  title: "Play",
  description:
    "Download Meridian Conflict for Windows: a small setup installs the launcher, which downloads the game and keeps it up to date. Free during the open playtest.",
};

const steps = [
  {
    title: "Download the setup",
    body: "A small file, MeridianConflictSetup. It is the launcher itself; the game comes next.",
  },
  {
    title: "Run it",
    body: "It installs for you alone, with no administrator prompt, and puts Meridian Conflict in the Start menu and on the desktop. Windows may stop it first with “Windows protected your PC”: choose More info, then Run anyway.",
  },
  {
    title: "Let the launcher fetch the game",
    body: "The launcher opens and downloads the newest playtest build, about 250 MB. Press Play when it is ready. From then on it updates the game before you play.",
  },
];

const needs = [
  ["System", site.download.platform + ", 64-bit"],
  ["Graphics", "A card with Vulkan and a current driver: NVIDIA, AMD or Intel"],
  ["Disk", "About 1 GB, more as old builds are kept for their replays"],
  ["Online play", "The playtest server is found by itself; games on your own network need nothing"],
];

export default function PlayPage() {
  return (
    <div className="relative overflow-hidden">
      <div
        aria-hidden
        className="pointer-events-none absolute inset-x-0 top-0 h-[90vh]"
        style={{
          background:
            "radial-gradient(55% 45% at 78% 18%, rgb(255 90 36 / 0.16), transparent 70%), radial-gradient(40% 40% at 15% 5%, rgb(255 255 255 / 0.05), transparent 70%)",
        }}
      />
      <div className="relative mx-auto max-w-[1480px] px-4 pb-24 pt-[calc(var(--header-h)+3.5rem)] sm:px-8">
        <p className="rise flex items-center gap-3 font-display text-base font-semibold text-dim" style={{ ["--i" as string]: 0 }}>
          <span className="h-px w-10 bg-accent" />
          <PlaytestBadge className="text-accent" />
        </p>
        <h1
          className="rise mt-4 font-display text-[clamp(3rem,8vw,6.5rem)] font-semibold leading-[0.9]"
          style={{ ["--i" as string]: 1 }}
        >
          Play <span className="font-extralight text-text/75">Meridian Conflict</span>
        </h1>
        <p className="rise mt-6 max-w-2xl text-lg leading-relaxed text-text/85 sm:text-xl" style={{ ["--i" as string]: 2 }}>
          Free during the open playtest. A small setup installs the launcher, the launcher downloads the game, and from
          then on it keeps the game up to date before you play.
        </p>
        <div className="rise mt-9 flex flex-wrap items-center gap-4" style={{ ["--i" as string]: 3 }}>
          <SetupDownload className="group relative inline-flex items-center gap-3 overflow-hidden bg-accent px-7 py-4 font-display text-xl font-semibold text-black shadow-[0_0_36px_-6px_rgb(255_90_36/0.85)] transition hover:brightness-110 active:scale-[0.98]" />
          <a
            href={site.discord}
            target="_blank"
            rel="noreferrer"
            className="glass inline-flex items-center gap-3 px-6 py-4 font-display text-lg font-semibold transition hover:border-white/30"
          >
            Join the Discord ↗
          </a>
        </div>
        <p className="rise mt-4 text-sm text-faint" style={{ ["--i" as string]: 4 }}>
          {site.download.platform} · Playtest channel · The setup is a few MB; the game about 250 MB
        </p>

        <section className="mt-20 grid gap-4 md:grid-cols-3">
          {steps.map((s, i) => (
            <div key={s.title} className="reveal glass relative p-6 sm:p-7">
              <span className="num font-display text-5xl font-extralight text-accent/80">{String(i + 1).padStart(2, "0")}</span>
              <h2 className="mt-3 font-display text-2xl font-semibold">{s.title}</h2>
              <p className="mt-3 leading-relaxed text-text/80">{s.body}</p>
            </div>
          ))}
        </section>

        <section className="mt-16 grid gap-10 lg:grid-cols-[1fr_1fr]">
          <div className="reveal">
            <h2 className="flex items-center gap-3 font-display text-lg font-semibold text-dim">
              <span className="h-px w-8 bg-accent" />
              What you need
            </h2>
            <dl className="mt-5 divide-y divide-white/[0.08] border-y border-white/[0.08]">
              {needs.map(([k, v]) => (
                <div key={k} className="grid grid-cols-[9rem_1fr] gap-4 py-4">
                  <dt className="font-display font-semibold text-dim">{k}</dt>
                  <dd className="text-text/85">{v}</dd>
                </div>
              ))}
            </dl>
          </div>
          <div className="reveal">
            <h2 className="flex items-center gap-3 font-display text-lg font-semibold text-dim">
              <span className="h-px w-8 bg-accent" />
              Good to know
            </h2>
            <ul className="mt-5 space-y-4 leading-relaxed text-text/80">
              <li>
                <span className="font-semibold text-text">It is a playtest.</span> Everything is work in progress, and
                things break. Tell us on Discord what you find.
              </li>
              <li>
                <span className="font-semibold text-text">If the game crashes,</span> it writes a report. In the
                launcher, Builds then Logs and Crash Reports opens the folder: send us the newest one.
              </li>
              <li>
                <span className="font-semibold text-text">To uninstall,</span> use Settings, Apps, Meridian Conflict.
                Your replays are kept.
              </li>
            </ul>
          </div>
        </section>
      </div>
    </div>
  );
}
