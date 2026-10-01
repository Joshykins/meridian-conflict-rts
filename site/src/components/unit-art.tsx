import type { FactionSlug } from "@/lib/shared";

// A unit's portrait: the game's own model of it, cut out by the game's portrait
// rasteriser at build time (scripts/models.mjs), standing in a pool of light so dark
// plate reads against the page.
export function UnitArt({
  portrait,
  faction,
  className = "",
  eager = false,
}: {
  portrait: string | null;
  faction: FactionSlug;
  className?: string;
  eager?: boolean;
}) {
  return (
    <div className={`stage ${faction === "regency" ? "stage-regency" : ""} ${className}`}>
      {portrait && (
        // Already sized and packed as WebP by the build: nothing for an image optimiser to do.
        // eslint-disable-next-line @next/next/no-img-element
        <img
          src={portrait}
          alt=""
          width={512}
          height={512}
          loading={eager ? "eager" : "lazy"}
          decoding="async"
          className="h-full w-full object-contain"
        />
      )}
    </div>
  );
}
