//! Timing for every sim phase and GPU pass, as the spec asks, plus table sizes.

use super::Scene;
use crate::ui::{ink, palette, rgb, type_scale, Rect, Ui};
use glam::Vec2;

/// Draws the panel with its top-right corner at `corner`; returns what it covered.
pub fn draw(ui: &mut Ui, s: &Scene, corner: Vec2) -> Rect {
    let st = &s.view.status;
    let gpu = s.gpu;
    let ms = |ns: u64| format!("{:.2} ms", ns as f32 / 1e6);
    let mut rows: Vec<(String, String, u32)> = Vec::new();
    rows.push((
        "Frame".into(),
        format!("{:.0} FPS  \u{b7}  CPU {:.2} ms", s.view.fps, s.view.cpu_ms),
        palette::TEXT,
    ));
    let gpu_total: f32 = gpu.gpu_passes.iter().map(|p| p.1).sum();
    rows.push((
        "GPU Frame".into(),
        format!("{gpu_total:.2} ms"),
        palette::TEXT,
    ));
    for (name, t) in &gpu.gpu_passes {
        rows.push((
            format!("   {}", name),
            format!("{t:.2} ms"),
            palette::DIM,
        ));
    }
    let over = st.tick_ns > 25_000_000;
    rows.push((
        "Sim Tick".into(),
        format!("{}  \u{b7}  Worst {}", ms(st.tick_ns), ms(st.worst_tick_ns)),
        if over { palette::BAD } else { palette::TEXT },
    ));
    for (name, ns) in &st.phases {
        rows.push((format!("   {}", name), ms(*ns), palette::DIM));
    }
    rows.push((
        "Tick".into(),
        format!("{}  \u{b7}  {:08X}", st.tick, st.hash as u32),
        palette::TEXT,
    ));
    rows.push((
        "Units / Shots".into(),
        format!("{} / {}", st.units, st.projectiles),
        palette::DIM,
    ));
    rows.push((
        "Wrecks / Stains".into(),
        format!("{} / {}", st.wrecks, st.stains),
        palette::DIM,
    ));
    rows.push((
        "Orders / Fields / Late".into(),
        format!("{} / {} / {}", st.orders, st.flow_fields, st.late_paths),
        palette::DIM,
    ));
    rows.push((
        "Drawn".into(),
        format!("{} + {} Props", gpu.dynamic_entities, gpu.static_entities),
        palette::DIM,
    ));
    rows.push((
        "Terrain Nodes".into(),
        gpu.terrain_nodes.to_string(),
        palette::DIM,
    ));

    let (w, row_h) = (300.0, 15.0);
    let r = Rect::new(corner.x - w, corner.y, w, rows.len() as f32 * row_h + 34.0);
    ui.fill(r, ink(0.7));
    ui.frame(r, rgb(palette::LINE, 0.12));
    ui.section(r.x + 12.0, r.y + 14.0, w - 24.0, "Performance");
    let mut y = r.y + 34.0;
    for (label, value, tone) in &rows {
        ui.text(
            r.x + 12.0,
            y,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            label,
        );
        ui.text_right(
            r.right() - 12.0,
            y,
            type_scale::MICRO,
            rgb(*tone, 1.0),
            value,
        );
        y += row_h;
    }
    r
}
