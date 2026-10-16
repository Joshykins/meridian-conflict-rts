// Trees and rocks, the leading prop kinds (renderer `ActiveDraws::props`): their
// vertex stages and what they hand on (entity.wgsl draws everything else). A forest
// is hundreds of thousands of these, each a few corners, in the colour pass, the
// pre-pass and every shadow cascade, so each pass gets only what it reads.

// Where a prop's vertex lands, and its normal and scaled model point: `vs_prop` and
// `vs_prop_depth` both place it here, so the pre-pass and the colour pass agree on
// its depth to the bit (`clip` is invariant in both).
struct PropPlaced {
    world: vec3<f32>,
    normal: vec3<f32>,
    local: vec3<f32>,
}

// The props' meshes and draws (renderer.rs `mesh_vb`, `mesh_ib`; cull.wgsl `commands`),
// for `prop_vertex`.
@group(0) @binding(40) var<storage, read> draw_words: array<u32>;
@group(0) @binding(41) var<storage, read> mesh_words: array<u32>;
@group(0) @binding(42) var<storage, read> mesh_indices: array<u32>;
// Words of a `DrawCommand` (cull.wgsl) and of a `MeshVertex` (mc-models).
const DRAW_WORDS: u32 = 5u;
const MESH_VERTEX_WORDS: u32 = 16u;

// The mesh vertex and instance that vertex `vertex` of draw `command` (cull.wgsl
// `PulledDraw`) stands for. The props draw each slot as one run of vertices, every
// instance's indices one after another, rather than as an instance per tree: a far
// tree is a few corners, and drawn as instances each tree took a vertex batch of its
// own, mostly empty, so a forest cost by its trees and not by its vertices. Only the
// depth passes draw so (`vs_prop_depth`): pulled, a vertex runs once per index, which
// their few outputs repay and the colour pass's twenty do not.
fn prop_vertex(vertex: u32, command: u32) -> VsIn {
    let c = command * DRAW_WORDS;
    let count = draw_words[c];
    let tree = vertex / count;
    let index = mesh_indices[draw_words[c + 2u] + vertex - tree * count];
    let w = u32(i32(index) + bitcast<i32>(draw_words[c + 3u])) * MESH_VERTEX_WORDS;
    var in: VsIn;
    in.pos = vec3<f32>(bitcast<f32>(mesh_words[w]), bitcast<f32>(mesh_words[w + 1u]), bitcast<f32>(mesh_words[w + 2u]));
    in.normal = vec3<f32>(bitcast<f32>(mesh_words[w + 3u]), bitcast<f32>(mesh_words[w + 4u]), bitcast<f32>(mesh_words[w + 5u]));
    in.uv = vec2<f32>(bitcast<f32>(mesh_words[w + 6u]), bitcast<f32>(mesh_words[w + 7u]));
    in.material = mesh_words[w + 8u];
    in.part = mesh_words[w + 9u];
    in.rig = mesh_words[w + 10u];
    in.face = vec4<f32>(
        bitcast<f32>(mesh_words[w + 11u]),
        bitcast<f32>(mesh_words[w + 12u]),
        bitcast<f32>(mesh_words[w + 13u]),
        bitcast<f32>(mesh_words[w + 14u]),
    );
    in.surface = mesh_words[w + 15u];
    in.instance = draw_words[c + 4u] + tree;
    return in;
}

fn prop_place(in: VsIn) -> PropPlaced {
    let entity_index = visible[in.instance];
    let e = load_entity(entity_index);
    let model = models[e.blueprint];
    let t = globals.sun.w;
    let time = globals.camera.w;
    let scale = select(1.0, f32(e.packed) * 0.001, e.packed != 0u) * thin_grow(entity_index);
    let stretch = select(1.0, e.arm_pitch.w, e.arm_pitch.w > 0.0);
    var p = in.pos;
    var n = in.normal;
    // A trampled tree tips over from its foot (renderer/fallen_trees.rs).
    let toppled = e.arm_pitch.x != 0.0;
    var tree = vec3<f32>(0.0);
    let is_tree = !toppled && (in.material == MAT_FOLIAGE || in.material == MAT_BARK);
    let tall = max(model.height * scale * stretch, 1.0);
    if is_tree {
        // As `vs_main`: no wind on a tree a few pixels tall.
        if tall * globals.lod.x < TREE_STILL_PX * distance(e.pos, globals.camera.xyz) {
            tree = blast_sway(entity_index);
        } else {
            tree = tree_air(e.pos, tall, e.unit_id, blast_sway(entity_index));
        }
    }
    if in.material == MAT_FOLIAGE {
        n = select(normalize(vec3<f32>(in.pos.xy * 0.18, 0.7)), in.face.xyz, dot(in.face.xyz, in.face.xyz) > 0.25);
        let flex = clamp(in.pos.z / max(model.height, 1.0), 0.0, 1.0);
        let card = f32((in.surface >> 8u) & 0x7Fu);
        let phase = time * 1.5 + (in.pos.x + in.pos.y * 0.7) * 0.35 + card * 0.37 + f32(e.unit_id % 31u);
        let shiver = time * 6.3 + card * 1.71 + in.pos.z * 0.3;
        p += (vec3<f32>(sin(phase), cos(phase * 0.83), 0.0) * 0.04
            + vec3<f32>(sin(shiver), cos(shiver * 1.13), sin(shiver * 0.71) * 0.5) * 0.07 * tree.z) * flex;
    }
    let heading = lerp_angle(e.prev_heading, e.heading, t);
    var origin = mix(e.prev_pos, e.pos, t);
    // Props carry an approximate height; stand them on the real surface.
    origin.z = terrain_height(origin.xy) - e.arm_pitch.z;
    let local = vec3<f32>(p.xy, p.z * stretch) * scale;
    if stretch != 1.0 {
        n = normalize(vec3<f32>(n.xy, n.z / stretch));
    }
    var up = vec3<f32>(0.0, 0.0, 1.0);
    let fwd0 = vec3<f32>(cos(heading), sin(heading), 0.0);
    var left = normalize(cross(up, fwd0));
    var fwd = cross(left, up);
    if toppled {
        let pitch = mix(e.arm_pitch.x, e.arm_pitch.y, t);
        let pitch_fwd = fwd * cos(pitch) + up * sin(pitch);
        up = up * cos(pitch) - fwd * sin(pitch);
        fwd = pitch_fwd;
    }
    let bank = mix(e._pad2.x, e._pad2.y, t);
    let bank_left = left * cos(bank) + up * sin(bank);
    up = up * cos(bank) - left * sin(bank);
    left = bank_left;
    var world = origin + fwd * local.x + left * local.y + up * local.z;
    var world_n = normalize(fwd * n.x + left * n.y + up * n.z);
    // The stem bends as a pole over its foot (`vs_main`).
    let reach = length(tree.xy);
    if is_tree && reach > 0.001 {
        let dir = tree.xy / reach;
        let bend = min(asin(min(reach / tall, 0.8)) * 1.5, 0.95);
        let h = world.z - origin.z;
        let r = max(h, 0.0) / tall;
        let f = select(1.0, r * (2.0 - r), r < 1.0);
        let mean = select((r - 1.0 / 3.0) / max(r, 0.001), r - r * r / 3.0, r < 1.0);
        let a = bend * mean;
        let turn = bend * f;
        let rel = world.xy - origin.xy;
        let along = dot(rel, dir);
        let stem_h = max(h, 0.0);
        world = vec3<f32>(
            origin.xy + rel + dir * (stem_h * sin(a) + along * (cos(turn) - 1.0)),
            origin.z + h + stem_h * (cos(a) - 1.0) - along * sin(turn),
        );
        let n_along = dot(world_n.xy, dir);
        world_n = normalize(vec3<f32>(
            world_n.xy + dir * (n_along * (cos(turn) - 1.0) + world_n.z * sin(turn)),
            world_n.z * cos(turn) - n_along * sin(turn),
        ));
    }
    return PropPlaced(world, world_n, local);
}

// Trees and rocks (the leading prop kinds, renderer `ActiveDraws::props`): `vs_main`
// for what a prop can be. Standing on the ground, scaled and stretched, a tree's
// leaves fluttering and its stem bent by the wind and blasts, a trampled one tipped
// over its foot; nothing a unit, wreck or ship needs. A forest is hundreds of
// thousands of these, and `vs_main`'s every other branch and its registers made
// the entity pass vertex-bound. The colour passes draw them with this, the pre-pass
// and the shadows with `vs_prop_depth`; both place the vertex with `prop_place`.
@vertex
fn vs_prop(in: VsIn) -> PropOut {
    let e = load_entity(visible[in.instance]);
    let model = models[e.blueprint];
    let placed = prop_place(in);
    let scale = select(1.0, f32(e.packed) * 0.001, e.packed != 0u) * thin_grow(visible[in.instance]);
    var out: PropOut;
    out.clip = globals.view_proj * vec4<f32>(placed.world, 1.0);
    out.world = placed.world;
    out.normal = placed.normal;
    out.uv = in.uv;
    out.material = in.material;
    out.owner_flags = e.owner_flags;
    out.state = vec4<f32>(e.build, e.health, in.pos.z / max(model.height, 0.1), hash11(f32(e.unit_id & 0xFFFFu)));
    out.local = in.pos;
    out.model_class = ((model.icon >> 8u) & 0xFFu) | ((in.surface & 0xFFFFu) << 16u);
    out.face = in.face;
    out.unit_id = e.unit_id;
    out.blueprint = e.blueprint;
    let warp_reach = max(model.bounds_radius * scale, 1.0);
    out.warp = clamp(0.5 + 0.5 * placed.local.x / warp_reach, 0.0, 1.0);
    return out;
}

// What a prop's depth passes (the pre-pass and the shadows) read of it: only what
// their cuts need (`fs_prop_prepass`, `fs_prop_shadow`). Handing on all of `VsOut`,
// twenty outputs for every vertex of every tree in four passes, was much of what
// a forest cost.
struct PropDepthOut {
    @builtin(position) @invariant clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) @interpolate(flat) material: u32,
    @location(3) @interpolate(flat) owner_flags: u32,
    // x health, y the point's share of the model's height (`VsOut::state` yz).
    @location(4) state: vec2<f32>,
    @location(5) local: vec3<f32>,
    // `VsOut::model_class`'s face pattern and random byte: the leaf atlas and tag.
    @location(6) @interpolate(flat) face_class: u32,
}

@vertex
fn vs_prop_depth(@builtin(vertex_index) vertex: u32, @builtin(instance_index) command: u32) -> PropDepthOut {
    let in = prop_vertex(vertex, command);
    let e = load_entity(visible[in.instance]);
    let model = models[e.blueprint];
    let world = prop_place(in).world;
    var out: PropDepthOut;
    if (push.pass_kind & PASS_KIND_MASK) == PASS_SHADOW {
        out.clip = globals.shadow_cascades[push.pass_kind >> PASS_CASCADE_SHIFT] * vec4<f32>(world, 1.0);
    } else {
        out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    }
    out.world = world;
    out.uv = in.uv;
    out.material = in.material;
    out.owner_flags = e.owner_flags;
    out.state = vec2<f32>(e.health, in.pos.z / max(model.height, 0.1));
    out.local = in.pos;
    out.face_class = (in.surface & 0xFFFFu) << 16u;
    return out;
}

// A prop's leaves past their outline or burnt away (`foliage_missing`).
fn prop_leaf_cut(in: PropDepthOut) -> bool {
    return in.material == MAT_FOLIAGE && leaf_cut(in.uv, in.local, in.state.x, foliage_texel(in.face_class, in.uv));
}

// `fs_prepass`'s cuts for a tree or a rock: its leaves, a tree coming apart in a
// clearing field, and the Precursor cutaway.
@fragment
fn fs_prop_prepass(in: PropDepthOut) {
    let vapor = (in.material == MAT_FOLIAGE || in.material == MAT_BARK)
        && vapor_at(in.owner_flags, in.state.x, in.state.y, in.local) > 0.0;
    if prop_leaf_cut(in) || vapor || precursor_cut(in.owner_flags, in.world, in.clip.xy) { discard; }
}

// `fs_shadow`'s cuts for a tree or a rock.
@fragment
fn fs_prop_shadow(in: PropDepthOut) {
    if prop_leaf_cut(in) || vapor_at(in.owner_flags, in.state.x, in.state.y, in.local) > 0.0 { discard; }
}

// What `fs_prop` reads of a tree or a rock: `VsOut` less what a prop always has the
// same (no drive, wreck, crackle, refit or weld) or takes from its model (`blueprint`).
// Twelve outputs for every vertex of a forest in place of twenty.
struct PropOut {
    @builtin(position) @invariant clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) @interpolate(flat) material: u32,
    @location(4) @interpolate(flat) owner_flags: u32,
    @location(5) state: vec4<f32>,
    @location(6) local: vec3<f32>,
    @location(7) @interpolate(flat) model_class: u32,
    @location(8) face: vec4<f32>,
    @location(9) @interpolate(flat) unit_id: u32,
    @location(10) @interpolate(flat) blueprint: u32,
    // `VsOut::warp.w`.
    @location(11) warp: f32,
}

// A prop's colour: `shade_entity` with what a prop never has held constant, so the
// driver compiles a copy without the drives, wrecks and refits.
@fragment
fn fs_prop(p: PropOut) -> @location(0) vec4<f32> {
    let model = models[p.blueprint];
    var in: VsOut;
    in.clip = p.clip;
    in.world = p.world;
    in.normal = p.normal;
    in.uv = p.uv;
    in.material = p.material;
    in.owner_flags = p.owner_flags;
    in.state = p.state;
    in.local = p.local;
    in.model_class = p.model_class;
    in.face = p.face;
    in.unit_id = p.unit_id;
    in.warp = vec4<f32>(0.0, 0.0, 0.0, p.warp);
    in.wreck = vec4<f32>(-1e6, 1e6, 0.0, 0.0);
    in.drive = vec4<f32>(-1.0, 0.0, 0.0, 0.0);
    in.drive_at = vec4<f32>(0.0);
    in.crackle = vec2<f32>(0.0);
    in.refit = vec3<f32>(0.0);
    in.skin = model.skin;
    in.marks = model.marks;
    in.dust = select(0.62, model.surface.y / max(model.height, 0.1), model.surface.y > 0.0);
    let body = max(select(model.bounds_radius, model.surface.x, model.surface.x > 0.0), 1.0);
    in.weld = vec4<f32>(0.0, 0.0, body, max(model.height, 1.0));
    return shade_entity(in);
}
