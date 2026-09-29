"use client";

import { useEffect, useMemo, useRef, useSyncExternalStore } from "react";
import { UnitTile } from "./unit-tile";
import { DOMAIN_COLOR } from "./unit-bits";
import { DOMAINS, type Domain, type UnitSummary } from "@/lib/shared";

type Filter = {
  faction: "all" | "arc" | "regency";
  domain: Domain | "all";
  tech: number;
  q: string;
  sort: "tech" | "name" | "health" | "dps";
};
const EMPTY: Filter = { faction: "all", domain: "all", tech: 0, q: "", sort: "tech" };

// The directory's filters live in the URL (so a filtered view can be shared)
// but the page itself is one static render of every unit: a filter is a
// transition over the same tiles, which slide to their new places.

function parse(search: string): Filter {
  const p = new URLSearchParams(search);
  const f = { ...EMPTY };
  const fac = p.get("faction");
  if (fac === "arc" || fac === "regency") f.faction = fac;
  const d = p.get("domain");
  if (d && DOMAINS.some((x) => x.key === d)) f.domain = d as Domain;
  const t = Number(p.get("tech"));
  if (t >= 1 && t <= 5) f.tech = t;
  f.q = p.get("q") ?? "";
  const s = p.get("sort");
  if (s === "name" || s === "health" || s === "dps") f.sort = s;
  return f;
}

function writeUrl(f: Filter) {
  const p = new URLSearchParams();
  if (f.faction !== "all") p.set("faction", f.faction);
  if (f.domain !== "all") p.set("domain", f.domain);
  if (f.tech) p.set("tech", String(f.tech));
  if (f.q) p.set("q", f.q);
  if (f.sort !== "tech") p.set("sort", f.sort);
  const qs = p.toString();
  window.history.replaceState(window.history.state, "", qs ? `?${qs}` : window.location.pathname);
  for (const l of listeners) l();
}

// The URL's query is the store: the server renders every unit, the client reads the filter from it.
const listeners = new Set<() => void>();
function subscribe(l: () => void) {
  listeners.add(l);
  window.addEventListener("popstate", l);
  return () => {
    listeners.delete(l);
    window.removeEventListener("popstate", l);
  };
}

export function UnitBrowser({ units }: { units: UnitSummary[] }) {
  const query = useSyncExternalStore(
    subscribe,
    () => window.location.search,
    () => "",
  );
  const f = useMemo(() => parse(query), [query]);
  const search = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "/" && document.activeElement?.tagName !== "INPUT") {
        e.preventDefault();
        search.current?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const set = (patch: Partial<Filter>) => writeUrl({ ...f, ...patch });

  const base = useMemo(() => {
    const q = f.q.trim().toLowerCase();
    return units.filter(
      (u) =>
        (f.faction === "all" || u.faction === f.faction) &&
        (!f.tech || u.tech === f.tech) &&
        (!q || u.name.toLowerCase().includes(q) || u.role.toLowerCase().includes(q) || (u.lore ?? "").toLowerCase().includes(q)),
    );
  }, [units, f.faction, f.tech, f.q]);

  const shown = useMemo(() => {
    const list = base.filter((u) => f.domain === "all" || u.domain === f.domain);
    const by: Record<Filter["sort"], (a: UnitSummary, b: UnitSummary) => number> = {
      tech: () => 0,
      name: (a, b) => a.name.localeCompare(b.name),
      health: (a, b) => b.health - a.health,
      dps: (a, b) => (b.dps ?? 0) - (a.dps ?? 0),
    };
    return [...list].sort(by[f.sort]);
  }, [base, f.domain, f.sort]);

  const groups = useMemo(() => {
    if (f.sort !== "tech") return [{ key: "all" as const, label: `${shown.length} units`, units: shown }];
    return DOMAINS.map((d) => ({ key: d.key, label: d.plural, units: shown.filter((u) => u.domain === d.key) })).filter(
      (g) => g.units.length,
    );
  }, [shown, f.sort]);

  const count = (d: Domain | "all") => (d === "all" ? base.length : base.filter((u) => u.domain === d).length);
  const dirty = JSON.stringify(f) !== JSON.stringify(EMPTY);

  return (
    <div>
      {/* Filter bar, pinned under the header. */}
      <div className="sticky top-[var(--header-h)] z-30 -mx-4 border-y border-white/10 bg-black/70 px-4 py-3 backdrop-blur-xl sm:-mx-8 sm:px-8">
        <div className="flex flex-wrap items-center gap-x-5 gap-y-3">
          <Segmented
            value={f.faction}
            onChange={(v) => set({ faction: v as Filter["faction"] })}
            options={[
              { value: "all", label: "All" },
              { value: "arc", label: "ARC" },
              { value: "regency", label: "Regency" },
            ]}
          />
          <Segmented
            value={String(f.tech)}
            onChange={(v) => set({ tech: Number(v) })}
            options={[{ value: "0", label: "Any tech" }, ...[1, 2, 3, 4, 5].map((t) => ({ value: String(t), label: `T${t}` }))]}
          />
          <label className="relative ml-auto flex min-w-[220px] flex-1 items-center sm:max-w-xs">
            <svg viewBox="0 0 16 16" className="pointer-events-none absolute left-3 size-4 text-faint" aria-hidden>
              <circle cx="7" cy="7" r="4.5" fill="none" stroke="currentColor" strokeWidth="1.5" />
              <path d="M10.5 10.5L14 14" stroke="currentColor" strokeWidth="1.5" />
            </svg>
            <input
              ref={search}
              value={f.q}
              onChange={(e) => set({ q: e.target.value })}
              placeholder="Search units"
              aria-label="Search units"
              className="h-10 w-full border border-white/10 bg-white/[0.04] pl-9 pr-10 text-[0.95rem] text-text placeholder:text-faint focus:border-accent/70 focus:bg-white/[0.06] focus:outline-none"
            />
            <kbd className="pointer-events-none absolute right-2.5 border border-white/15 px-1.5 font-display text-xs text-faint">
              /
            </kbd>
          </label>
        </div>
        <div className="mt-3 flex items-center gap-2 overflow-x-auto pb-0.5 [scrollbar-width:none]">
          <Chip active={f.domain === "all"} onClick={() => set({ domain: "all" })} count={count("all")}>
            Everything
          </Chip>
          {DOMAINS.map((d) => (
            <Chip
              key={d.key}
              active={f.domain === d.key}
              onClick={() => set({ domain: d.key })}
              color={DOMAIN_COLOR[d.key]}
              count={count(d.key)}
            >
              {d.plural}
            </Chip>
          ))}
          <span className="ml-auto flex shrink-0 items-center gap-2 pl-4 text-sm text-faint">
            Sort
            <select
              value={f.sort}
              onChange={(e) => set({ sort: e.target.value as Filter["sort"] })}
              className="h-8 border border-white/10 bg-black px-2 font-display font-semibold text-text focus:border-accent/70 focus:outline-none"
            >
              <option value="tech">By type</option>
              <option value="name">Name</option>
              <option value="health">Health</option>
              <option value="dps">Damage</option>
            </select>
          </span>
        </div>
      </div>

      {/* A new filter re-deals the grid: tiles rise in, staggered across the first rows. */}
      <div key={`${f.faction}|${f.domain}|${f.tech}|${f.sort}`} className="mt-8 space-y-12">
        {groups.map((g, gi) => (
          <section key={g.key}>
            <header
              className="rise mb-4 flex items-baseline gap-3 border-b border-white/[0.07] pb-2"
              style={{ ["--i" as string]: gi }}
            >
              {g.key !== "all" && <span className="size-2" style={{ background: DOMAIN_COLOR[g.key] }} />}
              <h2 className="font-display text-2xl font-light">{g.label}</h2>
              {g.key !== "all" && <span className="num font-display text-sm text-faint">{g.units.length}</span>}
            </header>
            <ul className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
              {g.units.map((u, i) => (
                <li
                  key={`${u.faction}-${u.slug}`}
                  className={`[contain-intrinsic-size:auto_330px] [content-visibility:auto] ${gi < 2 && i < 15 ? "deal" : ""}`}
                  style={{ ["--i" as string]: gi * 3 + i }}
                >
                  <UnitTile unit={u} />
                </li>
              ))}
            </ul>
          </section>
        ))}
        {!groups.length && (
          <div className="ticks flex flex-col items-center gap-4 px-6 py-24 text-center">
            <p className="font-display text-3xl font-light">No units match</p>
            <p className="text-dim">Nothing on file fits every filter. Loosen one, or start again.</p>
            <button
              onClick={() => set(EMPTY)}
              className="mt-2 border border-accent/60 px-4 py-2 font-display font-semibold text-accent transition hover:bg-accent hover:text-black"
            >
              Clear filters
            </button>
          </div>
        )}
      </div>
      {dirty && groups.length > 0 && (
        <button
          onClick={() => set(EMPTY)}
          className="fixed bottom-5 left-1/2 z-30 -translate-x-1/2 border border-white/15 bg-black/80 px-4 py-2 font-display text-sm font-semibold text-dim backdrop-blur-xl transition hover:border-accent/60 hover:text-text"
        >
          Showing {shown.length} of {units.length} · Clear filters
        </button>
      )}
    </div>
  );
}

function Segmented({
  value,
  onChange,
  options,
}: {
  value: string;
  onChange: (v: string) => void;
  options: { value: string; label: string }[];
}) {
  return (
    <div className="flex border border-white/10 bg-white/[0.03] p-0.5" role="radiogroup">
      {options.map((o) => {
        const on = o.value === value;
        return (
          <button
            key={o.value}
            role="radio"
            aria-checked={on}
            onClick={() => onChange(o.value)}
            className={`relative px-3 py-1.5 font-display text-sm font-semibold transition-colors duration-200 ${on ? "bg-white text-black" : "text-dim hover:text-text"}`}
          >
            {o.label}
          </button>
        );
      })}
    </div>
  );
}

function Chip({
  active,
  onClick,
  color,
  count,
  children,
}: {
  active: boolean;
  onClick: () => void;
  color?: string;
  count: number;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      aria-pressed={active}
      disabled={!count && !active}
      className={`group relative flex shrink-0 items-center gap-2 border px-3 py-1.5 font-display text-sm font-semibold transition-colors duration-200 disabled:opacity-30 ${
        active
          ? "border-white/40 bg-white/[0.08] text-text"
          : "border-white/[0.08] text-dim hover:border-white/20 hover:text-text"
      }`}
    >
      {color && <span className="size-1.5" style={{ background: color }} />}
      {children}
      <span className="num text-xs text-faint">{count}</span>
      <span
        className={`absolute inset-x-0 -bottom-px h-[2px] bg-accent transition-transform duration-300 ${active ? "scale-x-100" : "scale-x-0"}`}
      />
    </button>
  );
}
