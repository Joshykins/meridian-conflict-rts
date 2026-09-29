/** The game icon: a steel M split by the orange meridian. */
export function Emblem({ className = "", glow = true }: { className?: string; glow?: boolean }) {
  return (
    <svg viewBox="0 0 64 64" className={className} aria-hidden>
      <defs>
        <linearGradient id="em-steel" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#f4f5f7" />
          <stop offset="1" stopColor="#9aa0a8" />
        </linearGradient>
        <radialGradient id="em-glow" cx=".5" cy=".85" r=".6">
          <stop offset="0" stopColor="#ff5a24" stopOpacity=".45" />
          <stop offset="1" stopColor="#ff5a24" stopOpacity="0" />
        </radialGradient>
      </defs>
      <rect x="1" y="1" width="62" height="62" rx="14" fill="#16171b" stroke="#2c2e33" />
      {glow && <rect x="1" y="1" width="62" height="62" rx="14" fill="url(#em-glow)" />}
      <path d="M13 48V16h9l10 16 10-16h9v32h-8.5V30.5L32 42l-10-11.5V48z" fill="url(#em-steel)" />
      <rect x="30.6" y="6" width="2.8" height="52" fill="#ff5a24" />
      {glow && <rect x="29.4" y="6" width="5.2" height="52" fill="#ff5a24" opacity=".25" />}
    </svg>
  );
}

/** "Meridian" bold over "Conflict" light, as the in-game brand panel sets it. */
export function Wordmark({ className = "" }: { className?: string }) {
  return (
    <span className={`font-display leading-none ${className}`}>
      <span className="font-semibold">Meridian</span> <span className="font-light text-dim">Conflict</span>
    </span>
  );
}
