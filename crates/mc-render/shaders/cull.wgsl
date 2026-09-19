// GPU-driven visibility: interpolate, frustum-cull and pick a level of detail
// for every entity, then build compact per-mesh instance lists and the
// indirect draw commands that consume them. The CPU never sees an entity.
//
//   cs_clear   -> zero the per-slot counters
//   cs_cull    -> vis[i] = draw slot (or NOT_VISIBLE); counters[slot] += 1
//   cs_prefix  -> commands[slot] = {mesh, count, first}; counters[slot] = first
//   cs_scatter -> visible[counters[slot]++] = i

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

fn classify(e: Entity, index: u32, dynamic: bool) -> u32 {
    let flags = e.owner_flags;
    if !dynamic && (props_dead[index >> 5u] & (1u << (index & 31u))) != 0u {
        return NOT_VISIBLE;
    }
    // An upgrade assembles hidden inside the structure it replaces.
    if (flags & FLAG_UPGRADE) != 0u && (flags & FLAG_IN_FACTORY) != 0u {
        return NOT_VISIBLE;
    }
    let model = models[e.blueprint];
    let t = globals.sun.w;
    var scale = 1.0;
    if e.scale != 0u {
        scale = f32(e.scale) * 0.001;
    }
    let radius = model.bounds_radius * scale;
    let center = mix(e.prev_pos, e.pos, t) + vec3<f32>(0.0, 0.0, model.height * scale * 0.5);
    for (var i = 0; i < 5; i++) {
        let plane = globals.frustum[i];
        if dot(plane.xyz, center) + plane.w < -radius {
            return NOT_VISIBLE;
        }
    }
    let dist = max(distance(center, globals.camera.xyz), 1.0);
    let px = radius * globals.lod.x / dist;
    let is_unit = (flags & (KIND_WRECK | KIND_PROP)) == 0u;
    if (flags & KIND_GHOST) != 0u {
        return model.slot;
    }
    if is_unit {
        if px < globals.lod.y {
            // Strategic icon: the last slot.
            return globals.counts.z - 1u;
        }
    } else if px < 1.2 {
        return NOT_VISIBLE;
    }
    if px > globals.lod.z {
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
    if push.dynamic == 1u {
        slot = classify(dynamic_entities[i], i, true);
        out_index = globals.counts.y + i;
    } else {
        slot = classify(static_entities[i], i, false);
        out_index = i;
    }
    vis[out_index] = slot;
    if slot != NOT_VISIBLE {
        atomicAdd(&counters[slot], 1u);
    }
}

@compute @workgroup_size(1)
fn cs_prefix() {
    var first = 0u;
    let n = globals.counts.z;
    for (var s = 0u; s < n; s++) {
        let count = atomicLoad(&counters[s]);
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
    if push.dynamic == 1u {
        vis_index = globals.counts.y + i;
        entity_index = i | DYNAMIC_BIT;
    }
    let slot = vis[vis_index];
    if slot != NOT_VISIBLE {
        let at = atomicAdd(&counters[slot], 1u);
        visible[at] = entity_index;
    }
}
