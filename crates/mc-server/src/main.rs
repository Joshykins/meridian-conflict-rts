//! `meridian-server`: the public game server. See `docs/SERVER.md`.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::mpsc;

use mc_server::ServerConfig;

const USAGE: &str = "usage: meridian-server [options]
  --bind ADDR:PORT          where to listen (default 0.0.0.0:7777)
  --data-dir DIR            names.json and replays/ live here (default ./meridian-data)
  --max-rooms N             games open at once, 1-512 (default 64)
  --rooms-per-player N      games one player may have open at once (default 2)
  --max-per-ip N            connections from one address at once (default 16)
  --max-connections N       connections at once, all told (default 1024)
  --motd TEXT               a message shown to every player on sign-in
  --motd-file PATH          the same, read from a file
  --replay-days N           delete replays older than N days, at start and daily;
                            0 keeps them all (default 14)
  -h, --help                this text

Logs go to standard error. RUST_LOG sets the level (default info), e.g. RUST_LOG=debug.
SIGINT or SIGTERM stops the server cleanly: running games end and their replays are kept.";

fn parse() -> Result<ServerConfig, String> {
    let mut config = ServerConfig::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--bind" => config.bind = value()?,
            "--data-dir" => config.data_dir = PathBuf::from(value()?),
            "--max-rooms" => config.max_rooms = number(&arg, &value()?)?,
            "--rooms-per-player" => config.rooms_per_player = number(&arg, &value()?)?,
            "--max-per-ip" => config.max_per_ip = number(&arg, &value()?)?,
            "--max-connections" => config.max_connections = number(&arg, &value()?)?,
            "--motd" => config.motd = value()?,
            "--motd-file" => {
                let path = value()?;
                config.motd = std::fs::read_to_string(&path)
                    .map_err(|e| format!("--motd-file {path}: {e}"))?
                    .trim_end()
                    .to_owned();
            }
            "--replay-days" => config.replay_days = number(&arg, &value()?)?,
            "-h" | "--help" => return Err(String::new()),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(config)
}

fn number<T: std::str::FromStr>(flag: &str, text: &str) -> Result<T, String>
where
    T::Err: std::fmt::Display,
{
    text.parse().map_err(|e| format!("{flag} {text}: {e}"))
}

fn main() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let config = match parse() {
        Ok(config) => config,
        Err(msg) if msg.is_empty() => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(msg) => {
            eprintln!("meridian-server: {msg}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let (stop_tx, stop_rx) = mpsc::channel();
    if let Err(e) = ctrlc::set_handler(move || {
        let _ = stop_tx.send(());
    }) {
        log::error!("cannot catch SIGINT and SIGTERM: {e}");
        return ExitCode::FAILURE;
    }
    log::info!(
        "meridian-server {}, data in {}",
        env!("CARGO_PKG_VERSION"),
        config.data_dir.display()
    );
    let server = match mc_server::start(config) {
        Ok(server) => server,
        Err(e) => {
            log::error!("cannot start: {e}");
            return ExitCode::FAILURE;
        }
    };
    // Blocks until a signal; a second one while shutting down is ignored.
    let _ = stop_rx.recv();
    server.shutdown();
    ExitCode::SUCCESS
}
