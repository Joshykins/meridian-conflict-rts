//! The publishing side of the build store (docs/RELEASES.md). scripts/release.sh
//! drives it; by hand:
//!
//!   mc-release keygen KEYFILE PUBFILE      a new release key (once, ever)
//!   mc-release pack --stage DIR --store DIR --key KEYFILE --version-of EXE [--notes FILE] [--newest]
//!   mc-release news --store DIR --key KEYFILE NEWS.json   checks the news and signs it into the store
//!   mc-release unlist --store DIR --key KEYFILE --channel CHANNEL BUILD
//!                                           takes BUILD out of the channel's history (one
//!                                           listed by mistake) and signs the history again
//!   mc-release origin REPLAY                who recorded a replay, one `key: value` a line

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use mc_builds::pack::{self, Identity};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("mc-release: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("keygen") => {
            let [_, key, public] = args else {
                return Err("usage: mc-release keygen KEYFILE PUBFILE".into());
            };
            let hex = pack::keygen(Path::new(key)).map_err(|e| e.to_string())?;
            std::fs::write(public, format!("{hex}\n")).map_err(|e| e.to_string())?;
            println!("release key written to {key}: back it up; the public half is in {public}");
            Ok(())
        }
        Some("pack") => pack_cmd(&args[1..]),
        Some("news") => news_cmd(&args[1..]),
        Some("unlist") => unlist_cmd(&args[1..]),
        Some("origin") => {
            let [_, file] = args else {
                return Err("usage: mc-release origin REPLAY".into());
            };
            let p = mc_net::Origin::peek_file(file).map_err(|e| e.to_string())?;
            let o = p.origin;
            print!(
                "format: {}\nbuild: {}\ncommit: {}\nchannel: {}\nsim: {:016x}\nmap: {:016x}\nblueprints: {:016x}\n",
                p.format,
                o.build,
                o.commit,
                o.channel.map_or("", |c| c.name()),
                o.sim,
                o.content.map_id,
                o.content.blueprint_hash
            );
            Ok(())
        }
        _ => {
            Err("usage: mc-release keygen | pack | news | origin (see the source's header)".into())
        }
    }
}

fn pack_cmd(args: &[String]) -> Result<(), String> {
    let (mut stage, mut store, mut key, mut version_of, mut notes, mut newest) =
        (None, None, None, None, String::new(), false);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().cloned().ok_or(format!("{a} takes a value"));
        match a.as_str() {
            "--stage" => stage = Some(PathBuf::from(value()?)),
            "--store" => store = Some(PathBuf::from(value()?)),
            "--key" => key = Some(PathBuf::from(value()?)),
            "--version-of" => version_of = Some(value()?),
            "--notes" => {
                let file = value()?;
                notes = std::fs::read_to_string(&file)
                    .map_err(|e| format!("{file}: {e}"))?
                    .trim()
                    .to_owned();
            }
            "--newest" => newest = true,
            other => return Err(format!("unknown option {other}")),
        }
    }
    let (Some(stage), Some(store), Some(key), Some(version_of)) = (stage, store, key, version_of)
    else {
        return Err("pack needs --stage, --store, --key and --version-of".into());
    };
    let key = pack::load_key(&key).map_err(|e| e.to_string())?;
    let id = identity(&version_of)?;
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let packed = pack::pack(&stage, &store, id, notes, created).map_err(|e| e.to_string())?;
    pack::publish(&store, &packed.manifest, &key, newest).map_err(|e| e.to_string())?;
    let total: u64 = packed.manifest.files.iter().map(|e| e.packed).sum();
    println!(
        "{} ({} files, {:.1} MB packed): {} new blobs, {:.1} MB new",
        packed.manifest.build,
        packed.manifest.files.len(),
        total as f64 / 1e6,
        packed.new_blobs,
        packed.new_bytes as f64 / 1e6
    );
    Ok(())
}

fn news_cmd(args: &[String]) -> Result<(), String> {
    let [flag_store, store, flag_key, key, file] = args else {
        return Err("usage: mc-release news --store DIR --key KEYFILE NEWS.json".into());
    };
    if flag_store != "--store" || flag_key != "--key" {
        return Err("usage: mc-release news --store DIR --key KEYFILE NEWS.json".into());
    }
    let key = pack::load_key(Path::new(key)).map_err(|e| e.to_string())?;
    let bytes = std::fs::read(file).map_err(|e| format!("{file}: {e}"))?;
    let news = mc_builds::feed::News::parse(&bytes).map_err(|e| format!("{file}: {e}"))?;
    mc_builds::feed::sign_into(Path::new(store), mc_builds::feed::News::PATH, &news, &key)
        .map_err(|e| e.to_string())?;
    println!(
        "news: {} items, {} links signed into {store}",
        news.items.len(),
        news.links.len()
    );
    Ok(())
}

fn unlist_cmd(args: &[String]) -> Result<(), String> {
    let usage = "usage: mc-release unlist --store DIR --key KEYFILE --channel CHANNEL BUILD";
    let [f_store, store, f_key, key, f_channel, channel, build] = args else {
        return Err(usage.into());
    };
    if f_store != "--store" || f_key != "--key" || f_channel != "--channel" {
        return Err(usage.into());
    }
    let key = pack::load_key(Path::new(key)).map_err(|e| e.to_string())?;
    let channel: mc_core::Channel = channel.parse().map_err(|e| format!("{e}"))?;
    // The store holds Windows builds: scripts/release.sh publishes no other.
    let at = mc_builds::feed::History::path(channel, "windows-x64");
    let file = Path::new(store).join(&at);
    let bytes = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    let history = mc_builds::feed::History::parse(&bytes).map_err(|e| e.to_string())?;
    if !history.builds.iter().any(|r| &r.build == build) {
        return Err(format!("{build} is not in the {channel} history"));
    }
    let history = history.without(build);
    mc_builds::feed::sign_into(Path::new(store), &at, &history, &key).map_err(|e| e.to_string())?;
    println!(
        "{build} taken out of {at}; it lists {} builds now",
        history.builds.len()
    );
    Ok(())
}

/// A build's identity from `meridian --version` (`key: value` lines, saved to
/// a file by scripts/release.sh, since the executable may be another system's).
fn identity(file: &str) -> Result<Identity, String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
    let field = |name: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(name)?.strip_prefix(": "))
            .map(|v| v.trim().to_owned())
            .ok_or(format!("{file} has no {name}"))
    };
    let platform = field("platform").unwrap_or_else(|_| mc_builds::platform().to_owned());
    Ok(Identity {
        build: field("build")?,
        number: field("number")?
            .parse()
            .map_err(|_| "number is not a number")?,
        channel: field("channel")?.parse().map_err(|e| format!("{e}"))?,
        commit: field("commit")?,
        sim: field("sim")?,
        platform,
    })
}
