// A stand-in for a unit render until real captures exist: a plan-view
// schematic drawn from the unit's icon class, tech and a seed from its slug,
// so every unit gets its own drawing and the same one on every page.
// Labelled as a schematic wherever it is shown large.

type Props = {
  slug: string;
  icon: string | null;
  domain: string;
  tech: number;
  faction: string;
  className?: string;
  detail?: boolean;
};

function rng(seed: string) {
  let h = 2166136261;
  for (let i = 0; i < seed.length; i++) h = Math.imul(h ^ seed.charCodeAt(i), 16777619);
  return () => {
    h = Math.imul(h ^ (h >>> 15), 2246822507);
    h = Math.imul(h ^ (h >>> 13), 3266489909);
    h ^= h >>> 16;
    return (h >>> 0) / 4294967296;
  };
}

const r1 = (n: number) => Math.round(n * 10) / 10;

function kindOf(icon: string | null, domain: string) {
  switch (icon) {
    case "Tank":
    case "Artillery":
    case "AntiAir":
    case "Scout":
    case "Salvage":
      return domain === "land" ? "tracked" : domain === "naval" ? "ship" : "structure-defense";
    case "Engineer":
      return domain === "air" ? "plane" : "tracked";
    case "Bot":
    case "Commander":
    case "Titan":
      return "walker";
    case "Fighter":
    case "Bomber":
    case "TorpedoBomber":
      return "plane";
    case "Gunship":
    case "Transport":
    case "SalvageDrone":
      return domain === "space" ? "spacecraft" : "rotor";
    case "Ship":
    case "Warship":
    case "SalvageBoat":
    case "SensorShip":
      return domain === "space" ? "spacecraft" : "ship";
    case "Submarine":
      return "sub";
    case "SalvageCarrier":
      return domain === "air" ? "plane" : "tracked";
    case "Factory":
      return "factory";
    case "Power":
      return "reactor";
    case "Extractor":
      return "extractor";
    case "Shield":
    case "Damper":
      return "shield";
    case "Intel":
      return domain === "air" ? "plane" : domain === "naval" ? "ship" : domain === "space" ? "spacecraft" : "radar";
    case "Storage":
      return "storage";
    case "Wall":
      return "wall";
    case "Silo":
    case "AntiNuke":
      return "silo";
    case "Defense":
      return "structure-defense";
  }
  if (domain === "air") return "plane";
  if (domain === "naval") return "ship";
  if (domain === "space") return "spacecraft";
  if (domain === "structure") return "factory";
  return "tracked";
}

export function UnitSchematic({ slug, icon, domain, tech, faction, className = "", detail = false }: Props) {
  const rand = rng(`${faction}/${slug}`);
  const kind = kindOf(icon, domain);
  const hot = faction === "regency" ? "#e0342b" : "#ff5a24";
  const metal = faction === "regency" ? "#9a7447" : "#f2f2f0";
  // Each kind is drawn at its own size; this brings them to one frame, a little bigger by tier.
  const KIND_SCALE: Record<string, number> = {
    tracked: 1.45,
    walker: 1.4,
    plane: 1.05,
    rotor: 1.3,
    ship: 0.78,
    sub: 0.8,
    spacecraft: 0.8,
    factory: 1.05,
    reactor: 1.1,
    extractor: 1.05,
    shield: 0.95,
    radar: 1.2,
    storage: 1.15,
    wall: 1.05,
    silo: 1.2,
  };
  const scale = (KIND_SCALE[kind] ?? 1.3) * (0.86 + Math.min(tech, 5) * 0.035);
  const s = { stroke: metal, strokeWidth: 1.1, fill: "none", vectorEffect: "non-scaling-stroke" as const };
  const fillPlate = faction === "regency" ? "rgba(154,116,71,0.08)" : "rgba(242,242,240,0.05)";
  const p = { ...s, fill: fillPlate };
  const gun = {
    stroke: hot,
    strokeWidth: 1.6,
    fill: "none",
    vectorEffect: "non-scaling-stroke" as const,
    strokeLinecap: "round" as const,
  };

  const body = drawBody(kind, rand, tech, { s, p, gun, hot });

  return (
    <svg viewBox="-100 -100 200 200" className={`blueprint ${className}`} role="img" aria-label="Schematic">
      {/* Range rings and a heading line, as the in-game selection draws them. */}
      <circle r="86" fill="none" stroke="rgba(255,255,255,0.07)" strokeDasharray="1 3" />
      <circle r="62" fill="none" stroke="rgba(255,255,255,0.05)" />
      <path d="M-96 0H-80M80 0H96M0 -96V-80M0 80V96" stroke="rgba(255,255,255,0.25)" />
      <g
        transform={`scale(${r1(scale)}) rotate(${kind.startsWith("structure") || kind === "factory" || kind === "reactor" || kind === "extractor" || kind === "storage" || kind === "radar" || kind === "silo" || kind === "shield" || kind === "wall" ? 0 : -90})`}
      >
        {body}
      </g>
      {detail && (
        <g fontFamily="var(--font-barlow-sc)" fontSize="6" fill="rgba(255,255,255,0.45)">
          <path d="M-70 78H70M-70 75V81M70 75V81" stroke="rgba(255,255,255,0.3)" strokeWidth="0.6" />
          <text x="0" y="88" textAnchor="middle">
            {Math.round(scale * 140)} u
          </text>
        </g>
      )}
    </svg>
  );
}

type Pens = {
  s: React.SVGProps<SVGElement>;
  p: React.SVGProps<SVGElement>;
  gun: React.SVGProps<SVGElement>;
  hot: string;
};

function drawBody(kind: string, rand: () => number, tech: number, { s, p, gun, hot }: Pens) {
  const S = s as React.SVGProps<SVGPathElement>;
  const P = p as React.SVGProps<SVGPathElement>;
  const G = gun as React.SVGProps<SVGPathElement>;
  const lit = tech >= 2; // Emitters are earned: nothing on tech 1 glows.
  const glow = lit ? { fill: hot, opacity: 0.85 } : { fill: "none", stroke: "rgba(255,255,255,0.35)" };

  switch (kind) {
    case "tracked": {
      const L = 46 + rand() * 18;
      const W = 30 + rand() * 10;
      const tw = 7 + rand() * 3;
      const tr = 10 + rand() * 6;
      const barrel = 26 + rand() * 30;
      const twin = rand() > 0.72;
      const treads = [];
      for (let x = -L / 2 + 3; x < L / 2; x += 4)
        treads.push(`M${r1(x)} ${r1(-W / 2)}v${r1(tw)}M${r1(x)} ${r1(W / 2)}v${r1(-tw)}`);
      return (
        <>
          <rect x={-L / 2} y={-W / 2} width={L} height={tw} {...(P as object)} />
          <rect x={-L / 2} y={W / 2 - tw} width={L} height={tw} {...(P as object)} />
          <path d={treads.join("")} {...S} strokeOpacity={0.35} />
          <path
            d={`M${-L / 2 + 4} ${-W / 2 + tw}H${L / 2 - 2}L${L / 2 + 4} 0L${L / 2 - 2} ${W / 2 - tw}H${-L / 2 + 4}Z`}
            {...P}
          />
          <path d={`M${-L / 2 + 8} ${-W / 4}H${-L / 2 + 16}M${-L / 2 + 8} ${W / 4}H${-L / 2 + 16}`} {...S} strokeOpacity={0.5} />
          {twin ? (
            <path d={`M${tr * 0.6} -3H${tr + barrel}M${tr * 0.6} 3H${tr + barrel}`} {...G} />
          ) : (
            <path d={`M${tr * 0.6} 0H${tr + barrel}`} {...G} strokeWidth={2.4} />
          )}
          <path d={`M${-tr} ${-tr * 0.8}L${tr * 0.4} ${-tr}L${tr} 0L${tr * 0.4} ${tr}L${-tr} ${tr * 0.8}Z`} {...P} />
          <circle r={2} {...(glow as object)} />
        </>
      );
    }
    case "walker": {
      const big = tech >= 3;
      const sw = 22 + rand() * 8 + (big ? 10 : 0);
      const leg = 16 + rand() * 6 + (big ? 8 : 0);
      return (
        <>
          <path d={`M${-leg} ${-sw - 8}l10 -6h14l6 6v10h-30z`} {...P} />
          <path d={`M${-leg} ${sw + 8}l10 6h14l6 -6v-10h-30z`} {...P} />
          <path d={`M-10 ${-sw * 0.6}L-${leg} ${-sw - 4}M-10 ${sw * 0.6}L-${leg} ${sw + 4}`} {...S} />
          <path d={`M-16 -${sw * 0.7}L12 -${sw * 0.8}L22 -8V8L12 ${sw * 0.8}L-16 ${sw * 0.7}L-22 0Z`} {...P} />
          <rect x={-4} y={-sw - 6} width={30} height={10} {...(P as object)} />
          <rect x={-4} y={sw - 4} width={30} height={10} {...(P as object)} />
          <path d={`M26 ${-sw - 1}H${56 + rand() * 20}M26 ${sw + 1}H${50 + rand() * 20}`} {...G} strokeWidth={2.2} />
          <path d="M-6 -8H10L14 0L10 8H-6Z" {...S} />
          <circle cx={4} r={2.4} {...(glow as object)} />
        </>
      );
    }
    case "plane": {
      const L = 70 + rand() * 30;
      const span = 60 + rand() * 50;
      const sweep = 10 + rand() * 26;
      const tail = rand() > 0.5;
      return (
        <>
          <path d={`M${L / 2} 0L${L / 2 - 14} -4H${-L / 2 + 6}L${-L / 2} 0L${-L / 2 + 6} 4H${L / 2 - 14}Z`} {...P} />
          <path d={`M${8} -4L${8 - sweep} ${-span / 2}H${-4 - sweep} L${-14} -4Z`} {...P} />
          <path d={`M${8} 4L${8 - sweep} ${span / 2}H${-4 - sweep} L${-14} 4Z`} {...P} />
          {tail ? (
            <path
              d={`M${-L / 2 + 12} -3L${-L / 2 + 2} -18H${-L / 2 - 4}L${-L / 2 + 2} -3ZM${-L / 2 + 12} 3L${-L / 2 + 2} 18H${-L / 2 - 4}L${-L / 2 + 2} 3Z`}
              {...P}
            />
          ) : (
            <path d={`M${-L / 2 + 4} -3L${-L / 2 - 6} -12M${-L / 2 + 4} 3L${-L / 2 - 6} 12`} {...S} />
          )}
          <path d={`M${L / 2 - 10} -1.5H${L / 2 + 12}`} {...G} />
          <path
            d={`M${L / 2 - 20} -2.4L${L / 2 - 12} 0L${L / 2 - 20} 2.4Z`}
            fill={lit ? hot : "none"}
            stroke={lit ? "none" : "rgba(255,255,255,.4)"}
          />
          <path
            d={`M${-L / 2 - 2} -2H${-L / 2 - 10}M${-L / 2 - 2} 2H${-L / 2 - 10}`}
            stroke={hot}
            strokeOpacity={lit ? 0.9 : 0.3}
            strokeWidth={1.4}
          />
        </>
      );
    }
    case "rotor": {
      const L = 60 + rand() * 20;
      const pod = 22 + rand() * 12;
      return (
        <>
          <path d={`M${L / 2} 0L${L / 2 - 16} -8H${-L / 2 + 4}L${-L / 2 - 4} 0L${-L / 2 + 4} 8H${L / 2 - 16}Z`} {...P} />
          <path d={`M-6 -8V-${pod}M-6 8V${pod}`} {...S} />
          <circle cx={-6} cy={-pod - 7} r={9} {...(P as object)} />
          <circle cx={-6} cy={pod + 7} r={9} {...(P as object)} />
          <circle cx={-6} cy={-pod - 7} r={16} {...(S as object)} strokeDasharray="2 3" strokeOpacity={0.4} />
          <circle cx={-6} cy={pod + 7} r={16} {...(S as object)} strokeDasharray="2 3" strokeOpacity={0.4} />
          <path d={`M${L / 2 - 6} 0H${L / 2 + 16}`} {...G} />
          <circle cx={L / 2 - 18} r={2.2} {...(glow as object)} />
        </>
      );
    }
    case "ship":
    case "sub":
    case "spacecraft": {
      const sub = kind === "sub";
      const space = kind === "spacecraft";
      const L = 110 + rand() * 50 + tech * 8;
      const B = sub ? 16 : 26 + rand() * 10 + tech * 2;
      const bow = sub ? L * 0.18 : L * (0.28 + rand() * 0.1);
      const hull = space
        ? `M${L / 2} 0L${L / 2 - bow} ${-B / 2}L${-L / 2 + 8} ${-B / 2 - 6}L${-L / 2} ${-B / 3}V${B / 3}L${-L / 2 + 8} ${B / 2 + 6}L${L / 2 - bow} ${B / 2}Z`
        : sub
          ? `M${L / 2} 0C${L / 2} -8 ${L / 2 - bow} ${-B / 2} ${L / 2 - bow - 6} ${-B / 2}H${-L / 2 + 10}C${-L / 2} ${-B / 2} ${-L / 2} ${B / 2} ${-L / 2 + 10} ${B / 2}H${L / 2 - bow - 6}C${L / 2 - bow} ${B / 2} ${L / 2} 8 ${L / 2} 0Z`
          : `M${L / 2} 0Q${L / 2 - bow * 0.3} ${-B / 2} ${L / 2 - bow} ${-B / 2}H${-L / 2 + 6}L${-L / 2} ${-B / 2 + 5}V${B / 2 - 5}L${-L / 2 + 6} ${B / 2}H${L / 2 - bow}Q${L / 2 - bow * 0.3} ${B / 2} ${L / 2} 0Z`;
      const turrets = sub ? 0 : 1 + Math.floor(rand() * (1 + tech));
      const ts = [];
      for (let i = 0; i < turrets; i++) {
        const x = L / 2 - bow * 0.8 - i * ((L - bow) / (turrets + 1));
        const aft = x < -L * 0.1;
        ts.push(
          <g key={i} transform={`translate(${r1(x)} 0)`}>
            <circle r={B * 0.22} {...(P as object)} />
            <path
              d={
                aft
                  ? `M${-B * 0.2} -1.6H${-B * 0.2 - 14}M${-B * 0.2} 1.6H${-B * 0.2 - 14}`
                  : `M${B * 0.2} -1.6H${B * 0.2 + 14}M${B * 0.2} 1.6H${B * 0.2 + 14}`
              }
              {...G}
            />
          </g>,
        );
      }
      return (
        <>
          {!space && !sub && (
            <path
              d={`M${-L / 2} ${-B / 2 - 6}L${-L / 2 - 30} ${-B / 2 - 16}M${-L / 2} ${B / 2 + 6}L${-L / 2 - 30} ${B / 2 + 16}`}
              stroke="rgba(255,255,255,.12)"
            />
          )}
          <path d={hull} {...P} />
          <path d={`M${L / 2 - bow} 0H${-L / 2 + 10}`} {...S} strokeOpacity={0.25} strokeDasharray="3 3" />
          {sub ? (
            <path d="M8 -4H-10L-14 0L-10 4H8L12 0Z" {...P} />
          ) : (
            <rect x={-L * 0.12} y={-B * 0.22} width={L * 0.16} height={B * 0.44} {...(P as object)} />
          )}
          {ts}
          {space && (
            <path
              d={`M${-L / 2 - 2} ${-B / 4}H${-L / 2 - 16}M${-L / 2 - 2} 0H${-L / 2 - 20}M${-L / 2 - 2} ${B / 4}H${-L / 2 - 16}`}
              stroke={hot}
              strokeWidth={2}
              opacity={lit ? 0.9 : 0.4}
            />
          )}
          {sub && <path d={`M${L / 2 - 4} -3H${L / 2 + 10}M${L / 2 - 4} 3H${L / 2 + 10}`} {...G} />}
        </>
      );
    }
    case "factory": {
      const W = 120 + tech * 6;
      const D = 84 + rand() * 12;
      const bays = 3 + Math.floor(rand() * 3);
      const bay = [];
      for (let i = 0; i < bays; i++) bay.push(`M${-W / 2 + 14 + i * ((W - 28) / bays)} ${-D / 2 + 10}v${D - 32}`);
      return (
        <>
          <rect x={-W / 2} y={-D / 2} width={W} height={D} {...(P as object)} />
          <rect x={-W / 2 + 10} y={-D / 2 + 10} width={W - 20} height={D - 32} {...(S as object)} strokeOpacity={0.5} />
          <path d={bay.join("")} {...S} strokeOpacity={0.3} />
          <path d={`M${-W / 4} ${D / 2}v-12h${W / 2}v12`} stroke={hot} strokeWidth={1.4} fill="none" />
          {Array.from({ length: 6 }, (_, i) => (
            <rect
              key={i}
              x={-W / 4 + 4 + (i * (W / 2 - 8)) / 6}
              y={D / 2 - 8}
              width={3}
              height={4}
              fill={hot}
              opacity={lit ? 0.9 : 0.35}
            />
          ))}
          <rect x={W / 2 - 26} y={-D / 2 - 8} width={18} height={18} {...(P as object)} />
        </>
      );
    }
    case "reactor": {
      const R = 40 + tech * 5;
      return (
        <>
          <rect x={-R - 14} y={-R - 14} width={2 * R + 28} height={2 * R + 28} {...(S as object)} strokeOpacity={0.4} />
          <circle r={R} {...(P as object)} />
          <circle r={R * 0.66} {...(S as object)} />
          <circle r={R * 0.3} fill={hot} opacity={lit ? 0.75 : 0.25} />
          {Array.from({ length: 8 }, (_, i) => {
            const a = (i / 8) * Math.PI * 2;
            return (
              <path
                key={i}
                d={`M${r1(Math.cos(a) * R * 0.7)} ${r1(Math.sin(a) * R * 0.7)}L${r1(Math.cos(a) * R)} ${r1(Math.sin(a) * R)}`}
                {...S}
              />
            );
          })}
        </>
      );
    }
    case "extractor": {
      const R = 34 + tech * 4;
      return (
        <>
          <rect
            x={-R - 20}
            y={-R - 20}
            width={2 * R + 40}
            height={2 * R + 40}
            {...(S as object)}
            strokeOpacity={0.35}
            strokeDasharray="4 3"
          />
          <circle r={R} {...(P as object)} />
          <circle r={R * 0.75} {...(S as object)} strokeOpacity={0.5} />
          <circle r={R * 0.5} {...(S as object)} strokeOpacity={0.35} />
          <path d={`M${-R - 10} ${-R * 0.3}H${R + 10}M${-R - 10} ${R * 0.3}H${R + 10}`} {...S} />
          <rect x={-8} y={-R * 0.3 - 4} width={16} height={R * 0.6 + 8} {...(P as object)} />
          <circle r={3} fill={hot} opacity={lit ? 0.9 : 0.4} />
        </>
      );
    }
    case "shield": {
      const R = 50 + tech * 8;
      return (
        <>
          <circle r={R} fill="rgba(199,210,232,0.05)" stroke="rgba(199,210,232,0.5)" strokeDasharray="1 2.5" />
          <circle r={R * 0.8} fill="none" stroke="rgba(199,210,232,0.2)" />
          <rect x={-18} y={-18} width={36} height={36} {...(P as object)} />
          <circle r={10} {...(S as object)} />
          <circle r={4} fill="#c7d2e8" opacity={lit ? 0.9 : 0.5} />
        </>
      );
    }
    case "radar": {
      const R = 26 + tech * 4;
      return (
        <>
          <rect x={-22} y={-22} width={44} height={44} {...(P as object)} />
          <path d={`M0 0L${R * 1.8} ${-R}A${R * 2} ${R * 2} 0 0 1 ${R * 1.8} ${R}Z`} fill={hot} opacity={0.08} />
          <path d={`M-4 ${-R}Q${R} 0 -4 ${R}`} {...S} strokeWidth={1.6} />
          <circle r={R * 2.6} fill="none" stroke="rgba(255,255,255,.08)" strokeDasharray="2 4" />
          <circle r={3} fill={hot} opacity={lit ? 0.9 : 0.4} />
        </>
      );
    }
    case "storage": {
      const n = 2 + Math.min(tech, 3);
      return (
        <>
          <rect x={-60} y={-44} width={120} height={88} {...(P as object)} />
          {Array.from({ length: n * 2 }, (_, i) => (
            <circle
              key={i}
              cx={-60 + (120 / n) * ((i % n) + 0.5)}
              cy={i < n ? -18 : 18}
              r={Math.min(16, 50 / n)}
              {...(S as object)}
            />
          ))}
        </>
      );
    }
    case "wall": {
      return (
        <>
          {Array.from({ length: 5 }, (_, i) => (
            <rect key={i} x={-80 + i * 32} y={-10} width={30} height={20} {...(P as object)} />
          ))}
          <path d="M-80 0H80" {...S} strokeOpacity={0.3} />
        </>
      );
    }
    case "silo": {
      return (
        <>
          <rect x={-56} y={-56} width={112} height={112} {...(P as object)} />
          <circle r={36} {...(S as object)} />
          <path d="M0 -36V36" stroke={hot} strokeWidth={1.8} />
          <circle r={24} {...(S as object)} strokeOpacity={0.4} strokeDasharray="3 3" />
          {[-44, 44].map((x) =>
            [-44, 44].map((y) => <rect key={`${x}${y}`} x={x - 4} y={y - 4} width={8} height={8} fill={hot} opacity={0.6} />),
          )}
        </>
      );
    }
    default: {
      // Turrets and other defenses: a gun on a pad.
      const pad = 34 + tech * 4;
      const barrel = 40 + rand() * 40;
      return (
        <>
          <rect x={-pad} y={-pad} width={pad * 2} height={pad * 2} {...(S as object)} strokeOpacity={0.4} />
          <path
            d={`M${-pad * 0.7} 0L${-pad * 0.35} ${-pad * 0.6}H${pad * 0.35}L${pad * 0.7} 0L${pad * 0.35} ${pad * 0.6}H${-pad * 0.35}Z`}
            {...P}
          />
          <circle r={pad * 0.36} {...(P as object)} />
          <path d={`M${pad * 0.3} -2H${barrel}M${pad * 0.3} 2H${barrel}`} {...G} transform="rotate(-35)" />
          <circle r={3} fill={hot} opacity={lit ? 0.9 : 0.4} />
        </>
      );
    }
  }
}
