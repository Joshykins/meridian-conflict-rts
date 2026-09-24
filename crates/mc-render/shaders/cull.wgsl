// GPU-driven visibility: interpolate, frustum-cull and pick a level of detail
// for every entity, then build compact per-mesh instance lists and the
// indirect draw commands that consume them. The CPU never sees an entity.
//
//   cs_clear   -> zero the per-slot counters
//   cs_cull    -> vis[i] = draw slot (or NOT_VISIBLE); counters[slot] += 1
//   cs_prefix  -> commands[slot] = {mesh, count, first}; counters[slot] = first
//   cs_scatter -> visible[counters[slot]++] = i
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

var<immediate> push: CullPush;

@compute @workgroup_size(64)
fn cs_clear(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x < globals.counts.z {
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
    // An aircraft stored below an airbase (`mirror::UNIT_STORED`) is listed, not drawn.
    if dynamic && (e._pad3a & 0x800u) != 0u {
        return NOT_VISIBLE;
    }
    let model = models[e.blueprint];
    let t = globals.sun.w;
    var scale = 1.0;
    if (e.owner_flags & KIND_PROP) != 0u && e.scale != 0u {
        scale = f32(e.scale) * 0.001;
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
    } else if px < 1.2 {
        return NOT_VISIBLE;
    }
    let detail_bias = select(1.0, 0.55, (model.icon & 0x200000u) != 0u);
    if px > globals.lod.z * detail_bias {
        return model.slot;
    }
    if px > globals.lod.w {
        return model.slot + 1u;
    }
    return model.slot + 2u;
}

@compute @workgroup_size(64)
fn cs_cull(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i >= push.count {
        return;
    }
    var slot: u32;
    var out_index: u32;
    var flags: u32;
    if push.dynamic == 1u {
        slot = classify(dynamic_entities[i], i, true);
        out_index = globals.counts.y + i;
        flags = dynamic_entities[i].owner_flags;
    } else {
        slot = classify(static_entities[i], i, false);
        out_index = i;
        flags = static_entities[i].owner_flags;
    }
    vis[out_index] = slot;
    if slot != NOT_VISIBLE && slot != globals.counts.z - 1u {
        atomicAdd(&counters[slot], 1u);
    }
}

@compute @workgroup_size(1)
fn cs_prefix() {
    var first = 0u;
    let n = globals.counts.z;
    for (var s = 0u; s < n; s++) {
        var count = atomicLoad(&counters[s]);
        if s == n - 1u {
            // The icon slot: a fixed row per dynamic entity (see the top).
            count = globals.counts.x;
        }
        commands[s].index_count = slots[s].index_count;
        commands[s].instance_count = count;
        commands[s].first_index = slots[s].first_index;
        commands[s].vertex_offset = slots[s].vertex_offset;
        commands[s].first_instance = first;
        atomicStore(&counters[s], first);
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
    let slot = vis[vis_index];
    let icon_slot = globals.counts.z - 1u;
    if slot != NOT_VISIBLE && slot != icon_slot {
        let at = atomicAdd(&counters[slot], 1u);
        visible[at] = entity_index;
    }
    if push.dynamic == 1u {
        let shows = slot == icon_slot || icon_too(flags, slot);
        visible[atomicLoad(&counters[icon_slot]) + i] = select(NOT_VISIBLE, entity_index, shows);
    }
}
