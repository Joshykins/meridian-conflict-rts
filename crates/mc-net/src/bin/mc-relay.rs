//! Dedicated relay: hosts one match, then exits.
//!
//! `mc-relay --bind 0.0.0.0:7777 --players 2 [--replay-dir DIR] [--input-delay TICKS] [--auto-start]`

use std::process::ExitCode;

use mc_net::{RelayConfig, RelayServer};

const USAGE: &str = "usage: mc-relay [--bind ADDR:PORT] [--players 1-8] [--replay-dir DIR] [--input-delay TICKS] [--auto-start]
  --bind         listen address (default 0.0.0.0:7777)
  --players      player slots (default 8)
  --replay-dir   write the match as a .mcreplay file into DIR
  --input-delay  ticks between issuing and executing a command (default 2)
  --auto-start   start when every slot is filled and ready instead of waiting for the host";

fn parse() -> Result<(String, RelayConfig), String> {
    let mut bind = "0.0.0.0:7777".to_owned();
    let mut config = RelayConfig::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--bind" => bind = value()?,
            "--players" => {
                config.players = value()?.parse().map_err(|e| format!("--players: {e}"))?
            }
            "--replay-dir" => config.replay_dir = Some(value()?.into()),
            "--input-delay" => {
                config.input_delay = value()?
                    .parse()
                    .map_err(|e| format!("--input-delay: {e}"))?
            }
            "--auto-start" => config.auto_start = true,
            "-h" | "--help" => return Err(String::new()),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok((bind, config))
}

fn main() -> ExitCode {
    let (bind, config) = match parse() {
        Ok(parsed) => parsed,
        Err(msg) => {
            if !msg.is_empty() {
                eprintln!("mc-relay: {msg}");
            }
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    let players = config.players;
    let result = RelayServer::bind(&bind, config).and_then(|server| {
        eprintln!(
            "mc-relay: listening on {} for {players} players",
            server.local_addr()?
        );
        server.run()
    });
    match result {
        Ok(summary) => {
            eprintln!("mc-relay: match over after {} ticks", summary.ticks);
            if let Some(path) = &summary.replay_path {
                eprintln!("mc-relay: replay at {}", path.display());
            }
            if let Some(e) = &summary.replay_error {
                eprintln!("mc-relay: replay is incomplete: {e}");
            }
            if let Some(tick) = summary.desync_tick {
                eprintln!("mc-relay: players desynced at tick {tick}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("mc-relay: {e}");
            ExitCode::FAILURE
        }
    }
}
