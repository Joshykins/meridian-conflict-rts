use super::*;
impl Game {
    pub(crate) fn editor_test_state(&self) -> serde_json::Value {
        serde_json::json!({
            "tick": self.tick(),
            "units": self.view.frame.units.len(),
            "beams": self.view.frame.beams.len(),
            "dead_props": self.view.frame.props_dead.iter().map(|v| v.count_ones()).sum::<u32>(),
            "reclaimed_props": self.view.frame.city.iter().filter(|s| s.flags & mc_sim::city::RECLAIMED != 0).count(),
            "players": self.view.status.players.iter().map(|p| serde_json::json!({"name":p.name,"materials":p.materials})).collect::<Vec<_>>(),
        })
    }
    pub(crate) fn editor_camera(&mut self, camera: Camera) {
        self.camera = camera;
    }
}
