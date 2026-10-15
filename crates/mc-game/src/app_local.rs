use super::*;
pub(crate) fn local_start(
    request: MatchRequest,
    blueprints: &Blueprints,
    record: bool,
) -> Result<GameStart, String> {
    let mission = crate::campaign::definition(&request.map)?;
    local_mission_start(request, blueprints, record, mission)
}

pub(crate) fn local_mission_start(
    request: MatchRequest,
    blueprints: &Blueprints,
    record: bool,
    mission: Option<mc_sim::mission::Definition>,
) -> Result<GameStart, String> {
    local_checkpoint_start(request, blueprints, record, mission, None)
}

pub(crate) fn local_checkpoint_start(
    request: MatchRequest,
    blueprints: &Blueprints,
    record: bool,
    mission: Option<mc_sim::mission::Definition>,
    checkpoint: Option<crate::campaign_checkpoints::Checkpoint>,
) -> Result<GameStart, String> {
    start_local(request, blueprints, record, mission, checkpoint, None)
}

pub(crate) fn local_editor_start(
    request: MatchRequest,
    blueprints: &Blueprints,
    mission: Option<mc_sim::mission::Definition>,
    slot: u8,
) -> Result<GameStart, String> {
    start_local(request, blueprints, false, mission, None, Some(slot))
}

fn start_local(
    request: MatchRequest,
    blueprints: &Blueprints,
    record: bool,
    mission: Option<mc_sim::mission::Definition>,
    checkpoint: Option<crate::campaign_checkpoints::Checkpoint>,
    editor_slot: Option<u8>,
) -> Result<GameStart, String> {
    let MatchRequest {
        map,
        mut config,
        colors,
        survival,
        sky,
        fallen_army,
    } = request;
    if mission.is_some() && editor_slot.is_none() {
        config = crate::campaign::config();
    }
    let authored = mc_data::weather::MapConfig::for_map(map.path())?;
    if !authored.players.is_empty() {
        config.players = crate::map_players::roster(&authored.players);
    }
    if let Some(slot) = editor_slot {
        let selected = config
            .players
            .get_mut(usize::from(slot))
            .ok_or("Unknown test player slot")?;
        selected.controller = mc_sim::tables::Controller::Human;
    }
    let content = mc_net::ContentId {
        map_id: map.content_id(),
        blueprint_hash: blueprints.content_hash(),
    };
    let human = editor_slot.map(usize::from).or_else(|| {
        config
            .players
            .iter()
            .position(|p| p.controller == mc_sim::tables::Controller::Human)
    });
    let observing = human.is_none();
    let local = human.unwrap_or(0);
    let humans = if observing {
        Vec::new()
    } else {
        vec![mc_net::PlayerSetup {
            slot: mc_core::PlayerId(local as u8),
            name: config.players[local].name.clone(),
            data: Vec::new(),
        }]
    };
    let start = mc_net::MatchStart {
        content,
        seed: config.seed,
        input_delay: 1,
        players: humans,
        options: crate::match_options::MatchOptions {
            config: config.clone(),
            survival: survival.clone(),
            mission,
            colors,
            map: map.name().to_owned(),
            map_id: map.content_id(),
            sky,
            fallen_army,
        }
        .encode()?,
    };
    let content = start.content;
    let mut session = if let Some(saved) = checkpoint {
        mc_net::LocalSession::resume(
            start,
            mc_core::PlayerId(local as u8),
            mc_net::session::Pacing::RealTime,
            saved.next_tick,
            saved.blob,
        )
    } else if observing {
        mc_net::LocalSession::observer(start, mc_net::session::Pacing::RealTime)
    } else {
        mc_net::LocalSession::new(
            start,
            mc_core::PlayerId(local as u8),
            mc_net::session::Pacing::RealTime,
        )
    }
    .map_err(|e| e.to_string())?;
    let record = if record {
        let recorded = crate::issues::MatchRecord::new().and_then(|m| {
            let origin = crate::build_info::origin(content);
            session.record_to(&m.replay, &origin).map(|()| m)
        });
        match recorded {
            Ok(m) => {
                log::info!("recording match {} to {}", m.id, m.replay.display());
                Some(m)
            }
            Err(e) => {
                log::warn!("this match will not be recorded: {e}");
                None
            }
        }
    } else {
        None
    };
    let start_index = config
        .players
        .get(local)
        .map(|p| p.start as usize)
        .unwrap_or(0);
    Ok(GameStart {
        map,
        colors,
        sky,
        session: Box::new(session),
        prefetched: Vec::new(),
        local: local as u8,
        start_index,
        roster: config.players.clone(),
        observing,
        scene: None,
        range: None,
        record,
        recorder: None,
        seek: None,
        net: None,
        keep: Vec::new(),
    })
}

/// Records `start` from the sim thread (`recorder.rs`): for a match whose session
/// does not record itself, a network match or the test range.
pub(crate) fn record_in_sim(start: &mut GameStart) {
    match crate::issues::MatchRecord::new() {
        Ok(m) => {
            start.recorder = Some(crate::recorder::Recorder::new(m.replay.clone()));
            start.record = Some(m);
        }
        Err(e) => log::warn!("this match will not be recorded: {e}"),
    }
}
