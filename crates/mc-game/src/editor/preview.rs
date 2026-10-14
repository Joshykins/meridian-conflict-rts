use super::*;
pub(super) fn build(
    doc: &Document,
    content_id: u64,
    blueprints: Arc<Blueprints>,
    pool: Arc<Pool>,
) -> Result<RenderFrame, String> {
    let info = &doc.map.info;
    let (w, h) = info.size_cells();
    let mut terrain = mc_map::Heightfield::from_samples(
        w,
        h,
        doc.map.samples().to_vec(),
        info.min_z,
        info.z_step,
        info.water_level,
    );
    terrain = terrain.with_decks(doc.map.decks.clone());
    let data = mc_sim::world::MapData {
        doodads: doc
            .placed
            .doodads
            .iter()
            .cloned()
            .map(|d| (d.prop_index, d))
            .collect(),
        name: info.name.clone(),
        content_id,
        ore: doc.map.ore.clone(),
        starts: doc.map.starts.clone(),
        props: doc.map.props.clone(),
    };
    let mut world = World::with_terrain(
        terrain,
        data,
        blueprints.clone(),
        pool,
        &config(&doc.placed, &doc.config),
    )
    .map_err(|e| e.to_string())?;
    world
        .spawn_placed_units(&doc.placed)
        .map_err(|e| e.to_string())?;
    let mut frame = RenderFrame::default();
    world.write_render_frame(None, &mut frame);
    frame.terrain_edits.clear();
    Ok(frame)
}
