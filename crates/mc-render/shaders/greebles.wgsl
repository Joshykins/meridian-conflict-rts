// Texture detail on a unit's plates (docs/STANDARD.md, finish): the working detail of
// a real machine, drawn, never modelled. Rides after surface.wgsl in the `surface`
// prelude.
//
// Two layers, neither left to chance:
// - Panelling, by zone (`SMARK_*` armour, engine, machinery, walkway; none): a plate
//   (`surf_courses`) is cut into a regular grid of panels as dense as the skin's
//   `detail` asks. Armour has few, large panels; machinery small ones with a bolt in
//   each corner; engine decking is grating; walkways tread plate.
// - Fittings, placed by the model's author (`MeshBuilder::mark`): a grille, louvres, a
//   vent, ribs, an access cover, a hatch, a bolt row, an applique plate or a weld seam,
//   filling its box on the face it crosses, with relief (`fitting_h`, differenced in
//   entity.wgsl for the normal); and paint (hazard stripes, a stencilled number, a
//   warning sign, lettering, `mark_paint`).
// Only weathering varies by chance: wear, scratches, dirt.

struct Greeble {
    // Relief: 1 the plate's face, lower is let into it.
    h: f32,
    // Multiplies albedo: holes and recesses dark.
    cavity: f32,
    // Paint over it: rgb and how much.
    paint: vec4<f32>,
}

fn greeble_none() -> Greeble {
    return Greeble(1.0, 1.0, vec4<f32>(0.0));
}

// The panel a plate's point lies in: the plate cut into whole panels about `size` across.
fn greeble_panel(cell: SurfaceCell, size: f32) -> SurfaceCell {
    let n = vec2<f32>(surf_fit(2.0 * cell.half.x, size), surf_fit(2.0 * cell.half.y, size));
    let pw = 2.0 * cell.half / n;
    let q = cell.p + cell.half;
    let k = clamp(floor(q / pw), vec2<f32>(0.0), n - 1.0);
    var panel: SurfaceCell;
    panel.p = q - (k + 0.5) * pw;
    panel.half = pw * 0.5;
    panel.id = hash21(k * vec2<f32>(5.13, 9.71) + vec2<f32>(cell.id * 37.0, cell.id * 61.0));
    return panel;
}

// Non-slip tread plate: raised diamonds in rows.
fn greeble_walkway(i: SurfaceIn, cell: SurfaceCell, fw: f32) -> Greeble {
    var g = greeble_none();
    let pitch = i.scale * 0.07;
    let row = floor(cell.p.y / pitch);
    let flip = select(1.0, -1.0, row % 2.0 > 0.5);
    let q = vec2<f32>(fract(cell.p.x / pitch) - 0.5, fract(cell.p.y / pitch) - 0.5) * pitch;
    let r = vec2<f32>(q.x + flip * q.y, q.x - flip * q.y) * 0.7071;
    let lug = surf_step(pitch * 0.32 - abs(r.x), 0.0, fw) * surf_step(pitch * 0.07 - abs(r.y), 0.0, fw);
    g.h = 0.9 + 0.12 * lug;
    g.cavity = 0.88 + 0.12 * lug;
    let seen = 1.0 - smoothstep(pitch * 0.15, pitch * 0.4, fw);
    g.h = mix(1.0, g.h, seen);
    g.cavity = mix(0.94, g.cavity, seen);
    return g;
}

// The panelling at a point of plate `cell`, by the point's zone: a regular grid, its
// seams finer than the plate's (`gap`, `bevel`).
fn surf_greeble(i: SurfaceIn, cell: SurfaceCell, d_face: f32, gap: f32, bevel: f32, fw: f32) -> Greeble {
    var g = greeble_none();
    if i.detail <= 0.0 {
        return g;
    }
    if i.zone == SMARK_WALKWAY {
        return greeble_walkway(i, cell, fw);
    }
    var zone_size = 1.0;
    switch i.zone {
        case SMARK_ARMOUR: { zone_size = 2.2; }
        case SMARK_ENGINE: { zone_size = 0.8; }
        case SMARK_MACHINERY: { zone_size = 0.55; }
        default: {}
    }
    let size = i.scale * mix(1.2, 0.5, i.detail) * zone_size;
    let panel = greeble_panel(cell, size);
    let small = min(panel.half.x, panel.half.y);
    let e = surf_edge(panel.p, panel.half);
    g.h = mix(0.72, 1.0, surf_rise(e, gap * 0.45, bevel * 0.4));
    g.cavity = 1.0 - 0.6 * surf_band(e, gap * 0.5, fw);
    // Panels changed at different times: a shade apart, never patchwork.
    g.cavity *= 0.97 + 0.05 * hash11(panel.id * 3.7);
    let field = panel.half - vec2<f32>((gap + bevel) * 1.6);
    if field.x > bevel * 2.0 && field.y > bevel * 2.0 {
        let inside = surf_edge(panel.p, field);
        switch i.zone {
            case SMARK_MACHINERY: {
                // A bolt in each corner of every panel.
                let bolt = surf_rivets(panel.p, panel.half, (gap + bevel) * 1.2, 1e3, i.scale * 0.018);
                g.h = max(g.h, 1.0 + 0.3 * bolt);
                g.cavity *= 1.0 - 0.25 * step(0.01, bolt) * (1.0 - bolt);
            }
            case SMARK_ENGINE: {
                // Grating: square holes over the whole panel.
                let pitch = i.scale * 0.06;
                let q = abs(fract((panel.p + field) / pitch) - 0.5) * pitch;
                let hole = surf_step(pitch * 0.32 - max(q.x, q.y), 0.0, fw) * surf_step(inside, 0.0, fw);
                g.h = min(g.h, 1.0 - 0.75 * hole);
                g.cavity *= 1.0 - 0.75 * hole;
            }
            default: {}
        }
    }
    // Nothing smaller than a few pixels: the detail fades to its mean as it shrinks.
    let seen = 1.0 - smoothstep(small * 0.08, small * 0.25, fw);
    g.h = mix(1.0, g.h, seen);
    g.cavity = mix(0.95, g.cavity, seen);
    return g;
}

// ---- marks ---------------------------------------------------------------------

// A mark's box laid on the face a point lies on: the point (`q`, metres from the box's
// middle) and the box's half size (`half`) along the face's level (`right`, toward the
// viewer's right) and upward (`up`) directions, model space.
struct MarkPlane {
    q: vec2<f32>,
    half: vec2<f32>,
    right: vec3<f32>,
    up: vec3<f32>,
}

// `n` the face's normal (model space, either sign), `screen` the model-space step one
// pixel right on screen.
fn mark_plane(here: MarksHere, n: vec3<f32>, screen: vec3<f32>) -> MarkPlane {
    let flat = abs(n.z) > 0.7;
    let along = select(normalize(vec3<f32>(-n.y, n.x, 0.0) + vec3<f32>(1e-6, 0.0, 0.0)), vec3<f32>(0.0, 1.0, 0.0), flat);
    let right = along * select(-1.0, 1.0, dot(along, screen) >= 0.0);
    var up = normalize(cross(n, right));
    up = select(up, -up, up.z < 0.0 || (flat && up.x < 0.0));
    var p: MarkPlane;
    p.right = right;
    p.up = up;
    p.q = vec2<f32>(dot(here.at, right), dot(here.at, up));
    p.half = vec2<f32>(dot(abs(right), here.half), dot(abs(up), here.half));
    return p;
}

// A fitting's relief and shade at `q` in a box of half size `half` (`SMARK_GRILLE` ..
// `SMARK_SEAM`): x the height (1 the face, lower let in, higher proud), y what it does
// to the paint. `pitch` the mark's own (0 the standard); `fw` a pixel, metres.
fn fitting_h(kind: u32, q: vec2<f32>, half: vec2<f32>, pitch: f32, fw: f32) -> vec2<f32> {
    let inside = surf_edge(q, half);
    if inside < -fw {
        return vec2<f32>(1.0, 1.0);
    }
    // Lay the fitting along the box's long side.
    let turned = half.y > half.x;
    let p = select(q, q.yx, turned);
    let hf = select(half, half.yx, turned);
    let rim = 0.035;
    let frame = surf_band(inside - rim * 0.5, rim * 0.25, fw);
    let field = surf_step(inside, rim, fw);
    var h = 1.0 - 0.7 * frame;
    var shade = 1.0 - 0.85 * frame;
    switch kind {
        case SMARK_GRILLE: {
            let pt = select(0.07, pitch, pitch > 0.0);
            let c = abs(fract((q + half) / pt) - 0.5) * pt;
            let hole = surf_step(pt * 0.33 - max(c.x, c.y), 0.0, fw) * field;
            h = min(h, 1.0 - 0.85 * hole);
            shade *= 1.0 - 0.85 * hole;
        }
        case SMARK_LOUVRES: {
            // Slats across the box, each raked down so its lower edge stands out.
            let pt = select(0.09, pitch, pitch > 0.0);
            let s = fract((q.y + half.y) / pt);
            h = min(h, mix(1.0, 0.3 + 0.65 * s, field));
            shade *= 1.0 - 0.7 * field * (1.0 - s) * (1.0 - s);
        }
        case SMARK_VENT: {
            let pt = select(0.065, pitch, pitch > 0.0);
            let row = floor((q.y + half.y) / pt);
            let x = q.x + half.x + 0.5 * pt * (row % 2.0);
            let c = vec2<f32>(fract(x / pt) - 0.5, fract((q.y + half.y) / pt) - 0.5) * pt;
            let hole = surf_step(pt * 0.3 - length(c), 0.0, fw) * field;
            h = min(h, 1.0 - 0.8 * hole);
            shade *= 1.0 - 0.8 * hole;
        }
        case SMARK_RIBS: {
            let pt = select(0.12, pitch, pitch > 0.0);
            let d = abs(fract((p.y + hf.y) / pt) - 0.5) * pt;
            let rib = smoothstep(pt * 0.32, pt * 0.18, d) * field;
            h = 1.0 + 0.45 * rib;
            shade = 1.0 - 0.3 * field * (1.0 - rib);
        }
        case SMARK_ACCESS: {
            // The cover's seam just inside the box, a bolt in each corner, a handle.
            let seam = surf_band(inside - 0.02, 0.012, fw);
            h = 1.0 - 0.8 * seam;
            shade = 1.0 - 0.8 * seam;
            let bolt = surf_rivets(q, half, 0.06, 1e3, 0.026);
            h = max(h, 1.0 + 0.4 * bolt);
            shade *= 1.0 - 0.5 * step(0.01, bolt) * (1.0 - bolt) + 0.35 * bolt * bolt;
            let grip = surf_step(0.035 - abs(p.y), 0.0, fw) * surf_step(hf.x * 0.28 - abs(p.x), 0.0, fw);
            h = min(h, 1.0 - 0.55 * grip);
            shade *= 1.0 - 0.6 * grip;
        }
        case SMARK_HATCH: {
            // A round lid: its seam, a raised lid inside it, the hinge to one side.
            let r = min(half.x, half.y) * 0.92;
            let d = length(q);
            let seam = surf_band(d - r, 0.014, fw);
            let lid = surf_step(r - d, 0.0, fw);
            h = 1.0 - 0.8 * seam + 0.15 * lid * (1.0 - seam);
            shade = 1.0 - 0.8 * seam;
            let hinge = surf_step(0.06 - abs(q.y + r * 0.95), 0.0, fw) * surf_step(r * 0.35 - abs(q.x), 0.0, fw);
            h = max(h, 1.0 + 0.4 * hinge);
            let ring = surf_band(d - r * 0.78, 0.004, fw);
            shade *= 1.0 - 0.25 * ring;
            h = min(h, 1.0 - 0.2 * ring);
        }
        case SMARK_BOLTS: {
            let pt = select(0.18, pitch, pitch > 0.0);
            let k = clamp(round(p.x / pt) * pt, -hf.x + pt * 0.5, hf.x - pt * 0.5);
            let d = length(vec2<f32>(p.x - k, p.y)) / 0.03;
            let bolt = select(0.0, sqrt(max(1.0 - d * d, 0.0)), d < 1.0);
            h = 1.0 + 0.5 * bolt;
            // Dark round its foot, worn bright on its crown.
            shade = 1.0 - 0.5 * step(0.01, bolt) * (1.0 - bolt) + 0.35 * bolt * bolt;
        }
        case SMARK_PANEL: {
            // A plate bolted over the face: proud, bevelled, a ring of bolts.
            h = 1.0 + 0.6 * surf_rise(inside, 0.0, 0.03);
            // The gap under its edge is dark; its bevel catches the light.
            shade = 1.0 - 0.75 * surf_band(inside, 0.012, fw) + 0.15 * surf_band(inside - 0.03, 0.012, fw);
            let bolt = surf_rivets(q, half, 0.05, 0.22, 0.022);
            h = max(h, 1.6 + 0.3 * bolt * step(0.01, bolt));
            shade *= 1.0 - 0.45 * step(0.01, bolt) * (1.0 - bolt) + 0.35 * bolt * bolt;
        }
        case SMARK_LAP: {
            // The plate on the +side laps over the one on the -side: a deep shadow under
            // its edge, the edge itself worn bright.
            let over = smoothstep(-0.008, 0.008, p.y);
            h = 1.0 + 0.5 * over;
            shade = 1.0 - 0.85 * surf_band(p.y + 0.025, 0.022, fw) + 0.6 * surf_band(p.y - 0.008, 0.008, fw);
        }
        case SMARK_GROOVE: {
            // A machined channel: dark down its middle, its lips polished bright.
            let w = select(0.045, pitch * 0.5, pitch > 0.0);
            let channel = 1.0 - smoothstep(w * 0.7, w, abs(p.y));
            h = 1.0 - 0.9 * channel;
            shade = 1.0 - 0.8 * channel * smoothstep(w, w * 0.3, abs(p.y)) + 0.5 * surf_band(abs(p.y) - w, w * 0.15, fw);
        }
        case SMARK_OUTLINE: {
            let line = surf_band(inside - 0.04, 0.012, fw);
            h = 1.0 - 0.7 * line;
            shade = 1.0 - 0.8 * line + 0.35 * surf_band(inside - 0.06, 0.006, fw);
        }
        case SMARK_LAMP: {
            // The slot the lamp sits in (its light is `lamp_light`).
            let slot = surf_step(0.035 - abs(p.y), 0.0, fw) * surf_step(hf.x - 0.03 - abs(p.x), 0.0, fw);
            let rim = surf_band(abs(p.y) - 0.045, 0.01, fw) * surf_step(hf.x - abs(p.x), 0.0, fw);
            h = 1.0 - 0.6 * slot - 0.3 * rim;
            shade = 1.0 - 0.5 * rim;
        }
        case SMARK_SEAM: {
            // A weld bead down the middle, rippled where each pass was laid.
            let across = abs(p.y);
            let bead = 1.0 - smoothstep(0.012, 0.028, across);
            let ripple = 0.5 + 0.5 * sin(p.x * 140.0);
            h = 1.0 + 0.4 * bead * (0.8 + 0.2 * ripple);
            shade = 1.0 - 0.35 * surf_band(across - 0.03, 0.006, fw);
        }
        default: {}
    }
    // Clipped to its box; nothing finer than a pixel or two.
    let keep = surf_step(inside, 0.0, fw);
    let seen = 1.0 - smoothstep(0.012, 0.035, fw);
    return vec2<f32>(mix(1.0, h, keep * seen), mix(1.0, shade, keep * mix(0.5, 1.0, seen)));
}

// A lamp slot's light at `q` (`SMARK_LAMP`), 0 to 1: a bright core down the slot.
fn lamp_light(q: vec2<f32>, half: vec2<f32>, fw: f32) -> f32 {
    let turned = half.y > half.x;
    let p = select(q, q.yx, turned);
    let hf = select(half, half.yx, turned);
    return surf_step(0.022 - abs(p.y), 0.0, fw) * surf_step(hf.x - 0.05 - abs(p.x), 0.0, fw);
}

// A placed mark's paint (`SMARK_HAZARD`, `SMARK_STENCIL`, `SMARK_WARNING`, `SMARK_LABEL`)
// on its plane, over paint `under`, for a pixel `fw` metres wide: rgb and how much.
fn mark_paint(here: MarksHere, plane: MarkPlane, fw: f32, under: vec3<f32>) -> vec4<f32> {
    // Light marks on dark paint, dark on light.
    let lum = dot(under, vec3<f32>(0.3, 0.59, 0.11));
    let ink = select(vec3<f32>(0.02, 0.02, 0.022), vec3<f32>(0.62, 0.6, 0.55), lum < 0.04);
    let q = plane.q;
    let half = plane.half;
    let inside = surf_step(surf_edge(q, half), 0.0, fw);
    switch here.kind {
        case SMARK_HAZARD: {
            // Yellow and black on the diagonal.
            let w = 0.16;
            let band = step(0.5, fract((here.at.x + here.at.y + here.at.z) / (2.0 * w)));
            return vec4<f32>(mix(vec3<f32>(0.02, 0.02, 0.022), SURF_SAFETY, band), 0.92);
        }
        case SMARK_STENCIL: {
            // The number's digits left to right, a line as tall as the box.
            let height = half.y * 1.6;
            let u = q.x / height;
            let v = q.y / height;
            let value = u32(max(here.arg, 0.0));
            let digits = select(select(1u, 2u, value >= 10u), 3u, value >= 100u);
            let pitch = 0.72;
            let left = -0.5 * pitch * f32(digits - 1u);
            let k = clamp(round((u - left) / pitch), 0.0, f32(digits - 1u));
            let place = digits - 1u - u32(k);
            var d = value;
            for (var j = 0u; j < place; j++) {
                d = d / 10u;
            }
            // Each digit's box is half as wide as it is tall (`surf_digit`).
            let p = vec2<f32>((u - left - k * pitch) * 1.6, v * 2.0);
            let on = surf_digit(p, d % 10u, fw / height * 2.0);
            return vec4<f32>(ink, on * 0.95);
        }
        case SMARK_WARNING: {
            // A yellow triangle edged black, with a bar and a dot.
            let r = min(half.x, half.y);
            let t = vec2<f32>(abs(q.x), q.y + r * 0.35);
            let tri = max(t.x * 0.866 + t.y * 0.5 - r * 0.5, -t.y);
            let fill = surf_step(-tri, 0.0, fw);
            let edge = surf_step(-tri, r * 0.09, fw);
            let bar = surf_step(r * 0.05 - abs(q.x), 0.0, fw) * surf_step(r * 0.16 - abs(q.y - r * 0.02), 0.0, fw);
            let spot = surf_step(r * 0.06 - length(q - vec2<f32>(0.0, -r * 0.24)), 0.0, fw);
            let black = (fill - edge) + max(bar, spot) * edge;
            let rgb = mix(SURF_SAFETY, vec3<f32>(0.02, 0.02, 0.022), saturate(black));
            return vec4<f32>(rgb, fill * 0.95);
        }
        case SMARK_LABEL: {
            // Lines of small lettering: words of set lengths (`arg` picks them), letters
            // as narrow strokes with gaps between words.
            let line = 0.05;
            let rows = max(floor(2.0 * half.y / line), 1.0);
            let row = floor((half.y - q.y) / line);
            let y = fract((half.y - q.y) / line);
            let x = q.x + half.x;
            let letter = 0.032;
            let k = floor(x / letter);
            // Word breaks: every few letters, the number of them set by the label and row.
            let word = 3.0 + floor(hash11(here.arg * 7.1 + row * 3.3 + floor(k / 6.0)) * 4.0);
            let gap = select(1.0, 0.0, (k % (word + 1.0)) >= word);
            let length_ok = step(x, 2.0 * half.x * (0.55 + 0.45 * hash11(here.arg + row * 1.7)));
            let stroke = surf_step(0.3 - abs(fract(x / letter) - 0.5), 0.0, fw / letter)
                * surf_step(0.3 - abs(y - 0.45), 0.0, fw / line);
            let on = stroke * gap * length_ok * step(row, rows - 1.0) * inside;
            return vec4<f32>(ink, on * 0.85);
        }
        default: {}
    }
    return vec4<f32>(0.0);
}
