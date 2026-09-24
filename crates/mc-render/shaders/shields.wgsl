//!use bindings
// Projected shield membranes. Same-team bubbles are a CSG union: overlapping
// Aegises fuse into one glass surface, with a plasma seam where they meet.
// One fullscreen triangle traces every dome — the cube is not instanced,
// because forty overlapping proxies each re-solved the union on the same
// pixel. The fragment writes closed-form depth so the ground cannot clip
// the membrane into a cap.
//
// The glass carries a world-space honeycomb so merged fields share one lattice
// (triplanar, no spin). Energy runs the seams from the pole to the rim and
// the field breathes; the launch beam is born in the crystal and dissolves
// into a tight cyan knot on the glass. A hit blooms the struck plates with a
// brief crackle, then a hex ripple travels out — across a seam when two
// bubbles meet, not across a gap. Where the shell meets the ground, water,
// or a hull, a bright contact line is drawn — on a unit it follows the
// mesh where it cuts the glass, not the collision disc. A break peels the lip in from
// the rim, dumps the remaining charge, and sheds the plates — it does not
// pop off. A collapsing dome gets out of the way of the bubbles it was
// covering, so they are not hidden until it is gone.

struct ShieldPush {
    count: u32,
    _pad: u32,
}

var<immediate> push: ShieldPush;

struct Shield {
    pos: vec3<f32>,
    radius: f32,
    prev_open: f32,
    open: f32,
    health: f32,
    packed: u32,
    unit_id: u32,
    projector: f32,
    height: f32,
    overlap: u32,
    contact_n: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    contacts: array<u32, 16>,
}

struct ShieldHit {
    pos: vec3<f32>,
    start: f32,
    strength: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

@group(1) @binding(0) var<storage, read> shields: array<Shield>;
@group(1) @binding(1) var<storage, read> shield_hits: array<ShieldHit>;

const PAD: f32 = 2.0;
const HIT_COUNT: u32 = 64u;
const CONTACT_MAX: u32 = 16u;
// Interior of another dome: a hair under the shell so the fusion seam is
// drawn by both sides instead of vanishing into a numerical hole.
const UNION_INSET: f32 = 0.985;
// Centre to vertex of one hex plate, metres. World-space so every dome shares
// the lattice. Triplanar keeps the cells from stretching on the vertical rim.
const HEX: f32 = 5.5;
const SQRT3: f32 = 1.7320508;

fn shield_open(s: Shield) -> f32 {
    return mix(s.prev_open, s.open, globals.sun.w);
}

fn team_of(s: Shield) -> u32 {
    return (s.packed >> 8u) & 255u;
}

// A shattered dome filling while it peels. Damaged live glass is not this.
fn collapsing(s: Shield) -> bool {
    return ((s.packed >> 24u) & 1u) == 1u;
}

fn is_hull(s: Shield) -> bool {
    return ((s.packed >> 25u) & 1u) == 1u;
}

// Dry grid: the projector is dark. A closing bubble still has its glass.
fn stalled(s: Shield) -> bool {
    return ((s.packed >> 26u) & 1u) == 1u;
}

fn hull_center(s: Shield) -> vec3<f32> {
    return vec3<f32>(s.pos.xy, s.pos.z + s.height * 0.5);
}

fn hull_radii(s: Shield) -> vec3<f32> {
    // A little extra on Z so the shell crosses the dirt and draws a foot ring
    // instead of kissing the ground at a single point.
    return vec3<f32>(s.radius, s.radius, max(s.height * 0.5 + 0.8, 0.5));
}

fn shell_center(s: Shield) -> vec3<f32> {
    return select(s.pos, hull_center(s), is_hull(s));
}

fn shell_normal(p: vec3<f32>, s: Shield) -> vec3<f32> {
    if is_hull(s) {
        let r = hull_radii(s);
        return normalize((p - hull_center(s)) / max(r * r, vec3<f32>(1e-4)));
    }
    return (p - s.pos) / max(s.radius, 0.001);
}

fn shell_polar(p: vec3<f32>, s: Shield) -> f32 {
    return length(p.xy - shell_center(s).xy) / max(s.radius, 0.001);
}

// Ray vs ellipsoid. `sphere_hits` needs a unit direction; the scaled ray
// is not one, so the general quadratic keeps t in world-ray space.
fn ellipsoid_hits(ro: vec3<f32>, rd: vec3<f32>, c: vec3<f32>, r: vec3<f32>) -> vec2<f32> {
    let inv = 1.0 / max(r, vec3<f32>(1e-3));
    let o = (ro - c) * inv;
    let d = rd * inv;
    let a = dot(d, d);
    if a < 1.0e-10 {
        return vec2<f32>(-1.0);
    }
    let b = dot(o, d);
    let disc = b * b - a * (dot(o, o) - 1.0);
    if disc < 0.0 {
        return vec2<f32>(-1.0);
    }
    let s = sqrt(max(disc, 0.0));
    return vec2<f32>((-b - s) / a, (-b + s) / a);
}

// Ray vs the visible shell: a hemisphere on the ground, or the hull ellipsoid.
fn shell_hits(ro: vec3<f32>, rd: vec3<f32>, s: Shield) -> vec2<f32> {
    if is_hull(s) {
        return ellipsoid_hits(ro, rd, hull_center(s), hull_radii(s));
    }
    return sphere_hits(ro, rd, s.pos, s.radius);
}

fn spheres_overlap(a: Shield, b: Shield) -> bool {
    // Hull wraps never join the dome union, and a veil fuses with nothing.
    if is_hull(a) || is_hull(b) || is_veil(a) || is_veil(b) {
        return false;
    }
    let d = a.pos - b.pos;
    let r = a.radius + b.radius + PAD * 2.0;
    return dot(d, d) <= r * r;
}

fn world_from_ndc(ndc_xy: vec2<f32>) -> vec3<f32> {
    let h = globals.inv_view_proj * vec4<f32>(ndc_xy, 0.01, 1.0);
    return h.xyz / max(h.w, 1e-5);
}

fn hit_depth(p: vec3<f32>) -> f32 {
    let clip = globals.view_proj * vec4<f32>(p, 1.0);
    return saturate(clip.z / max(clip.w, 1e-5));
}

fn reveal_at(polar: f32, open: f32) -> f32 {
    return open * 1.08 - polar * 0.88;
}

fn hex_round(q: f32, r: f32) -> vec2<f32> {
    let s = -q - r;
    var qq = round(q);
    var rr = round(r);
    let ss = round(s);
    let qd = abs(qq - q);
    let rd = abs(rr - r);
    let sd = abs(ss - s);
    if qd > rd && qd > sd {
        qq = -rr - ss;
    } else if rd > sd {
        rr = -qq - ss;
    }
    return vec2<f32>(qq, rr);
}

fn hex_qr(xy: vec2<f32>) -> vec2<f32> {
    let q = (xy.x * 0.57735027 - xy.y * 0.33333334) / HEX;
    let r = (xy.y * 0.6666667) / HEX;
    return hex_round(q, r);
}

fn hex_center(qr: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(
        HEX * (SQRT3 * qr.x + 0.8660254 * qr.y),
        HEX * (1.5 * qr.y),
    );
}

// x: 0 at the plate centre, ~0.866 at the shared flat. y: a stable cell hash.
fn hex_on(st: vec2<f32>) -> vec2<f32> {
    let qr = hex_qr(st);
    let local = (st - hex_center(qr)) / HEX;
    let g = abs(local);
    let edge = max(g.y * 0.8660254 + g.x * 0.5, g.x);
    return vec2<f32>(edge, hash21(qr + vec2<f32>(13.1, 7.7)));
}

// World-space triplanar honeycomb: XY on the cap, XZ/YZ on the walls, so the
// rim does not stretch into stripes. Dominant plane owns the cell hash.
fn hex_triplanar(p: vec3<f32>, n: vec3<f32>) -> vec2<f32> {
    let an = abs(n);
    let w = an * an * an * an;
    let hx = hex_on(p.yz);
    let hy = hex_on(p.xz);
    let hz = hex_on(p.xy);
    let sum = max(w.x + w.y + w.z, 1e-5);
    let edge = (hx.x * w.x + hy.x * w.y + hz.x * w.z) / sum;
    var hsh = hz.y;
    if w.x > w.y && w.x > w.z {
        hsh = hx.y;
    } else if w.y > w.z {
        hsh = hy.y;
    }
    return vec2<f32>(edge, hsh);
}

// True when `p` sits inside dome `i`'s visible volume, so another shell
// through this point is interior to the team's union.
fn covers(p: vec3<f32>, i: u32) -> bool {
    let s = shields[i];
    let open = shield_open(s);
    // A collapsing dome is no longer a solid field: do not hide the bubbles
    // nested under it until the last of the peel has gone. A hull wrap is
    // its own membrane; it does not punch holes in a dome.
    if open <= 0.001 || s.radius <= 0.0 || collapsing(s) || is_hull(s) {
        return false;
    }
    let d = p - s.pos;
    if d.z < -0.08 {
        return false;
    }
    let r = s.radius * UNION_INSET;
    if dot(d, d) >= r * r {
        return false;
    }
    let polar = length(d.xy) / max(s.radius, 0.001);
    return reveal_at(polar, open) > 0.02;
}

fn covered_except(p: vec3<f32>, skip: u32, team: u32) -> bool {
    let home = shields[skip];
    if home.overlap == 0u {
        return false;
    }
    let n = push.count;
    for (var i = 0u; i < n; i++) {
        if i == skip {
            continue;
        }
        let s = shields[i];
        if team_of(s) != team || !spheres_overlap(home, s) {
            continue;
        }
        if covers(p, i) {
            return true;
        }
    }
    return false;
}

// Closest other same-team shell, in metres. Zero on the fusion circle.
fn fusion_gap(p: vec3<f32>, skip: u32, team: u32) -> f32 {
    let home = shields[skip];
    if home.overlap == 0u {
        return 1.0e9;
    }
    var best = 1.0e9;
    let n = push.count;
    for (var i = 0u; i < n; i++) {
        if i == skip {
            continue;
        }
        let s = shields[i];
        if team_of(s) != team || shield_open(s) <= 0.15 || collapsing(s) || !spheres_overlap(home, s) {
            continue;
        }
        let d = p - s.pos;
        if d.z < -0.08 {
            continue;
        }
        best = min(best, abs(length(d) - s.radius));
    }
    return best;
}

// Both roots of ray vs sphere. Negative components are misses.
fn sphere_hits(ro: vec3<f32>, rd: vec3<f32>, c: vec3<f32>, r: f32) -> vec2<f32> {
    let oc = ro - c;
    let b = dot(oc, rd);
    let disc = b * b - dot(oc, oc) + r * r;
    if disc < 0.0 {
        return vec2<f32>(-1.0);
    }
    let s = sqrt(max(disc, 0.0));
    return vec2<f32>(-b - s, -b + s);
}

fn eye_in_union(eye: vec3<f32>) -> bool {
    let n = push.count;
    for (var i = 0u; i < n; i++) {
        if covers(eye, i) {
            return true;
        }
    }
    return false;
}

// True when a live same-team shell also meets this ray, so a collapsing dome
// in front of it should get out of the way instead of hiding it for the
// whole peel.
fn live_on_ray(ro: vec3<f32>, rd: vec3<f32>, skip: u32, team: u32) -> bool {
    let home = shields[skip];
    if home.overlap == 0u {
        return false;
    }
    let n = push.count;
    for (var i = 0u; i < n; i++) {
        if i == skip {
            continue;
        }
        let s = shields[i];
        if team_of(s) != team || collapsing(s) || shield_open(s) <= 0.001 || !spheres_overlap(home, s) {
            continue;
        }
        let ts = shell_hits(ro, rd, s);
        for (var k = 0u; k < 2u; k++) {
            let t = select(ts.x, ts.y, k == 1u);
            if t <= 0.001 {
                continue;
            }
            let p = ro + rd * t;
            if p.z < s.pos.z - 0.05 {
                continue;
            }
            if reveal_at(shell_polar(p, s), shield_open(s)) >= 0.0 {
                return true;
            }
        }
    }
    return false;
}

// Closest point on any team's union. x is t, y is the owning instance.
fn union_hit(ro: vec3<f32>, rd: vec3<f32>) -> vec2<f32> {
    var best_t = 1.0e9;
    var best_i = -1.0;
    let inside = eye_in_union(ro);
    let n = push.count;
    for (var i = 0u; i < n; i++) {
        let s = shields[i];
        // Hull fields are the posed mesh, drawn in the entity pass.
        if is_hull(s) || shield_open(s) <= 0.001 || s.radius <= 0.0 {
            continue;
        }
        let ts = shell_hits(ro, rd, s);
        for (var k = 0u; k < 2u; k++) {
            let t = select(ts.x, ts.y, k == 1u);
            if t <= 0.001 || t >= best_t {
                continue;
            }
            let p = ro + rd * t;
            if p.z < s.pos.z - 0.05 {
                continue;
            }
            // The peel has already taken this plate. Fall through to the next
            // dome so a bubble under a collapsing one is seen as the lip
            // recedes, not only once the dead shell is gone.
            if reveal_at(shell_polar(p, s), shield_open(s)) < 0.0 {
                continue;
            }
            if collapsing(s) && live_on_ray(ro, rd, i, team_of(s)) {
                continue;
            }
            // Outside the volume the nearest sphere hit is the union. Inside,
            // a closer hit may still sit in another dome.
            if inside && covered_except(p, i, team_of(s)) {
                continue;
            }
            best_t = t;
            best_i = f32(i);
        }
    }
    if best_i < 0.0 {
        return vec2<f32>(-1.0);
    }
    return vec2<f32>(best_t, best_i);
}

// The strike lives on this bubble: on or inside the shell. Slack is only
// numeric — a gap between two fields is not ownership.
fn hit_on_field(p: vec3<f32>, s: Shield) -> bool {
    if s.radius <= 0.0 {
        return false;
    }
    if is_hull(s) {
        let d = (p - hull_center(s)) / hull_radii(s);
        return length(d) <= 1.12;
    }
    let d = p - s.pos;
    if d.z < -0.5 {
        return false;
    }
    return length(d) <= s.radius + 0.75;
}

// Glass that actually meets. The union pad is wider so nearby bubbles are
// walked for CSG; a visible gap is still two fields, and the ripple stops.
fn fields_meet(a: Shield, b: Shield) -> bool {
    if is_hull(a) || is_hull(b) || is_veil(a) || is_veil(b) {
        return false;
    }
    let d = a.pos - b.pos;
    let r = a.radius + b.radius;
    return dot(d, d) <= r * r;
}

// True when this strike may paint `owner`'s glass: it landed here, or on a
// same-team dome that shares a seam with this one.
fn hit_reaches(p: vec3<f32>, owner: u32) -> bool {
    let home = shields[owner];
    if hit_on_field(p, home) {
        return true;
    }
    if home.overlap == 0u {
        return false;
    }
    let team = team_of(home);
    let n = push.count;
    for (var i = 0u; i < n; i++) {
        if i == owner {
            continue;
        }
        let s = shields[i];
        if team_of(s) != team || !fields_meet(home, s) {
            continue;
        }
        if hit_on_field(p, s) {
            return true;
        }
    }
    return false;
}

// Light from strikes on the membrane. rgb is added colour, a is extra alpha.
// Stays in the field's cyan — a heavier shot is denser, not ice-white. Struck
// plates bloom and crackle; the hex ripple is what travels.
fn hits_at(p: vec3<f32>, time: f32, edge: f32, owner: u32) -> vec4<f32> {
    var rgb = vec3<f32>(0.0);
    var a = 0.0;
    let qr = hex_qr(p.xy);
    let here = hex_center(qr);
    let cell_h = hash21(qr + vec2<f32>(13.1, 7.7));
    for (var i = 0u; i < HIT_COUNT; i++) {
        let h = shield_hits[i];
        if h.strength <= 0.0 || !hit_reaches(h.pos, owner) {
            continue;
        }
        let age = time - h.start;
        if age < 0.0 || age > 1.65 {
            continue;
        }
        // A short bloom envelope; the ring keeps going after the crackle dies.
        let envelope = 1.0 - smoothstep(0.02, 0.38, age);
        let fade = 1.0 - smoothstep(0.2, 1.65, age);
        let cool = saturate((h.strength - 0.45) / 3.4);
        let col = mix(vec3<f32>(0.28, 0.82, 1.0), vec3<f32>(0.68, 0.94, 1.0), cool * 0.5);
        let hot = mix(1.05, 1.7, cool);
        let there = hex_center(hex_qr(h.pos.xy));
        let cell_d = length(here - there);
        let dist = length(p - h.pos);
        let reach = HEX * 1.15 + h.strength * 1.6;
        // The whole strike strobes instead of fading smoothly.
        let tick = floor(time * 56.0 + h.start * 9.0);
        let strobe = 0.38 + 0.62 * step(0.26, hash21(vec2<f32>(h.start * 17.0, tick)));
        let plates = (1.0 - smoothstep(reach * 0.4, reach, cell_d)) * envelope * strobe * h.strength;
        let fill = plates * (0.18 + 0.82 * saturate(1.0 - edge * 1.12));
        // Nearby hexes pop on their own clock — a crackle, not a filled disc.
        let near = 1.0 - smoothstep(HEX * 0.35, reach * 1.4, cell_d);
        let pop = step(0.58 - envelope * 0.24, hash21(vec2<f32>(cell_h * 41.0, tick + cell_h * 8.0)));
        let crackle = near * pop * envelope * strobe * h.strength;
        let crackle_fill = crackle * (0.1 + 0.9 * saturate(1.0 - edge * 1.05));
        let wave = cell_d - age * (18.0 + h.strength * 8.0);
        let ring = exp(-abs(wave) * 1.65) * fade * h.strength;
        let seam = pow(saturate((edge - 0.58) / 0.30), 1.25);
        let ring_hex = ring * (0.22 + 0.78 * seam);
        let knot = exp(-(dist * dist) / 4.8) * envelope * strobe * h.strength * saturate(1.0 - edge * 1.05);
        let punch = (fill * 2.8 + crackle_fill * 5.6 + ring_hex * 5.2 + knot * 3.4) * hot;
        rgb += col * punch;
        a += (fill * 0.16 + crackle_fill * 0.22 + ring_hex * 0.16 + knot * 0.24) * hot;
    }
    return vec4<f32>(rgb, a);
}

struct ShieldOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
}

struct ShieldFrag {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
}

fn tech_of(s: Shield) -> u32 {
    return (s.packed >> 16u) & 255u;
}

fn projector_of(s: Shield) -> vec3<f32> {
    let h = select(32.0, s.projector, s.projector > 0.5);
    return vec3<f32>(s.pos.xy, s.pos.z + h);
}

fn energy_of(s: Shield) -> vec3<f32> {
    // Idle field: pale cyan. Hits keep their own denser cyan.
    return mix(vec3<f32>(0.62, 0.84, 1.0), globals.glow.rgb, 0.2);
}

fn column_radius(s: Shield) -> f32 {
    return 1.35 + f32(tech_of(s)) * 0.18;
}

fn sheath_radius(s: Shield) -> f32 {
    return column_radius(s) * 2.7;
}

// Live glass: climb to the pole. Charging: stop at the crown so the trickle
// does not invent a dome that is still down.
fn column_hi(s: Shield) -> f32 {
    if collapsing(s) && shield_open(s) < 0.18 {
        return projector_of(s).z + 26.0;
    }
    return s.pos.z + s.radius * 0.92;
}

fn column_live(s: Shield) -> bool {
    if is_hull(s) || stalled(s) || is_veil(s) {
        return false;
    }
    return shield_open(s) > 0.08 || collapsing(s);
}

fn column_cyan() -> vec3<f32> {
    return vec3<f32>(0.22, 0.68, 1.0);
}

// Launch beam up the spire axis. Negative: miss.
fn column_hit_r(ro: vec3<f32>, rd: vec3<f32>, s: Shield, cr: f32) -> f32 {
    if !column_live(s) {
        return -1.0;
    }
    let c = s.pos.xy;
    let oc = (ro.xy - c);
    let d = rd.xy;
    let a = dot(d, d);
    if a < 1.0e-8 {
        return -1.0;
    }
    let b = dot(oc, d);
    let disc = b * b - a * (dot(oc, oc) - cr * cr);
    if disc < 0.0 {
        return -1.0;
    }
    let sd = sqrt(disc);
    let t0 = (-b - sd) / a;
    let t1 = (-b + sd) / a;
    let t = select(t1, t0, t0 > 0.002);
    if t <= 0.002 {
        return -1.0;
    }
    let p = ro + rd * t;
    // Sink into the crystal so the shaft is born inside the column.
    let lo = projector_of(s).z - 3.2;
    let hi = column_hi(s);
    if p.z < lo || p.z > hi {
        return -1.0;
    }
    return t;
}

fn column_hit(ro: vec3<f32>, rd: vec3<f32>, s: Shield) -> f32 {
    return column_hit_r(ro, rd, s, column_radius(s));
}

fn sheath_hit(ro: vec3<f32>, rd: vec3<f32>, s: Shield) -> f32 {
    return column_hit_r(ro, rd, s, sheath_radius(s));
}

fn column_power(s: Shield, time: f32) -> f32 {
    let open = shield_open(s);
    let dying = select(0.0, 1.0, collapsing(s));
    // Filling while down: a held pulse, not a dome that is not there.
    if dying > 0.5 && open < 0.18 {
        return 0.38 + 0.28 * (0.5 + 0.5 * sin(time * 3.5));
    }
    return open * (1.0 + dying * 1.7);
}

fn column_shade(p: vec3<f32>, s: Shield, time: f32) -> vec4<f32> {
    let proj = projector_of(s);
    let apex = column_hi(s);
    let climb = saturate((p.z - proj.z) / max(apex - proj.z, 1.0));
    let energy = energy_of(s);
    let power = column_power(s, time);
    // Dissolve before the clip plane so the tube never reads as a cut pipe.
    let remain = apex - p.z;
    let taper = smoothstep(0.4, 11.0, remain);
    let fade = (0.78 + 0.42 * climb) * power * taper;
    let radial = p.xy - s.pos.xy;
    let ang = atan2(radial.y, radial.x);
    let pulse = 0.55 + 0.45 * sin(time * 8.6 - climb * 20.0);
    let packet = exp(-pow(fract(climb * 6.2 - time * 1.85) - 0.5, 2.0) * 62.0);
    let helix = 0.5 + 0.5 * pow(0.5 + 0.5 * sin(ang * 3.0 + climb * 24.0 - time * 10.0), 2.2);
    // Six streams from the wreath pods, climbing with the shaft.
    let lobe = pow(0.5 + 0.5 * cos(ang * 6.0 - climb * 10.0 - time * 3.2), 2.8);
    // Shaft runs cyan; the knot on the glass is the arrival, not a cap here.
    let core = mix(energy, column_cyan(), 0.42 + packet * 0.28 + climb * 0.35);
    let rgb = core * (1.25 + pulse * 0.55 + packet * 1.7 + climb * 0.85 + lobe * 0.7) * helix * fade;
    let a = (0.15 + pulse * 0.06 + packet * 0.22 + lobe * 0.05) * fade;
    return vec4<f32>(rgb, a);
}

fn sheath_shade(p: vec3<f32>, s: Shield, time: f32) -> vec4<f32> {
    let proj = projector_of(s);
    let apex = column_hi(s);
    let climb = saturate((p.z - proj.z) / max(apex - proj.z, 1.0));
    let power = column_power(s, time);
    let remain = apex - p.z;
    let taper = smoothstep(0.8, 14.0, remain);
    let fade = (0.45 + 0.4 * climb) * power * taper;
    let radial = p.xy - s.pos.xy;
    let ang = atan2(radial.y, radial.x);
    let helix = pow(0.5 + 0.5 * sin(ang * 6.0 + climb * 18.0 - time * 7.4), 3.4);
    let packet = exp(-pow(fract(climb * 4.4 - time * 1.15) - 0.5, 2.0) * 48.0);
    let core = mix(energy_of(s), column_cyan(), 0.55 + climb * 0.25);
    let rgb = core * (0.55 + helix * 1.4 + packet * 1.1) * fade;
    let a = (0.05 + helix * 0.08 + packet * 0.07) * fade;
    return vec4<f32>(rgb, a);
}

// Arrival at the apex, plus idle current on the glass. Rings are born at the
// pole and run out — the beam hitting the membrane, not a projector sweep.
fn projector_paint(p: vec3<f32>, s: Shield, hx: vec2<f32>, seam: f32, time: f32) -> vec4<f32> {
    let d = p - shell_center(s);
    let polar = length(d.xy) / max(s.radius, 0.001);
    let elev = saturate(d.z / max(select(s.radius, s.height * 0.5, is_hull(s)), 0.001));
    let energy = energy_of(s);
    let ice = mix(energy, vec3<f32>(0.78, 0.92, 1.0), 0.22);
    let open = shield_open(s);
    let breathe = 0.38 + 0.62 * (0.5 + 0.5 * sin(time * 2.55 + hx.y * 6.2831855));
    // Tight cyan join in metres, matching the shaft — not a dome-scale blob.
    // A hull wrap has no projector, so the pole stays quiet.
    let cr = column_radius(s);
    let radial = length(d.xy);
    var contact = exp(-(radial * radial) / (cr * cr * 3.4)) * (0.9 + 0.1 * sin(time * 14.0));
    var corona = exp(-polar * polar * 18.0) * (0.28 + 0.16 * sin(time * 5.6));
    if is_hull(s) {
        contact = 0.0;
        corona = 0.0;
    }
    let phase = polar * 3.6 - time * 0.95;
    let ring0 = exp(-pow(fract(phase) - 0.14, 2.0) * 130.0) * exp(-polar * 1.8) * 0.42;
    let ring1 = exp(-pow(fract(phase + 0.5) - 0.14, 2.0) * 110.0) * exp(-polar * 2.5) * 0.28;
    let splash = corona + ring0 + ring1;
    // Packets on the honeycomb, riding the same outward current.
    let run = fract(polar * 5.2 - time * 0.8 + hx.y * 0.16);
    let packet = exp(-pow(run - 0.5, 2.0) * 40.0);
    let current = seam * (0.28 + 0.72 * packet) * breathe;
    let flow = pow(1.0 - saturate(polar), 1.85) * (0.1 + 0.45 * elev) * breathe;
    let rgb = column_cyan() * contact * 3.6 + ice * splash * 0.72 + energy * (current * 1.35 + flow * 0.2);
    let a = contact * 0.32 + corona * 0.028 + ring0 * 0.048 + ring1 * 0.03 + current * 0.14 + flow * 0.02;
    return vec4<f32>(rgb * open, a * open);
}

// Local metres of `p` in the unit's posed frame: +x forward, +y left, +z up.
fn contact_local(p: vec3<f32>, e: Entity, model: ModelInfo) -> vec3<f32> {
    let t = globals.sun.w;
    let heading = lerp_angle(e.prev_heading, e.heading, t);
    let origin = mix(e.prev_pos, e.pos, t);
    var up = vec3<f32>(0.0, 0.0, 1.0);
    if (model.icon & 0x10000u) != 0u {
        up = terrain_normal(origin.xy, max(e.radius, 4.0));
    }
    let fwd0 = vec3<f32>(cos(heading), sin(heading), 0.0);
    let left = normalize(cross(up, fwd0));
    let fwd = cross(left, up);
    let rel = p - origin;
    return vec3<f32>(dot(rel, fwd), dot(rel, left), dot(rel, up));
}

// Distance to the baked hull column. Negative is inside the mesh span.
fn hull_plan_sd(local: vec3<f32>, model: ModelInfo, blueprint: u32) -> f32 {
    let half = max(model.plan_half, 0.5);
    let uv = local.xy / half;
    if max(abs(uv.x), abs(uv.y)) > HULL_PLAN_REACH {
        return HULL_PLAN_RANGE;
    }
    let layers = textureNumLayers(hull_plans);
    if layers == 0u {
        return HULL_PLAN_RANGE;
    }
    let tex = uv / HULL_PLAN_REACH * 0.5 + 0.5;
    let i = i32(min(blueprint, layers - 1u));
    let s = textureSampleLevel(hull_plans, clamp_sampler, tex, i, 0.0);
    let sd_xy = (0.5 - s.r) * 2.0 * HULL_PLAN_RANGE;
    let z0 = s.g * max(model.height, 0.5);
    let z1 = s.b * max(model.height, 0.5);
    var dz = 0.0;
    if local.z < z0 {
        dz = z0 - local.z;
    } else if local.z > z1 {
        dz = local.z - z1;
    }
    if sd_xy > 0.0 {
        return length(vec2<f32>(sd_xy, dz));
    }
    if dz > 0.0 {
        return dz;
    }
    return sd_xy;
}

// Bright line where the shell kisses terrain, water, or a hull. Tight core
// plus a short halo so it reads as a stroke, not a wash. A hull stroke
// follows the mesh where it cuts the glass, not the collision disc.
fn contact_at(p: vec3<f32>, s: Shield) -> f32 {
    let ground = max(terrain_height(p.xy), globals.map.z);
    let gap = p.z - ground;
    var line = exp(-gap * gap * 1.55) * 0.85 + exp(-gap * gap * 0.28) * 0.4;
    // Hull wrap: a stroke where the glass sits on the dirt around the feet.
    if is_hull(s) {
        let ankle = p.z - s.pos.z;
        line = max(line, exp(-ankle * ankle * 1.4) * 0.95 + exp(-ankle * ankle * 0.22) * 0.4);
    }
    let shell = s.radius;
    let n = min(s.contact_n, CONTACT_MAX);
    for (var i = 0u; i < n; i++) {
        let e = dynamic_entities[s.contacts[i]];
        if e.unit_id == s.unit_id || e.radius < 0.4 {
            continue;
        }
        let origin = mix(e.prev_pos, e.pos, globals.sun.w);
        let off = origin - s.pos;
        let model = models[e.blueprint];
        let h = max(model.height, e.radius * 0.8);
        let reach = max(model.plan_half, e.radius) + h * 0.55 + 4.0;
        let d2 = dot(off, off);
        let lo = shell - reach;
        let hi = shell + reach;
        if d2 < lo * max(lo, 0.0) || d2 > hi * hi {
            continue;
        }
        let hull = hull_plan_sd(contact_local(p, e, model), model, e.blueprint);
        line = max(line, exp(-hull * hull * 1.35) * 0.9 + exp(-hull * hull * 0.25) * 0.35);
    }
    return saturate(line);
}

fn kill() -> ShieldFrag {
    discard;
    return ShieldFrag(vec4<f32>(0.0), 0.0);
}

@vertex
fn vs_shield(@builtin(vertex_index) vid: u32) -> ShieldOut {
    var out: ShieldOut;
    out.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
    if vid >= 3u || push.count == 0u {
        return out;
    }
    let ccw = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    let ndc = ccw[vid];
    out.world = world_from_ndc(ndc);
    // Mid clip-space depth: sitting on the near plane (z = w) gets clipped
    // away. The fragment shader replaces this with the dome's depth.
    out.clip = vec4<f32>(ndc, 0.5, 1.0);
    return out;
}

@fragment
fn fs_shield(in: ShieldOut) -> ShieldFrag {
    let eye = globals.camera.xyz;
    let time = globals.camera.w;
    let dir = normalize(in.world - eye);
    let surf = union_hit(eye, dir);
    let hit = surf.x >= 0.0;
    var color = vec3<f32>(0.0);
    var alpha = 0.0;
    var depth_t = 1.0e9;
    let n = push.count;
    for (var i = 0u; i < n; i++) {
        let t = column_hit(eye, dir, shields[i]);
        if t > 0.0 {
            let beam = column_shade(eye + dir * t, shields[i], time);
            color += beam.rgb;
            alpha += beam.a;
            depth_t = min(depth_t, t);
        }
        let ts = sheath_hit(eye, dir, shields[i]);
        if ts > 0.0 {
            let veil = sheath_shade(eye + dir * ts, shields[i], time);
            color += veil.rgb;
            alpha += veil.a;
            depth_t = min(depth_t, ts);
        }
    }
    if !hit && depth_t > 1.0e8 {
        return kill();
    }

    if hit {
        let owner = u32(surf.y + 0.5);
        let s = shields[owner];
        let p = eye + dir * surf.x;
        let open = shield_open(s);
        var nrm = shell_normal(p, s);
        let polar = shell_polar(p, s);
        let reveal = reveal_at(polar, open);
        if reveal >= 0.0 && is_veil(s) {
            // The veil: its own glass, laid over whatever columns were met on the way.
            let glass = veil_glass(p, s, dir, owner, time, reveal);
            let shaft_a = saturate(alpha);
            let pre = glass.rgb * glass.a + color * shaft_a * (1.0 - glass.a);
            alpha = glass.a + shaft_a * (1.0 - glass.a);
            color = pre / max(alpha, 1e-5);
            depth_t = min(depth_t, surf.x);
        } else if reveal >= 0.0 {
            let health = saturate(s.health);
            // Peel only. A live dome stays steady at any charge; stall-close
            // keeps its health and stays quiet.
            let dying = select(0.0, 1.0, collapsing(s));
            var born = 0.0;
            if open < 0.96 {
                born = exp(-abs(reveal) * 22.0) * (1.0 - open);
            }
            if dying > 0.5 {
                // Hot peel: a wide lip racing in from the rim as the glass goes.
                born = exp(-abs(reveal) * 8.0) * (0.45 + 0.55 * open);
            }
            let outward = nrm;
            if dot(nrm, -dir) < 0.0 {
                nrm = -nrm;
            }
            let cell = select(HEX, 2.8, is_hull(s));
            let hx = hex_triplanar(p * (HEX / cell), outward);
            let eye_dist = length(p - eye);
            let cell_px = cell * globals.lod.x / max(eye_dist, 1.0);
            let facing = saturate(dot(nrm, -dir));
            // Honeycomb is strongest on the rim. A hull wrap is seen face-on
            // from the play camera, so its plates have to read without a graze.
            let rim_hex = mix(select(0.28, 0.82, is_hull(s)), 1.0, pow(1.0 - facing, 1.2));
            let hex_see = smoothstep(1.6, 7.5, cell_px) * rim_hex;
            let seam = pow(saturate((hx.x - 0.66) / 0.22), 1.45) * hex_see;
            let plate = (1.0 - smoothstep(0.70, 0.90, hx.x)) * hex_see;
            let fres = pow(1.0 - facing, 2.1);
            let live = 0.68 + 0.32 * sin(time * 2.4);
            let touch = contact_at(p, s);
            let blow = hits_at(p, time, hx.x, owner);
            let stress = (1.0 - health) * (1.0 - health);
            let fuse = exp(-fusion_gap(p, owner, team_of(s)) * 0.34);
            let paint = projector_paint(p, s, hx, seam, time);

            let team_c = globals.team_colors[s.packed & 7u].rgb;
            let energy = energy_of(s);
            let rim_c = mix(energy, team_c, 0.16);
            let fuse_c = mix(vec3<f32>(0.55, 0.86, 1.0), team_c, 0.1);
            let ice = mix(energy, vec3<f32>(0.82, 0.94, 1.0), 0.45);
            let touch_c = mix(column_cyan(), energy, 0.18);

            // Idle: grazing glass that breathes, a contact stroke, and a live honeycomb.
            // Hull wraps add a face-on skin so they read on the unit, not only at the rim.
            let skin = select(0.0, 0.22, is_hull(s));
            // Shaft is already in `color`. The membrane is a window — keep it.
            let shaft_rgb = color;
            let shaft_a = alpha;
            let shaft_t = depth_t;
            color = rim_c * (fres * 0.58 * live + stress * 0.18 + skin * 0.55 * live);
            color += energy * (seam * 1.05 * live + plate * 0.14 + stress * seam * 0.4 + skin * plate * 0.8);
            color += mix(energy, ice, dying) * born * mix(1.3, 3.4, dying);
            color += blow.rgb;
            color += fuse_c * fuse * 0.45;
            color += paint.rgb * (1.0 + dying * 1.6);
            color += touch_c * touch * 2.4;

            alpha = 0.008 + fres * 0.07 * live + stress * 0.045 + skin * 0.07;
            alpha += seam * 0.16 * live + plate * 0.03 + stress * seam * 0.06;
            alpha += born * mix(0.28, 0.72, dying) + blow.a + fuse * 0.1 + paint.a;
            alpha += touch * 0.28;
            alpha *= smoothstep(0.0, 0.1, open) * mix(0.75 + 0.25 * health, 1.25, dying);
            // Premul, then back — the fragment still does `color * alpha`.
            let glass_a = saturate(alpha);
            let glass_pre = color * glass_a;
            let shaft_pre = shaft_rgb * saturate(shaft_a);
            if shaft_a > 0.0 && shaft_t < surf.x {
                // Camera inside: the column sits in front of the far wall.
                let keep = 1.0 - saturate(shaft_a);
                let pre = shaft_pre + glass_pre * keep;
                alpha = saturate(shaft_a) + glass_a * keep;
                color = pre / max(alpha, 1e-5);
                depth_t = shaft_t;
            } else {
                // Camera outside: see the column through the glass.
                let see = 1.0 - glass_a;
                let pre = glass_pre + shaft_pre * see;
                alpha = glass_a + saturate(shaft_a) * see;
                color = pre / max(alpha, 1e-5);
                depth_t = min(depth_t, surf.x);
            }
        }
    }

    if alpha < 0.002 || depth_t > 1.0e8 {
        return kill();
    }
    let at = eye + dir * depth_t;
    color = apply_haze(apply_fog_of_war(color, at.xy), at, eye);
    return ShieldFrag(vec4<f32>(color * alpha, alpha), hit_depth(at));
}

// ---- The veil -------------------------------------------------------------------
// The Replication Engine's dome (packed bit 27, `mirror::SHIELD_VEIL`). It must say
// "you cannot break this" at a glance, so it shares nothing with the cyan glass: a
// dark, heavy membrane that dims what is inside it, a geodesic lattice of violet-white
// struts in latitude bands that turn slowly against each other, bright seams where the
// bands meet, and a hot rim. A hit is a caustic flare where it lands that is shed
// sideways and slides off round the dome, lighting the struts it passes. It never
// peels, never collapses, and fuses with no other field.

const VEIL_CELL: f32 = 15.0;
const VEIL_BANDS: f32 = 5.0;
const VEIL_VIOLET: vec3<f32> = vec3<f32>(0.52, 0.2, 1.0);
const VEIL_WHITE: vec3<f32> = vec3<f32>(0.96, 0.9, 1.0);
const VEIL_DARK: vec3<f32> = vec3<f32>(0.012, 0.004, 0.03);

fn is_veil(s: Shield) -> bool {
    return ((s.packed >> 27u) & 1u) == 1u;
}

// Triangular lattice of cell `c` at `q` (metres): x distance to the nearest strut in
// strut spacings, y how close to a node (1 on it), z a hash of the triangle.
fn veil_lattice(q: vec2<f32>, c: f32) -> vec3<f32> {
    let h = c * 0.8660254;
    let d0 = q.y / h;
    let d1 = (q.x * 0.8660254 + q.y * 0.5) / h;
    let d2 = (q.y * 0.5 - q.x * 0.8660254) / h;
    let e = abs(fract(vec3<f32>(d0, d1, d2) + 0.5) - 0.5);
    let edge = min(e.x, min(e.y, e.z));
    let node = exp(-dot(e, e) * 40.0);
    let id = hash21(vec2<f32>(floor(d0) * 1.37 + floor(d1) * 7.1, floor(d2) * 3.3 + 0.5));
    return vec3<f32>(edge, node, id);
}

// Band coordinates of a point on the dome: xy metres in the band's turning frame
// (x wraps a whole number of cells round the dome), z the band index, w metres to
// the nearest seam between bands.
fn veil_band(n: vec3<f32>, s: Shield, time: f32) -> vec4<f32> {
    let elev = asin(clamp(n.z, 0.0, 1.0));
    let span = 1.5707964 / VEIL_BANDS;
    let band = min(floor(elev / span), VEIL_BANDS - 1.0);
    let mid = (band + 0.5) * span;
    let around = 6.2831855 * s.radius * cos(mid);
    let cells = max(round(around / VEIL_CELL), 6.0);
    let dir = select(-1.0, 1.0, (u32(band) & 1u) == 0u);
    let turn = time * 0.012 * dir * (1.0 + band * 0.35);
    let az = fract((atan2(n.y, n.x) / 6.2831855) + turn);
    let q = vec2<f32>(az * cells * VEIL_CELL, elev * s.radius + band * 3.1);
    let seam_lo = abs(elev - band * span);
    let seam_hi = select(abs(elev - (band + 1.0) * span), 1.0e3, band >= VEIL_BANDS - 1.0);
    let seam = select(min(seam_lo, seam_hi), seam_hi, band < 0.5) * s.radius;
    return vec4<f32>(q, band, seam);
}

// Strikes on the veil: rgb added light, a extra cover. `lit` is the strut mask, so a
// sliding flare lights the lattice it crosses.
fn veil_hits(n: vec3<f32>, s: Shield, owner: u32, time: f32, lit: f32) -> vec4<f32> {
    var rgb = vec3<f32>(0.0);
    var a = 0.0;
    for (var i = 0u; i < HIT_COUNT; i++) {
        let h = shield_hits[i];
        if h.strength <= 0.0 || !hit_on_field(h.pos, s) {
            continue;
        }
        let age = time - h.start;
        if age < 0.0 || age > 1.8 {
            continue;
        }
        let u0 = normalize(h.pos - s.pos);
        // Shed sideways and down, round the dome: a great circle through the strike.
        var east = cross(vec3<f32>(0.0, 0.0, 1.0), u0);
        if dot(east, east) < 1e-6 {
            east = vec3<f32>(1.0, 0.0, 0.0);
        }
        east = normalize(east);
        let north = cross(u0, east);
        let side = select(-1.0, 1.0, hash11(h.start * 13.7 + h.pos.x * 0.11) > 0.5);
        let slide = normalize(east * side - north * 0.55);
        let travel = (26.0 + h.strength * 11.0) / max(s.radius, 1.0) * (1.0 - exp(-age * 2.6));
        let head = normalize(u0 * cos(travel) + slide * sin(travel));
        let fade = 1.0 - smoothstep(0.55, 1.8, age);
        let r = s.radius;
        // The flare riding on the glass.
        let d = length(n - head) * r;
        let size = 3.0 + h.strength * 1.6;
        let flare = exp(-d * d / (size * size)) * fade * h.strength;
        // The strike itself: a white burst for the first instant, wider than the flare.
        let d0 = length(n - u0) * r;
        let burst = exp(-d0 * d0 / (size * size * 5.0)) * (1.0 - smoothstep(0.0, 0.16, age)) * h.strength;
        // The trail it leaves along the arc behind it.
        let m = normalize(cross(u0, slide));
        let off = dot(n, m) * r;
        let along = atan2(dot(n, slide), dot(n, u0));
        let on_arc = step(-0.02, along) * step(along, travel + 0.01);
        let lead = pow(saturate(along / max(travel, 1e-3)), 2.0);
        let trail = exp(-off * off / (size * size * 0.25)) * on_arc * lead * fade * h.strength;
        // Caustic web: the struts round the flare catch its light.
        let web = lit * exp(-d / (size * 3.2)) * fade * h.strength;
        rgb += VEIL_WHITE * (flare * 3.0 + burst * 3.5) + mix(VEIL_VIOLET, VEIL_WHITE, 0.35) * (trail * 2.4 + web * 3.0);
        a += flare * 0.5 + burst * 0.6 + trail * 0.22 + web * 0.3;
    }
    return vec4<f32>(rgb, a);
}

// The veil's straight colour and cover at `p`, before it is laid over the columns.
fn veil_glass(p: vec3<f32>, s: Shield, dir: vec3<f32>, owner: u32, time: f32, reveal: f32) -> vec4<f32> {
    let eye = globals.camera.xyz;
    let outward = shell_normal(p, s);
    var nrm = outward;
    if dot(nrm, -dir) < 0.0 {
        nrm = -nrm;
    }
    let facing = saturate(dot(nrm, -dir));
    // A hard rim only: a soft fresnel over a dark body washes the whole dome lilac.
    let fres = pow(1.0 - facing, 5.0);
    let band = veil_band(outward, s, time);
    let lat = veil_lattice(band.xy, VEIL_CELL);
    // Metres a pixel covers here: struts are drawn a steady fraction of a pixel wide
    // or more, and fade out as their cells shrink under a few pixels.
    let px_m = length(p - eye) / max(globals.lod.x, 1.0);
    let cell_px = VEIL_CELL / max(px_m, 1e-4);
    let see = smoothstep(2.5, 9.0, cell_px);
    let h = VEIL_CELL * 0.8660254;
    let width = max(0.025, px_m * 0.7 / h);
    let strut = (1.0 - smoothstep(width * 0.5, width, lat.x)) * see;
    let node = lat.y * see;
    let seam = exp(-band.w * band.w / max(0.8, px_m * px_m * 2.0));
    // Slow light running up the struts from the ground, one band after another.
    let climb = fract(band.y / (s.radius * 0.9) - time * 0.09 + lat.z * 0.08);
    let run = exp(-pow(climb - 0.5, 2.0) * 90.0);
    // A few panes glint as the bands turn: the dome is solid, not a projection.
    let glint = step(0.93, lat.z) * (0.5 + 0.5 * sin(time * 0.7 + lat.z * 40.0)) * see * (1.0 - fres);
    let hits = veil_hits(outward, s, owner, time, strut + node);
    let touch = contact_at(p, s);
    let born = exp(-abs(reveal) * 18.0) * (1.0 - shield_open(s));

    // Straight colour: a near-black body that dims and tints what is under it, the rim
    // and the struts lit. Kept dark away from the struts so the dome reads as a mass.
    var rgb = VEIL_DARK;
    rgb += VEIL_VIOLET * (fres * 1.4 + strut * (0.22 + run * 2.2) + seam * 1.1 + glint * 0.25);
    rgb += VEIL_WHITE * (node * (0.15 + run * 1.4) + fres * fres * 0.9 + seam * run * 1.2);
    rgb += hits.rgb;
    rgb += mix(VEIL_VIOLET, VEIL_WHITE, 0.4) * (touch * 3.2 + born * 3.0);
    var a = 0.55 + fres * 0.3 + strut * 0.1 + node * 0.1 + seam * 0.2 + glint * 0.05;
    a += hits.a + touch * 0.3 + born * 0.4;
    a *= smoothstep(0.0, 0.1, shield_open(s));
    return vec4<f32>(rgb, saturate(a));
}
