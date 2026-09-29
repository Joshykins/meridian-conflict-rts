import type { Domain } from "@/lib/shared";

/** Tile colours by domain, as the in-game build strip codes them. */
export const DOMAIN_COLOR: Record<Domain, string> = {
  command: "var(--color-command)",
  land: "var(--color-land)",
  air: "var(--color-air)",
  naval: "var(--color-naval)",
  space: "var(--color-space)",
  experimental: "var(--color-titan)",
  structure: "var(--color-structure)",
};

export function TechPips({ tech, className = "" }: { tech: number; className?: string }) {
  return (
    <span className={`inline-flex items-center gap-1.5 ${className}`} aria-label={`Tech ${tech}`}>
      <span className="num font-display text-xs font-semibold text-dim">T{tech}</span>
      <span className="flex gap-[2px]">
        {[1, 2, 3, 4, 5].map((t) => (
          <span
            key={t}
            className="h-2.5 w-[3px]"
            style={{
              background: t <= tech ? (tech >= 4 ? "var(--color-titan)" : "var(--color-text)") : "rgb(255 255 255 / 0.12)",
            }}
          />
        ))}
      </span>
    </span>
  );
}

export function DomainTag({ domain, label }: { domain: Domain; label: string }) {
  return (
    <span className="inline-flex items-center gap-1.5 font-display text-xs font-semibold text-dim">
      <span className="size-1.5" style={{ background: DOMAIN_COLOR[domain] }} />
      {label}
    </span>
  );
}
