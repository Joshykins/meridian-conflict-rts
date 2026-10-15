//! A network match's moments staged for a match screenshot (`--net-shot STATE`).

/// A network match's link and news for a screenshot (`--net-shot STATE`): `play`
/// (link readout, lagging and dropped seats, chat, a player setting 2x), `chat`
/// (the chat line open), `watching` (this machine's side is out: it watches, its chat
/// line open among the observers), `paused`, `waiting` (loading), `rejoin` (this
/// machine reconnecting), `desync`.
pub(super) fn net_shot(
    state: &str,
    view: &crate::game::View,
) -> Result<(crate::netplay::NetLink, Vec<crate::netplay::NetNotice>), String> {
    use crate::netplay::{DesyncReport, NetLink, NetNotice, Rejoining};
    use mc_net::{Link, PeerStat};
    let seats = view.status.players.len() as u8;
    let stats = (0..seats)
        .map(|slot| PeerStat {
            slot: mc_core::PlayerId(slot),
            rtt_ms: 38 + 27 * slot as u16,
            link: match (state, slot) {
                ("play", 2) => Link::Lagging,
                ("play", 3) => Link::Dropped,
                ("waiting", 1) | ("waiting", 3) => Link::Loading,
                _ => Link::Connected,
            },
        })
        .collect();
    let said = |from: u8, text: &str, private: bool, watchers: bool| NetNotice::Chat {
        from: Some(from),
        name: view
            .status
            .players
            .get(from as usize)
            .map_or_else(String::new, |p| p.name.clone()),
        private,
        watchers,
        text: text.into(),
    };
    let chat = |from: u8, text: &str, private: bool| said(from, text, private, false);
    let mut link = NetLink {
        stats,
        input_delay: 2,
        ..NetLink::default()
    };
    let mut notices = Vec::new();
    match state {
        "play" | "chat" => {
            notices.push(chat(1, "gl hf", false));
            notices.push(chat(0, "moving the tanks up the east road", true));
            notices.push(chat(1, "bring it", false));
            if state == "play" {
                notices.push(NetNotice::Dropped(3.min(seats.saturating_sub(1))));
                link.speed = Some(200);
                notices.push(NetNotice::Speed {
                    by: Some(1.min(seats.saturating_sub(1))),
                    speed: 200,
                });
            }
        }
        "watching" => {
            let me = view.out.unwrap_or(0);
            let other = (me + 1) % seats.max(1);
            notices.push(chat(other, "push the north gate", false));
            notices.push(said(me, "gg, that nuke came out of nowhere", false, true));
            notices.push(NetNotice::Chat {
                from: None,
                name: "Spectre".into(),
                private: false,
                watchers: true,
                text: "watch the subs off the east coast".into(),
            });
        }
        "paused" => {
            link.paused_by = Some(Some(1.min(seats.saturating_sub(1))));
            notices.push(NetNotice::Paused(Some(1.min(seats.saturating_sub(1)))));
        }
        "waiting" => link.loading = Some(0b0101),
        "rejoin" => {
            link.rejoining = Some(Rejoining {
                attempts: 3,
                since: std::time::Instant::now() - std::time::Duration::from_secs(12),
            })
        }
        "desync" => {
            let good = 0x6d1f_42a9_0e33_c871;
            link.desync = Some(DesyncReport {
                tick: 4019,
                hashes: (0..seats)
                    .map(|s| (s, if s == 1 { good ^ 0x5a5a } else { good }))
                    .collect(),
                local: Some(0),
                ours: Some([1; mc_sim::state_hash::SECTION_COUNT]),
                theirs: vec![(1, {
                    let mut v = vec![1u64; mc_sim::state_hash::SECTION_COUNT];
                    v[2] = 2;
                    v[12] = 3;
                    v
                })],
                dump: crate::settings::config_dir()
                    .map(|d| d.join("desync").join("desync-1790454313-t4019-p0.mcsnap")),
            });
        }
        other => {
            return Err(format!(
            "--net-shot takes play, chat, watching, paused, waiting, rejoin or desync, not {other}"
        ))
        }
    }
    Ok((link, notices))
}
