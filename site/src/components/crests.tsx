// Simplified faction marks for the web, after the in-game crests
// (crates/mc-game/src/ui/emblem). ARC: an eagle's raised wings over a shield,
// a star, Asteria's horizon. Regency: ARC's drawing of a crown cut down to
// blades, over two broken rings open at the top.

export function ArcCrest({ className = "" }: { className?: string }) {
  return (
    <svg viewBox="0 0 200 200" className={className} aria-hidden fill="none" stroke="currentColor" strokeWidth="2">
      <path d="M100 150c-26-10-38-30-38-58V62l38-12 38 12v30c0 28-12 48-38 58z" fill="rgb(255 255 255 / 0.04)" />
      <path d="M100 62v76M78 92h44" strokeOpacity=".4" />
      {/* Wings, raised: one for Earth, one for Asteria. */}
      {[0, 1, 2, 3, 4].map((i) => (
        <g key={i} fill="rgb(242 242 240 / 0.9)" stroke="none">
          <path d={`M66 ${86 - i * 2}L${20 + i * 7} ${38 + i * 11}L${27 + i * 7} ${43 + i * 11}L66 ${92 - i * 2}Z`} />
          <path d={`M134 ${86 - i * 2}L${180 - i * 7} ${38 + i * 11}L${173 - i * 7} ${43 + i * 11}L134 ${92 - i * 2}Z`} />
        </g>
      ))}
      <path d="M100 22l3.5 8.5 9 .6-7 5.8 2.3 8.8-7.8-4.9-7.8 4.9 2.3-8.8-7-5.8 9-.6z" fill="var(--color-accent)" stroke="none" />
      <path d="M52 170c30-14 66-14 96 0" stroke="var(--color-air)" strokeOpacity=".8" />
      <path d="M62 180h76" strokeOpacity=".35" />
    </svg>
  );
}

export function RegencyCrest({ className = "" }: { className?: string }) {
  return (
    <svg viewBox="0 0 200 200" className={className} aria-hidden fill="none" stroke="currentColor" strokeWidth="2">
      {/* Two horizons, both open at the top. */}
      <path d="M66.7 57.5A58 58 0 1 0 133.3 57.5" strokeOpacity=".55" />
      <path d="M56.4 42.7A76 76 0 1 0 143.6 42.7" strokeOpacity=".25" />
      {/* Blades, swept like the plates on their machines. */}
      {[-2, -1, 0, 1, 2].map((i) => {
        const x = 100 + i * 22;
        const h = 70 - Math.abs(i) * 14;
        return (
          <path
            key={i}
            d={`M${x - 7} 130L${x + i * 4} ${130 - h}L${x + 7} 130Z`}
            fill={i === 0 ? "var(--color-regency)" : "rgb(154 116 71 / 0.25)"}
            stroke={i === 0 ? "none" : "var(--color-bronze)"}
          />
        );
      })}
      <path d="M58 136h84" stroke="var(--color-bronze)" strokeWidth="3" />
      <circle cx="100" cy="112" r="3.5" fill="#ff8a70" stroke="none" />
    </svg>
  );
}
