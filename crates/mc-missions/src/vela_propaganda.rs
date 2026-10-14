//! Optional destruction colour: no objectives, counters, rewards or campaign branches.
use super::*;
pub(super) fn events(s: &Script) {
    for (name, gate, lines) in [
        ("reach_welcome", None, &[1, 2][..]),
        ("reach_harrow", None, &[3][..]),
        ("reach_safe", None, &[4, 5][..]),
        ("reach_property", Some("arc_arrival"), &[6, 7][..]),
        ("reach_future", Some("arc_arrival"), &[8][..]),
        ("rebel_free", None, &[9][..]),
        ("rebel_speaker", None, &[10][..]),
        ("rebel_cleanup", None, &[11, 12][..]),
        ("rebel_mural", Some("betrayal"), &[13][..]),
        ("rebel_ore", None, &[14, 15][..]),
        ("reach_pdf", Some("arc_arrival"), &[16, 17][..]),
        ("rebel_home", Some("betrayal"), &[18, 19][..]),
        ("rebel_harbour", None, &[20, 21][..]),
    ] {
        let mut conditions = vec![C::PropDead(name.into()), C::DialogueIdle];
        if let Some(event) = gate {
            conditions.push(after(event, 0));
        }
        s.event(
            &format!("prop.{name}"),
            C::All(conditions),
            lines
                .iter()
                .map(|n| say(&format!("m1.prop.{n:02}")))
                .collect(),
        );
    }
}
