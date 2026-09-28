// After the tone map: SMAA 1x on the finished picture, and FSR 1 when the
// scene is drawn smaller than the output (renderer/post.rs).
//
// Every image here holds sRGB-encoded colour in a UNORM format, which is what
// both SMAA's edge search and FSR's filters expect. Only the passes that write
// the swapchain (an _SRGB format) decode back to linear.
//
// SMAA: Jorge Jimenez, Jose I. Echevarria, Belen Masia, Fernando Navarro and
// Diego Gutierrez, "SMAA: Enhanced Subpixel Morphological Antialiasing" (2012).
// FSR 1: AMD FidelityFX Super Resolution 1.0 (EASU + RCAS). Both ported from
// their reference sources under the MIT licence; the licences are in
// assets/smaa/LICENSE.txt and assets/fsr/LICENSE.txt.

@group(0) @binding(0) var post_input: texture_2d<f32>;
@group(0) @binding(1) var area_tex: texture_2d<f32>;
@group(0) @binding(2) var search_tex: texture_2d<f32>;
@group(0) @binding(3) var blend_tex: texture_2d<f32>;
@group(0) @binding(4) var post_linear: sampler;
@group(0) @binding(5) var post_point: sampler;

struct PostPush {
    // EASU: xy the output's size. RCAS: x sharpness as a linear factor (`exp2(-stops)`).
    a: vec4<f32>,
}

var<immediate> post: PostPush;

struct PostOut {
    @builtin(position) clip: vec4<f32>,
}

@vertex
fn vs_post(@builtin(vertex_index) index: u32) -> PostOut {
    let p = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var out: PostOut;
    out.clip = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    return out;
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

// ---- SMAA 1x, "high" preset ------------------------------------------------

const SMAA_THRESHOLD: f32 = 0.1;
const SMAA_MAX_SEARCH_STEPS: f32 = 16.0;
const SMAA_MAX_SEARCH_STEPS_DIAG: f32 = 8.0;
const SMAA_CORNER_ROUNDING_NORM: f32 = 0.25;
const SMAA_LOCAL_CONTRAST_ADAPTATION_FACTOR: f32 = 2.0;
const SMAA_AREATEX_MAX_DISTANCE: f32 = 16.0;
const SMAA_AREATEX_MAX_DISTANCE_DIAG: f32 = 20.0;
const SMAA_AREATEX_PIXEL_SIZE: vec2<f32> = vec2<f32>(1.0 / 160.0, 1.0 / 560.0);
const SMAA_AREATEX_SUBTEX_SIZE: f32 = 1.0 / 7.0;
const SMAA_SEARCHTEX_SIZE: vec2<f32> = vec2<f32>(66.0, 33.0);
const SMAA_SEARCHTEX_PACKED_SIZE: vec2<f32> = vec2<f32>(64.0, 16.0);

// 1 / size, size of the image being worked on (all SMAA images share it).
fn smaa_rt() -> vec4<f32> {
    let d = vec2<f32>(textureDimensions(post_input));
    return vec4<f32>(1.0 / d, d);
}

fn smaa_luma(uv: vec2<f32>) -> f32 {
    return dot(textureSampleLevel(post_input, post_point, uv, 0.0).rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// First pass: which pixels have an edge on their left (r) and top (g).
@fragment
fn fs_smaa_edges(in: PostOut) -> @location(0) vec4<f32> {
    let m = smaa_rt();
    let uv = in.clip.xy * m.xy;
    let o0 = m.xyxy * vec4<f32>(-1.0, 0.0, 0.0, -1.0) + uv.xyxy;
    let o1 = m.xyxy * vec4<f32>(1.0, 0.0, 0.0, 1.0) + uv.xyxy;
    let o2 = m.xyxy * vec4<f32>(-2.0, 0.0, 0.0, -2.0) + uv.xyxy;

    let l = smaa_luma(uv);
    let l_left = smaa_luma(o0.xy);
    let l_top = smaa_luma(o0.zw);
    let delta_near = abs(l - vec2<f32>(l_left, l_top));
    var edges = step(vec2<f32>(SMAA_THRESHOLD), delta_near);
    if dot(edges, vec2<f32>(1.0)) == 0.0 {
        discard;
    }
    let l_right = smaa_luma(o1.xy);
    let l_bottom = smaa_luma(o1.zw);
    var max_delta = max(delta_near, abs(l - vec2<f32>(l_right, l_bottom)));
    let l_left2 = smaa_luma(o2.xy);
    let l_top2 = smaa_luma(o2.zw);
    max_delta = max(max_delta, abs(vec2<f32>(l_left, l_top) - vec2<f32>(l_left2, l_top2)));
    let final_delta = max(max_delta.x, max_delta.y);
    // Local contrast adaptation: a weak edge beside a much stronger one is not an edge.
    edges *= step(vec2<f32>(final_delta), SMAA_LOCAL_CONTRAST_ADAPTATION_FACTOR * delta_near);
    return vec4<f32>(edges, 0.0, 0.0);
}

fn edges_at(uv: vec2<f32>) -> vec2<f32> {
    return textureSampleLevel(post_input, post_linear, uv, 0.0).rg;
}

// Two binary edges from one bilinear fetch placed a quarter texel off.
fn decode_diag2(e_in: vec2<f32>) -> vec2<f32> {
    var e = e_in;
    e.x = e.x * abs(5.0 * e.x - 5.0 * 0.75);
    return round(e);
}

fn decode_diag4(e_in: vec4<f32>) -> vec4<f32> {
    var e = e_in;
    e.x = e.x * abs(5.0 * e.x - 5.0 * 0.75);
    e.z = e.z * abs(5.0 * e.z - 5.0 * 0.75);
    return round(e);
}

// Returns (steps taken, whether the last fetch was still an edge, last edges).
fn search_diag1(uv: vec2<f32>, dir: vec2<f32>, m: vec4<f32>) -> vec4<f32> {
    var coord = vec4<f32>(uv, -1.0, 1.0);
    let t = vec3<f32>(m.xy, 1.0);
    var e = vec2<f32>(0.0);
    loop {
        if !(coord.z < SMAA_MAX_SEARCH_STEPS_DIAG - 1.0 && coord.w > 0.9) {
            break;
        }
        let xyz = t * vec3<f32>(dir, 1.0) + coord.xyz;
        coord = vec4<f32>(xyz, coord.w);
        e = edges_at(coord.xy);
        coord.w = dot(e, vec2<f32>(0.5));
    }
    return vec4<f32>(coord.zw, e);
}

fn search_diag2(uv: vec2<f32>, dir: vec2<f32>, m: vec4<f32>) -> vec4<f32> {
    var coord = vec4<f32>(uv, -1.0, 1.0);
    coord.x += 0.25 * m.x;
    let t = vec3<f32>(m.xy, 1.0);
    var e = vec2<f32>(0.0);
    loop {
        if !(coord.z < SMAA_MAX_SEARCH_STEPS_DIAG - 1.0 && coord.w > 0.9) {
            break;
        }
        let xyz = t * vec3<f32>(dir, 1.0) + coord.xyz;
        coord = vec4<f32>(xyz, coord.w);
        e = decode_diag2(edges_at(coord.xy));
        coord.w = dot(e, vec2<f32>(0.5));
    }
    return vec4<f32>(coord.zw, e);
}

fn area_diag(dist: vec2<f32>, e: vec2<f32>, offset: f32) -> vec2<f32> {
    var texcoord = vec2<f32>(SMAA_AREATEX_MAX_DISTANCE_DIAG) * e + dist;
    texcoord = SMAA_AREATEX_PIXEL_SIZE * texcoord + 0.5 * SMAA_AREATEX_PIXEL_SIZE;
    texcoord.x += 0.5;
    texcoord.y += SMAA_AREATEX_SUBTEX_SIZE * offset;
    return textureSampleLevel(area_tex, post_linear, texcoord, 0.0).rg;
}

fn diag_weights(uv: vec2<f32>, e: vec2<f32>, m: vec4<f32>) -> vec2<f32> {
    var weights = vec2<f32>(0.0);
    var d = vec4<f32>(0.0);
    if e.x > 0.0 {
        let r = search_diag1(uv, vec2<f32>(-1.0, 1.0), m);
        d.x = r.x + f32(r.w > 0.9);
        d.z = r.y;
    }
    let r2 = search_diag1(uv, vec2<f32>(1.0, -1.0), m);
    d.y = r2.x;
    d.w = r2.y;
    if d.x + d.y > 2.0 {
        let coords = vec4<f32>(-d.x + 0.25, d.x, d.y, -d.y - 0.25) * m.xyxy + uv.xyxy;
        let raw = vec4<f32>(
            textureSampleLevel(post_input, post_linear, coords.xy, 0.0, vec2<i32>(-1, 0)).rg,
            textureSampleLevel(post_input, post_linear, coords.zw, 0.0, vec2<i32>(1, 0)).rg,
        );
        let dc = decode_diag4(raw);
        let c = vec4<f32>(dc.y, dc.x, dc.w, dc.z);
        var cc = vec2<f32>(2.0) * c.xz + c.yw;
        // No crossing edge where the line's end was not found.
        cc = select(cc, vec2<f32>(0.0), step(vec2<f32>(0.9), d.zw) > vec2<f32>(0.5));
        weights += area_diag(d.xy, cc, 0.0);
    }

    let r3 = search_diag2(uv, vec2<f32>(-1.0, -1.0), m);
    d.x = r3.x;
    d.z = r3.y;
    if textureSampleLevel(post_input, post_linear, uv, 0.0, vec2<i32>(1, 0)).r > 0.0 {
        let r4 = search_diag2(uv, vec2<f32>(1.0, 1.0), m);
        d.y = r4.x + f32(r4.w > 0.9);
        d.w = r4.y;
    } else {
        d.y = 0.0;
        d.w = 0.0;
    }
    if d.x + d.y > 2.0 {
        let coords = vec4<f32>(-d.x, -d.x, d.y, d.y) * m.xyxy + uv.xyxy;
        let c = vec4<f32>(
            textureSampleLevel(post_input, post_linear, coords.xy, 0.0, vec2<i32>(-1, 0)).g,
            textureSampleLevel(post_input, post_linear, coords.xy, 0.0, vec2<i32>(0, -1)).r,
            textureSampleLevel(post_input, post_linear, coords.zw, 0.0, vec2<i32>(1, 0)).gr,
        );
        var cc = vec2<f32>(2.0) * c.xz + c.yw;
        cc = select(cc, vec2<f32>(0.0), step(vec2<f32>(0.9), d.zw) > vec2<f32>(0.5));
        weights += area_diag(d.xy, cc, 0.0).yx;
    }
    return weights;
}

// How far the last step of a search overshot, from the edges it fetched.
fn search_length(e: vec2<f32>, offset: f32) -> f32 {
    var scale = SMAA_SEARCHTEX_SIZE * vec2<f32>(0.5, -1.0);
    var bias = SMAA_SEARCHTEX_SIZE * vec2<f32>(offset, 1.0);
    scale += vec2<f32>(-1.0, 1.0);
    bias += vec2<f32>(0.5, -0.5);
    scale /= SMAA_SEARCHTEX_PACKED_SIZE;
    bias /= SMAA_SEARCHTEX_PACKED_SIZE;
    return textureSampleLevel(search_tex, post_linear, scale * e + bias, 0.0).r;
}

// The searches fetch between texels so one bilinear tap reads two edges
// (`uv` was offset for that by the caller). Each loop is also capped, so a
// bad fetch can never keep one going.
fn search_x_left(uv: vec2<f32>, end: f32, m: vec4<f32>) -> f32 {
    var t = uv;
    var e = vec2<f32>(0.0, 1.0);
    for (var i = 0; i < 20; i++) {
        if !(t.x > end && e.y > 0.8281 && e.x == 0.0) {
            break;
        }
        e = edges_at(t);
        t.x -= 2.0 * m.x;
    }
    let offset = -(255.0 / 127.0) * search_length(e, 0.0) + 3.25;
    return m.x * offset + t.x;
}

fn search_x_right(uv: vec2<f32>, end: f32, m: vec4<f32>) -> f32 {
    var t = uv;
    var e = vec2<f32>(0.0, 1.0);
    for (var i = 0; i < 20; i++) {
        if !(t.x < end && e.y > 0.8281 && e.x == 0.0) {
            break;
        }
        e = edges_at(t);
        t.x += 2.0 * m.x;
    }
    let offset = -(255.0 / 127.0) * search_length(e, 0.5) + 3.25;
    return -m.x * offset + t.x;
}

fn search_y_up(uv: vec2<f32>, end: f32, m: vec4<f32>) -> f32 {
    var t = uv;
    var e = vec2<f32>(1.0, 0.0);
    for (var i = 0; i < 20; i++) {
        if !(t.y > end && e.x > 0.8281 && e.y == 0.0) {
            break;
        }
        e = edges_at(t);
        t.y -= 2.0 * m.y;
    }
    let offset = -(255.0 / 127.0) * search_length(e.yx, 0.0) + 3.25;
    return m.y * offset + t.y;
}

fn search_y_down(uv: vec2<f32>, end: f32, m: vec4<f32>) -> f32 {
    var t = uv;
    var e = vec2<f32>(1.0, 0.0);
    for (var i = 0; i < 20; i++) {
        if !(t.y < end && e.x > 0.8281 && e.y == 0.0) {
            break;
        }
        e = edges_at(t);
        t.y += 2.0 * m.y;
    }
    let offset = -(255.0 / 127.0) * search_length(e.yx, 0.5) + 3.25;
    return -m.y * offset + t.y;
}

fn smaa_area(dist: vec2<f32>, e1: f32, e2: f32, offset: f32) -> vec2<f32> {
    var texcoord = vec2<f32>(SMAA_AREATEX_MAX_DISTANCE) * round(4.0 * vec2<f32>(e1, e2)) + dist;
    texcoord = SMAA_AREATEX_PIXEL_SIZE * texcoord + 0.5 * SMAA_AREATEX_PIXEL_SIZE;
    texcoord.y = SMAA_AREATEX_SUBTEX_SIZE * offset + texcoord.y;
    return textureSampleLevel(area_tex, post_linear, texcoord, 0.0).rg;
}

fn corner_rounding(d: vec2<f32>) -> vec2<f32> {
    let left_right = step(d.xy, d.yx);
    var rounding = (1.0 - SMAA_CORNER_ROUNDING_NORM) * left_right;
    // Less blending for pixels in the middle of a line.
    return rounding / (left_right.x + left_right.y);
}

fn corner_horizontal(weights: vec2<f32>, texcoord: vec4<f32>, d: vec2<f32>) -> vec2<f32> {
    let rounding = corner_rounding(d);
    var factor = vec2<f32>(1.0);
    factor.x -= rounding.x * textureSampleLevel(post_input, post_linear, texcoord.xy, 0.0, vec2<i32>(0, 1)).r;
    factor.x -= rounding.y * textureSampleLevel(post_input, post_linear, texcoord.zw, 0.0, vec2<i32>(1, 1)).r;
    factor.y -= rounding.x * textureSampleLevel(post_input, post_linear, texcoord.xy, 0.0, vec2<i32>(0, -2)).r;
    factor.y -= rounding.y * textureSampleLevel(post_input, post_linear, texcoord.zw, 0.0, vec2<i32>(1, -2)).r;
    return weights * clamp(factor, vec2<f32>(0.0), vec2<f32>(1.0));
}

fn corner_vertical(weights: vec2<f32>, texcoord: vec4<f32>, d: vec2<f32>) -> vec2<f32> {
    let rounding = corner_rounding(d);
    var factor = vec2<f32>(1.0);
    factor.x -= rounding.x * textureSampleLevel(post_input, post_linear, texcoord.xy, 0.0, vec2<i32>(1, 0)).g;
    factor.x -= rounding.y * textureSampleLevel(post_input, post_linear, texcoord.zw, 0.0, vec2<i32>(1, 1)).g;
    factor.y -= rounding.x * textureSampleLevel(post_input, post_linear, texcoord.xy, 0.0, vec2<i32>(-2, 0)).g;
    factor.y -= rounding.y * textureSampleLevel(post_input, post_linear, texcoord.zw, 0.0, vec2<i32>(-2, 1)).g;
    return weights * clamp(factor, vec2<f32>(0.0), vec2<f32>(1.0));
}

// Second pass: how much each edge pixel blends with its neighbours, from the
// shape of the line it lies on. `post_input` is the edges.
@fragment
fn fs_smaa_weights(in: PostOut) -> @location(0) vec4<f32> {
    let m = smaa_rt();
    let uv = in.clip.xy * m.xy;
    let pix = in.clip.xy;
    var weights = vec4<f32>(0.0);
    var e = textureSampleLevel(post_input, post_point, uv, 0.0).rg;
    if e.x + e.y == 0.0 {
        return weights;
    }
    let o0 = m.xyxy * vec4<f32>(-0.25, -0.125, 1.25, -0.125) + uv.xyxy;
    let o1 = m.xyxy * vec4<f32>(-0.125, -0.25, -0.125, 1.25) + uv.xyxy;
    let o2 = m.xxyy * vec4<f32>(-2.0, 2.0, -2.0, 2.0) * SMAA_MAX_SEARCH_STEPS + vec4<f32>(o0.xz, o1.yw);

    if e.y > 0.0 {
        // Edge at the top. Diagonals have both edges, so searching from one is enough,
        // and a diagonal found wins over the horizontal and vertical lines.
        let diag = diag_weights(uv, e, m);
        weights.x = diag.x;
        weights.y = diag.y;
        if diag.x == -diag.y {
            var coords = vec3<f32>(0.0);
            coords.x = search_x_left(o0.xy, o2.x, m);
            coords.y = o1.y;
            var d = vec2<f32>(coords.x, 0.0);
            let e1 = edges_at(coords.xy).r;
            coords.z = search_x_right(o0.zw, o2.y, m);
            d.y = coords.z;
            d = abs(round(m.zz * d - pix.xx));
            let sqrt_d = sqrt(d);
            let e2 = textureSampleLevel(post_input, post_linear, coords.zy, 0.0, vec2<i32>(1, 0)).r;
            var w = smaa_area(sqrt_d, e1, e2, 0.0);
            coords.y = uv.y;
            w = corner_horizontal(w, coords.xyzy, d);
            weights.x = w.x;
            weights.y = w.y;
        } else {
            e.x = 0.0;
        }
    }
    if e.x > 0.0 {
        // Edge at the left.
        var coords = vec3<f32>(0.0);
        coords.y = search_y_up(o1.xy, o2.z, m);
        coords.x = o0.x;
        var d = vec2<f32>(coords.y, 0.0);
        let e1 = edges_at(coords.xy).g;
        coords.z = search_y_down(o1.zw, o2.w, m);
        d.y = coords.z;
        d = abs(round(m.ww * d - pix.yy));
        let sqrt_d = sqrt(d);
        let e2 = textureSampleLevel(post_input, post_linear, coords.xz, 0.0, vec2<i32>(0, 1)).g;
        var w = smaa_area(sqrt_d, e1, e2, 0.0);
        coords.x = uv.x;
        w = corner_vertical(w, coords.xyxz, d);
        weights.z = w.x;
        weights.w = w.y;
    }
    return weights;
}

// Third pass: each pixel mixes with the neighbour its weights point to.
// `post_input` is the picture, `blend_tex` the weights.
fn smaa_blend(clip: vec2<f32>) -> vec4<f32> {
    let m = smaa_rt();
    let uv = clip * m.xy;
    let offset = m.xyxy * vec4<f32>(1.0, 0.0, 0.0, 1.0) + uv.xyxy;
    let here = textureSampleLevel(blend_tex, post_linear, uv, 0.0);
    var a = vec4<f32>(
        textureSampleLevel(blend_tex, post_linear, offset.xy, 0.0).a,
        textureSampleLevel(blend_tex, post_linear, offset.zw, 0.0).g,
        here.z,
        here.x,
    );
    if dot(a, vec4<f32>(1.0)) < 1e-5 {
        return textureSampleLevel(post_input, post_point, uv, 0.0);
    }
    let horizontal = max(a.x, a.z) > max(a.y, a.w);
    var blend_offset = vec4<f32>(0.0, a.y, 0.0, a.w);
    var blend_weight = a.yw;
    if horizontal {
        blend_offset = vec4<f32>(a.x, 0.0, a.z, 0.0);
        blend_weight = a.xz;
    }
    blend_weight /= dot(blend_weight, vec2<f32>(1.0));
    let blend_coord = blend_offset * vec4<f32>(m.xy, -m.xy) + uv.xyxy;
    return blend_weight.x * textureSampleLevel(post_input, post_linear, blend_coord.xy, 0.0)
        + blend_weight.y * textureSampleLevel(post_input, post_linear, blend_coord.zw, 0.0);
}

// Into another encoded image, for the upscaler.
@fragment
fn fs_smaa_blend(in: PostOut) -> @location(0) vec4<f32> {
    return vec4<f32>(smaa_blend(in.clip.xy).rgb, 1.0);
}

// Straight to the swapchain.
@fragment
fn fs_smaa_present(in: PostOut) -> @location(0) vec4<f32> {
    return vec4<f32>(srgb_to_linear(smaa_blend(in.clip.xy).rgb), 1.0);
}

// ---- FSR 1 ---------------------------------------------------------------

// Output pixel (at its centre) to the input's texels, as FsrEasuCon sets up.
fn easu_tap(
    ac: ptr<function, vec3<f32>>,
    aw: ptr<function, f32>,
    off: vec2<f32>,
    dir: vec2<f32>,
    len: vec2<f32>,
    lob: f32,
    clp: f32,
    c: vec3<f32>,
) {
    var v = vec2<f32>(off.x * dir.x + off.y * dir.y, off.x * -dir.y + off.y * dir.x);
    v *= len;
    let d2 = min(v.x * v.x + v.y * v.y, clp);
    // Lanczos 2 approximated without sin, sqrt or division.
    var wb = 2.0 / 5.0 * d2 - 1.0;
    var wa = lob * d2 - 1.0;
    wb *= wb;
    wa *= wa;
    wb = 25.0 / 16.0 * wb - (25.0 / 16.0 - 1.0);
    let w = wb * wa;
    *ac += c * w;
    *aw += w;
}

fn easu_set(
    dir: ptr<function, vec2<f32>>,
    len: ptr<function, f32>,
    w: f32,
    la: f32,
    lb: f32,
    lc: f32,
    ld: f32,
    le: f32,
) {
    let dc = ld - lc;
    let cb = lc - lb;
    var len_x = 1.0 / max(max(abs(dc), abs(cb)), 1e-8);
    let dir_x = ld - lb;
    (*dir).x += dir_x * w;
    len_x = clamp(abs(dir_x) * len_x, 0.0, 1.0);
    len_x *= len_x;
    *len += len_x * w;
    let ec = le - lc;
    let ca = lc - la;
    var len_y = 1.0 / max(max(abs(ec), abs(ca)), 1e-8);
    let dir_y = le - la;
    (*dir).y += dir_y * w;
    len_y = clamp(abs(dir_y) * len_y, 0.0, 1.0);
    len_y *= len_y;
    *len += len_y * w;
}

// Edge-adaptive upscale: a Lanczos-like kernel stretched along the local edge,
// clamped to the four nearest texels so it cannot ring.
@fragment
fn fs_easu(in: PostOut) -> @location(0) vec4<f32> {
    let input_size = vec2<f32>(textureDimensions(post_input));
    let output_size = post.a.xy;
    let ip = floor(in.clip.xy);
    let scale = input_size / output_size;
    var pp = ip * scale + (0.5 * scale - 0.5);
    let fp = floor(pp);
    pp -= fp;
    let rcp = 1.0 / input_size;
    let p0 = fp * rcp + vec2<f32>(1.0, -1.0) * rcp;
    let p1 = p0 + vec2<f32>(-1.0, 2.0) * rcp;
    let p2 = p0 + vec2<f32>(1.0, 2.0) * rcp;
    let p3 = p0 + vec2<f32>(0.0, 4.0) * rcp;
    let bczz_r = textureGather(0, post_input, post_point, p0);
    let bczz_g = textureGather(1, post_input, post_point, p0);
    let bczz_b = textureGather(2, post_input, post_point, p0);
    let ijfe_r = textureGather(0, post_input, post_point, p1);
    let ijfe_g = textureGather(1, post_input, post_point, p1);
    let ijfe_b = textureGather(2, post_input, post_point, p1);
    let klhg_r = textureGather(0, post_input, post_point, p2);
    let klhg_g = textureGather(1, post_input, post_point, p2);
    let klhg_b = textureGather(2, post_input, post_point, p2);
    let zzon_r = textureGather(0, post_input, post_point, p3);
    let zzon_g = textureGather(1, post_input, post_point, p3);
    let zzon_b = textureGather(2, post_input, post_point, p3);
    // Luma times two.
    let bczz_l = bczz_b * 0.5 + (bczz_r * 0.5 + bczz_g);
    let ijfe_l = ijfe_b * 0.5 + (ijfe_r * 0.5 + ijfe_g);
    let klhg_l = klhg_b * 0.5 + (klhg_r * 0.5 + klhg_g);
    let zzon_l = zzon_b * 0.5 + (zzon_r * 0.5 + zzon_g);
    let b_l = bczz_l.x;
    let c_l = bczz_l.y;
    let i_l = ijfe_l.x;
    let j_l = ijfe_l.y;
    let f_l = ijfe_l.z;
    let e_l = ijfe_l.w;
    let k_l = klhg_l.x;
    let l_l = klhg_l.y;
    let h_l = klhg_l.z;
    let g_l = klhg_l.w;
    let o_l = zzon_l.z;
    let n_l = zzon_l.w;
    var dir = vec2<f32>(0.0);
    var len = 0.0;
    easu_set(&dir, &len, (1.0 - pp.x) * (1.0 - pp.y), b_l, e_l, f_l, g_l, j_l);
    easu_set(&dir, &len, pp.x * (1.0 - pp.y), c_l, f_l, g_l, h_l, k_l);
    easu_set(&dir, &len, (1.0 - pp.x) * pp.y, f_l, i_l, j_l, k_l, n_l);
    easu_set(&dir, &len, pp.x * pp.y, g_l, j_l, k_l, l_l, o_l);
    let dir2 = dir * dir;
    var dir_r = dir2.x + dir2.y;
    let zero = dir_r < 1.0 / 32768.0;
    dir_r = select(inverseSqrt(max(dir_r, 1e-12)), 1.0, zero);
    dir.x = select(dir.x, 1.0, zero);
    dir *= dir_r;
    len = len * 0.5;
    len *= len;
    let stretch = (dir.x * dir.x + dir.y * dir.y) / max(max(abs(dir.x), abs(dir.y)), 1e-8);
    let len2 = vec2<f32>(1.0 + (stretch - 1.0) * len, 1.0 - 0.5 * len);
    let lob = 0.5 + ((1.0 / 4.0 - 0.04) - 0.5) * len;
    let clp = 1.0 / lob;
    let f = vec3<f32>(ijfe_r.z, ijfe_g.z, ijfe_b.z);
    let g = vec3<f32>(klhg_r.w, klhg_g.w, klhg_b.w);
    let j = vec3<f32>(ijfe_r.y, ijfe_g.y, ijfe_b.y);
    let k = vec3<f32>(klhg_r.x, klhg_g.x, klhg_b.x);
    let min4 = min(min(f, g), min(j, k));
    let max4 = max(max(f, g), max(j, k));
    var ac = vec3<f32>(0.0);
    var aw = 0.0;
    easu_tap(&ac, &aw, vec2<f32>(0.0, -1.0) - pp, dir, len2, lob, clp, vec3<f32>(bczz_r.x, bczz_g.x, bczz_b.x));
    easu_tap(&ac, &aw, vec2<f32>(1.0, -1.0) - pp, dir, len2, lob, clp, vec3<f32>(bczz_r.y, bczz_g.y, bczz_b.y));
    easu_tap(&ac, &aw, vec2<f32>(-1.0, 1.0) - pp, dir, len2, lob, clp, vec3<f32>(ijfe_r.x, ijfe_g.x, ijfe_b.x));
    easu_tap(&ac, &aw, vec2<f32>(0.0, 1.0) - pp, dir, len2, lob, clp, j);
    easu_tap(&ac, &aw, vec2<f32>(0.0, 0.0) - pp, dir, len2, lob, clp, f);
    easu_tap(&ac, &aw, vec2<f32>(-1.0, 0.0) - pp, dir, len2, lob, clp, vec3<f32>(ijfe_r.w, ijfe_g.w, ijfe_b.w));
    easu_tap(&ac, &aw, vec2<f32>(1.0, 1.0) - pp, dir, len2, lob, clp, k);
    easu_tap(&ac, &aw, vec2<f32>(2.0, 1.0) - pp, dir, len2, lob, clp, vec3<f32>(klhg_r.y, klhg_g.y, klhg_b.y));
    easu_tap(&ac, &aw, vec2<f32>(2.0, 0.0) - pp, dir, len2, lob, clp, vec3<f32>(klhg_r.z, klhg_g.z, klhg_b.z));
    easu_tap(&ac, &aw, vec2<f32>(1.0, 0.0) - pp, dir, len2, lob, clp, g);
    easu_tap(&ac, &aw, vec2<f32>(1.0, 2.0) - pp, dir, len2, lob, clp, vec3<f32>(zzon_r.z, zzon_g.z, zzon_b.z));
    easu_tap(&ac, &aw, vec2<f32>(0.0, 2.0) - pp, dir, len2, lob, clp, vec3<f32>(zzon_r.w, zzon_g.w, zzon_b.w));
    let pix = min(max4, max(min4, ac / aw));
    return vec4<f32>(pix, 1.0);
}

// Contrast-adaptive sharpening of the upscaled picture, to the swapchain.
const FSR_RCAS_LIMIT: f32 = 0.25 - 1.0 / 16.0;

fn rcas_load(p: vec2<i32>) -> vec3<f32> {
    let dims = vec2<i32>(textureDimensions(post_input));
    return textureLoad(post_input, clamp(p, vec2<i32>(0), dims - 1), 0).rgb;
}

@fragment
fn fs_rcas_present(in: PostOut) -> @location(0) vec4<f32> {
    let sp = vec2<i32>(in.clip.xy);
    let b = rcas_load(sp + vec2<i32>(0, -1));
    let d = rcas_load(sp + vec2<i32>(-1, 0));
    let e = rcas_load(sp);
    let f = rcas_load(sp + vec2<i32>(1, 0));
    let h = rcas_load(sp + vec2<i32>(0, 1));
    let mn4 = min(min(b, d), min(f, h));
    let mx4 = max(max(b, d), max(f, h));
    let hit_min = min(mn4, e) / (4.0 * mx4 + 1e-5);
    let hit_max = (1.0 - max(mx4, e)) / (4.0 * mn4 - 4.0 - 1e-5);
    let lobe_rgb = max(-hit_min, hit_max);
    let lobe = max(-FSR_RCAS_LIMIT, min(max(lobe_rgb.r, max(lobe_rgb.g, lobe_rgb.b)), 0.0)) * post.a.x;
    let pix = (lobe * (b + d + h + f) + e) / (4.0 * lobe + 1.0);
    return vec4<f32>(srgb_to_linear(clamp(pix, vec3<f32>(0.0), vec3<f32>(1.0))), 1.0);
}
