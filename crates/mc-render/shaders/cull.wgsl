// GPU-driven visibility: interpolate, frustum-cull and pick a level of detail
// for every entity, then build compact per-mesh instance lists and the
// indirect draw commands that consume them. The CPU never sees an entity.
//
//   cs_clear   -> zero the per-slot counters of every list
//   cs_cull    -> vis[i] = draw slot (or NOT_VISIBLE) and the lists it is in;
//                 counters[list][slot] += 1
//   cs_prefix  -> commands[list][slot] = {mesh, count, first}; counters[list][slot] = first
//   cs_scatter -> visible[counters[list][slot]++] = i
//
// There is one list per pass that draws models (CULL_LIST_*): the colour pass's,
// the depth pre-pass's and one per shadow cascade. The pre-pass and cascade lists
// hold what the colour list holds, less what those passes have no use for, so a
// tree outside a cascade never reaches the vertex shader in that cascade. Every
// list has `counts.z` slots; the icon slot is used in the colour list only.
//
// A unit whose model is drawn is listed a second time, in the icon slot: its
// strategic icon shows at every zoom, not only once the model is too small.
//
// The icon slot is not appended to with atomics: icons draw without depth, so
// wherever they overlap the draw order decides which is on top, and an atomic
// order reshuffles every frame (crowds of icons flicker). Instead the slot holds
// one entry per dynamic entity at its own row, NOT_VISIBLE where there is no
// icon, and vs_icon collapses those.

struct DrawSlot {
    index_count: u32,
    first_index: u32,
    vertex_offset: i32,
    pad: u32,
}

struct DrawCommand {
    index_count: u32,
    instance_count: u32,
    first_index: u32,
    vertex_offset: i32,
    first_instance: u32,
}

struct CullPush {
    // First entity of this dispatch within its buffer, and how many.
    count: u32,
    // 1: the dynamic buffer (units, wrecks, ghosts); 0: static props.
    dynamic: u32,
}

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var<storage, read> dynamic_entities: array<Entity>;
@group(0) @binding(2) var<storage, read> static_entities: array<Entity>;
@group(0) @binding(3) var<storage, read> models: array<ModelInfo>;
@group(0) @binding(4) var<storage, read> slots: array<DrawSlot>;
@group(0) @binding(5) var<storage, read_write> vis: array<u32>;
@group(0) @binding(6) var<storage, read_write> counters: array<atomic<u32>>;
@group(0) @binding(7) var<storage, read_write> commands: array<DrawCommand>;
@group(0) @binding(8) var<storage, read_write> visible: array<u32>;
@group(0) @binding(9) var<storage, read> props_dead: array<u32>;
@group(0) @binding(10) var<storage, read> effect_barriers: EffectBarriers;
// Per entity (indexed as `vis`): the blasts' push on a standing tree, xy the lean of
// its top in metres, z the stir of its leaves. entity.wgsl `tree_air` adds the wind.
@group(0) @binding(11) var<storage, read_write> tree_sway: array<vec4<f32>>;

var<immediate> push: CullPush;

@compute @workgroup_size(64)
fn cs_clear(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x < globals.counts.z * CULL_LIST_COUNT {
        atomicStore(&counters[id.x], 0u);
    }
}

// Units carry a strategic icon; wrecks, props, placement ghosts and whatever is
// still inside the factory assembling it do not.
fn has_icon(flags: u32) -> bool {
    return (flags & (KIND_WRECK | KIND_PROP | KIND_GHOST | FLAG_IN_FACTORY)) == 0u;
}

fn radar_only(flags: u32) -> bool {
    return (flags & STATE_RADAR) != 0u;
}

// The icon slot as well, for an entity `classify` gave a model slot.
fn icon_too(flags: u32, slot: u32) -> bool {
    return slot != NOT_VISIBLE && slot != globals.counts.z - 1u && has_icon(flags);
}

fn classify(e: Entity, index: u32, dynamic: bool) -> u32 {
    let flags = e.owner_flags;
    if !dynamic && (props_dead[index >> 5u] & (1u << (index & 31u))) != 0u {
        return NOT_VISIBLE;
    }
    // An upgrade assembles hidden inside the structure it replaces.
    if (flags & FLAG_UPGRADE) != 0u && (flags & FLAG_IN_FACTORY) != 0u {
        return NOT_VISIBLE;
    }
    // A unit stored in a lift ship's hold (`mirror::UNIT_STORED`) is listed, not drawn.
    if dynamic && (e.status[0] & 0x800u) != 0u {
        return NOT_VISIBLE;
    }
    // A ship in warp is out of the world: listed for its side, not drawn.
    if dynamic && (e.status[0] & WARP_STATUS_IN_WARP) != 0u {
        return NOT_VISIBLE;
    }
    let model = models[e.blueprint];
    let t = globals.sun.w;
    var scale = 1.0;
    if (e.owner_flags & KIND_PROP) != 0u && e.packed != 0u {
        scale = f32(e.packed) * 0.001;
    }
    let radius = model.bounds_radius * scale;
    let center = mix(e.prev_pos, e.pos, t) + vec3<f32>(0.0, 0.0, model.height * scale * 0.5);
    // Slack is for the frustum test only. LOD still uses the true radius.
    // Props: static records carry overview height; the vertex shader stands
    // them on the streamed surface, which can sit much higher or lower.
    // Near is left to the rasterizer so a sphere that straddles the clip
    // (camera inside a factory, a boom past the near plane) still draws.
    var cull_r = radius * 1.2;
    if (flags & KIND_PROP) != 0u {
        cull_r += 64.0;
    }
    for (var i = 0; i < 4; i++) {
        let plane = globals.frustum[i];
        if dot(plane.xyz, center) + plane.w < -cull_r {
            return NOT_VISIBLE;
        }
    }
    let dist = max(distance(center, globals.camera.xyz), 1.0);
    let px = radius * globals.lod.x / dist;
    if (flags & KIND_GHOST) != 0u {
        return model.slot;
    }
    if radar_only(flags) {
        // A radar contact is a blip, never the hull sitting in the fog.
        if has_icon(flags) {
            return globals.counts.z - 1u;
        }
        return NOT_VISIBLE;
    }
    if has_icon(flags) {
        if px < globals.lod.y {
            // Too small for a model: the strategic icon alone, from the last slot.
            return globals.counts.z - 1u;
        }
    } else if px < select(1.2, globals.detail.x, (flags & KIND_PROP) != 0u) {
        return NOT_VISIBLE;
    }
    var detail_bias = select(1.0, 0.55, (model.icon & 0x200000u) != 0u);
    if (flags & KIND_PROP) != 0u {
        detail_bias *= globals.detail.y;
    }
    if px > globals.lod.z * detail_bias {
        return model.slot;
    }
    if px > globals.lod.w * select(1.0, globals.detail.y, (flags & KIND_PROP) != 0u) {
        return model.slot + 1u;
    }
    // Props by the hundred thousand, a few pixels across: their far level.
    if (flags & KIND_PROP) != 0u && px < LOD_FAR_PX {
        return model.slot + LOD_FAR;
    }
    return model.slot + 2u;
}

// A unit drawn as a model less than this many pixels in radius on screen is left
// out of the depth pre-pass.
const UNIT_PREPASS_PX: f32 = 3.0;
// A unit whose radius is under this many of a cascade's texels casts nothing into it.
const UNIT_SHADOW_TEXELS: f32 = 1.0;

// `vis` holds a model slot with, above it, the lists besides the colour pass's
// that the entity is in (bit `l` for list `l`).
const LISTS_SHIFT: u32 = 24u;

fn vis_slot(v: u32) -> u32 {
    return select(v & ((1u << LISTS_SHIFT) - 1u), v, v == NOT_VISIBLE);
}

// The lists besides the colour pass's that an entity drawn with a model goes in.
fn other_lists(e: Entity) -> u32 {
    let model = models[e.blueprint];
    let prop = (e.owner_flags & KIND_PROP) != 0u;
    var scale = 1.0;
    if prop && e.packed != 0u {
        scale = f32(e.packed) * 0.001;
    }
    let r = model.bounds_radius * scale;
    let center = mix(e.prev_pos, e.pos, globals.sun.w) + vec3<f32>(0.0, 0.0, model.height * scale * 0.5);
    let on_screen = r * globals.lod.x / max(distance(e.pos, globals.camera.xyz), 1.0);
    var lists = 0u;
    // The depth pre-pass leaves out props only a few pixels across: they cost it a
    // whole alpha-tested draw and hide almost nothing, and the colour pass writes
    // their depth itself. So with units a couple of pixels across: an army seen
    // from afar is tens of thousands of them.
    if on_screen >= select(UNIT_PREPASS_PX, 10.0, prop) {
        lists |= 1u << CULL_LIST_PREPASS;
    }
    // Props stand on the streamed surface, which can sit well off the height their
    // record carries (see `classify`).
    let slack = r * 1.2 + select(0.0, 64.0, prop);
    for (var c = 0u; c < CULL_LIST_COUNT - CULL_LIST_SHADOW; c++) {
        // Small props (trees, rocks) cast nothing into the far cascade, and nothing
        // into a nearer one where they are only a couple of its texels across. Those
        // shadows are specks on screen. Big props (the Precursor works) keep theirs.
        // None either from props too small on screen (Globals::detail.z pixels).
        if prop && ((c >= 2u && r < 30.0) || r < 2.5 * globals.shadow_info[c].x || on_screen < globals.detail.z) {
            continue;
        }
        // Nor from a unit less than a couple of the cascade's texels across: its shadow
        // there is a speck, and an army is tens of thousands of them in every cascade.
        if !prop && r < UNIT_SHADOW_TEXELS * globals.shadow_info[c].x {
            continue;
        }
        // Only what stands inside the cascade's square, seen from the sun.
        let m = globals.shadow_cascades[c];
        let q = m * vec4<f32>(center, 1.0);
        let reach = vec2<f32>(
            length(vec3<f32>(m[0].x, m[1].x, m[2].x)),
            length(vec3<f32>(m[0].y, m[1].y, m[2].y)),
        ) * slack;
        if any(abs(q.xy / q.w) > vec2<f32>(1.0) + reach) {
            continue;
        }
        lists |= 1u << (CULL_LIST_SHADOW + c);
    }
    return lists;
}

// Where the blasts on screen (renderer/tree_wind.rs) push the top of a tree `tall`
// metres high standing at `foot`, metres (xy), and how hard they shake its leaves (z).
// A blast's push runs out through the trees at its front's speed, shoves each away
// and lets it swing back and settle. A shield in between stops it. Worked out once
// per tree here, not for every vertex of it in every pass that draws it: with a
// battle's blasts and shields on screen that per-vertex loop cost a forest several
// milliseconds in each shadow cascade and the pre-pass.
fn tree_blast(foot: vec3<f32>, tall: f32) -> vec3<f32> {
    let time = globals.camera.w;
    let mid = foot + vec3<f32>(0.0, 0.0, tall * 0.5);
    let give = inverseSqrt(max(tall / 10.0, 0.4));
    let swing = 1.3 * give + 0.5;
    var lean = vec2<f32>(0.0);
    var stir = 0.0;
    for (var i = 0u; i < u32(globals.tree_wind.x); i++) {
        let b = globals.tree_blasts[i * 2u];
        let range = globals.tree_blasts[i * 2u + 1u].x;
        let d = distance(b.xyz, mid);
        if d >= range { continue; }
        let since = time - b.w - d / max(globals.tree_blasts[i * 2u + 1u].z, 1.0);
        if since <= 0.0 || since > 3.5 { continue; }
        if effect_blocked(b.xyz, mid) { continue; }
        let away = foot.xy - b.xy;
        let dir = select(vec2<f32>(1.0, 0.0), away / max(length(away), 0.001), dot(away, away) > 0.01);
        let near = 1.0 - d / range;
        let force = globals.tree_blasts[i * 2u + 1u].y * near * near * give * (tall / 10.0);
        // Knocked over by the front and swung back by its own spring: out to the
        // full push in about a quarter of a second, back through upright, a
        // smaller swing the other way, settling. (1.8 makes the first peak ~1.)
        let spring = swing * 2.4;
        let shape = 1.8 * exp(-since * 1.9) * sin(since * spring);
        lean += dir * min(force, tall * 0.35) * shape;
        stir = max(stir, min(force / tall * 4.0, 1.0) * exp(-since * 1.2));
    }
    // A barrage's pushes add up, but a tree only bends so far: the sum eases into a
    // limit instead of throwing the crown past the ground.
    let most = tall * 0.4;
    lean *= inverseSqrt(1.0 + dot(lean, lean) / (most * most));
    return vec3<f32>(lean, stir);
}

fn effect_blocked(source: vec3<f32>, to: vec3<f32>) -> bool {
    for (var i = 0u; i < effect_barriers.header.x; i++) {
        if barrier_crosses(source, to, effect_barriers.entries[i]) { return true; }
    }
    return false;
}

@compute @workgroup_size(64)
fn cs_cull(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i >= push.count {
        return;
    }
    var e: Entity;
    var out_index: u32;
    if push.dynamic == 1u {
        e = dynamic_entities[i];
        out_index = globals.counts.y + i;
    } else {
        e = static_entities[i];
        out_index = i;
    }
    let slot = classify(e, i, push.dynamic == 1u);
    if slot == NOT_VISIBLE || slot == globals.counts.z - 1u {
        vis[out_index] = slot;
        return;
    }
    let lists = other_lists(e);
    vis[out_index] = slot | (lists << LISTS_SHIFT);
    // A standing prop (a trampled tree is past caring): what the blasts do to it.
    if (e.owner_flags & KIND_PROP) != 0u && e.arm_pitch.x == 0.0 {
        let scale = select(1.0, f32(e.packed) * 0.001, e.packed != 0u);
        let stretch = select(1.0, e.arm_pitch.w, e.arm_pitch.w > 0.0);
        let tall = max(models[e.blueprint].height * scale * stretch, 1.0);
        tree_sway[out_index] = vec4<f32>(tree_blast(e.pos, tall), 0.0);
    }
    atomicAdd(&counters[slot], 1u);
    for (var l = 1u; l < CULL_LIST_COUNT; l++) {
        if (lists & (1u << l)) != 0u {
            atomicAdd(&counters[l * globals.counts.z + slot], 1u);
        }
    }
}

// One thread per list, each over its own stretch of `visible`: the colour list
// first (every entity, and a row per unit for icons), then the others, each
// room for every entity.
@compute @workgroup_size(CULL_LIST_COUNT)
fn cs_prefix(@builtin(local_invocation_index) list: u32) {
    let entities = globals.counts.x + globals.counts.y;
    var first = 0u;
    if list > 0u {
        first = entities + globals.counts.x + (list - 1u) * entities;
    }
    let n = globals.counts.z;
    let base = list * n;
    for (var s = 0u; s < n; s++) {
        var count = atomicLoad(&counters[base + s]);
        if s == n - 1u {
            // The icon slot: a fixed row per dynamic entity (see the top).
            count = select(0u, globals.counts.x, list == CULL_LIST_MAIN);
        }
        commands[base + s].index_count = slots[s].index_count;
        commands[base + s].instance_count = count;
        commands[base + s].first_index = slots[s].first_index;
        commands[base + s].vertex_offset = slots[s].vertex_offset;
        commands[base + s].first_instance = first;
        atomicStore(&counters[base + s], first);
        first += count;
    }
}

@compute @workgroup_size(64)
fn cs_scatter(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i >= push.count {
        return;
    }
    var vis_index = i;
    var entity_index = i;
    var flags: u32;
    if push.dynamic == 1u {
        vis_index = globals.counts.y + i;
        entity_index = i | DYNAMIC_BIT;
        flags = dynamic_entities[i].owner_flags;
    } else {
        flags = static_entities[i].owner_flags;
    }
    let v = vis[vis_index];
    let slot = vis_slot(v);
    let icon_slot = globals.counts.z - 1u;
    if slot != NOT_VISIBLE && slot != icon_slot {
        let at = atomicAdd(&counters[slot], 1u);
        visible[at] = entity_index;
        let lists = v >> LISTS_SHIFT;
        for (var l = 1u; l < CULL_LIST_COUNT; l++) {
            if (lists & (1u << l)) != 0u {
                visible[atomicAdd(&counters[l * globals.counts.z + slot], 1u)] = entity_index;
            }
        }
    }
    if push.dynamic == 1u {
        let shows = slot == icon_slot || icon_too(flags, slot);
        visible[atomicLoad(&counters[icon_slot]) + i] = select(NOT_VISIBLE, entity_index, shows);
    }
}
