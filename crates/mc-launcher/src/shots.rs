//! `MeridianConflict --screenshot DIR`: the window drawn headless in each of
//! its states, one PNG apiece, for review. Each scene is a real install in a
//! scratch folder (fake builds, but on disk as a player's would be) with the
//! store's answers already in.

use std::path::{Path, PathBuf};

use mc_builds::feed::{History, Link, News, NewsItem, Release};
use mc_builds::versions::Progress;
use mc_builds::{Entry, Manifest, Store};
use mc_core::Channel;

use crate::app::{App, Files, Lane, Page, Remote, Replay, ReplayStep};
use crate::install::Install;
use crate::paint::{Backdrop, Painter};
use crate::screens;
use crate::ui::{Input, Memory, Ui};
use crate::work::{Job, Meter};

const NOTES_NEW: &str = "# Clouds\n- Cloud shadows hold solid at thin clouds' edges instead of flickering.\n- The shade map costs about a millisecond, down from six.\n\n# Units\n- The ARC commander's shoulder flak sits in an open shielded mount and rests up when idle.\n- Regency Sowers carry their seekers in cases.\n\n# Maps\n- Rimehold: a 20 km arctic survival map for four.";
const NOTES_OLD: &str = "# Interface\n- Prefabs: save a block of structures and lay it again from the build panel.\n- Engineering stations, T1 to T3, build and assist like a fixed engineer.\n\n# Fixes\n- A unit sealed in by hulls walks out on its own.";
const NOTES_OLDER: &str = "- The Last Harvest: a 24 km survival map for four defenders.\n- Site Repeat lays a half-built field again with one key.";

/// The scenes, by file name.
const SCENES: [&str; 7] = [
    "1-home",
    "2-updating",
    "3-news",
    "4-builds",
    "5-playtest-install",
    "6-offline",
    "7-replay",
];

pub(crate) fn write_all(dir: &Path) -> Result<Vec<PathBuf>, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut out = Vec::new();
    for scene in SCENES {
        for scale in [1.0, 1.5] {
            let path = if scale == 1.0 {
                dir.join(format!("{scene}.png"))
            } else {
                dir.join(format!("{scene}@1.5x.png"))
            };
            let root = std::env::temp_dir().join(format!(
                "meridian-launcher-shot-{}-{scene}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&root);
            let mut app = stage(scene, &root)?;
            let png = draw(&mut app, scene, scale);
            let _ = std::fs::remove_dir_all(&root);
            std::fs::write(&path, png?).map_err(|e| format!("{}: {e}", path.display()))?;
            out.push(path);
        }
    }
    Ok(out)
}

fn manifest(number: u32, channel: Channel, created: u64, notes: &str) -> Manifest {
    let suffix = if channel == Channel::Release {
        String::new()
    } else {
        format!("-{channel}")
    };
    let commit = format!("{:010x}", 0x003b_f881_5a00_u64 + number as u64 * 7919);
    Manifest {
        build: format!("0.1.0{suffix}+{commit}"),
        number,
        channel: channel.name().to_owned(),
        commit,
        sim: "a0bf5e5461660d67".into(),
        platform: mc_builds::platform().into(),
        created,
        notes: notes.into(),
        files: vec![
            Entry {
                path: mc_builds::game_exe(),
                size: 61_400_000 + number as u64 * 1000,
                sha256: format!("{:064x}", number),
                packed: 19_800_000,
            },
            Entry {
                path: "maps/rimehold.mcmap".into(),
                size: 412_000_000,
                sha256: format!("{:064x}", 1),
                packed: 233_000_000,
            },
        ],
    }
}

/// Puts `m` on disk as an installed build.
fn put(install: &Install, m: &Manifest) -> Result<(), String> {
    let dir = install.versions.dir(&m.key());
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("manifest.json"), m.to_bytes()).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(mc_builds::game_exe()), b"").map_err(|e| e.to_string())
}

fn done<T>(value: T) -> Option<Job<T>> {
    Some(Job::fixed(Some(Ok(value)), Meter::default()))
}

fn failed<T>(why: &str) -> Option<Job<T>> {
    Some(Job::fixed(Some(Err(why.to_owned())), Meter::default()))
}

fn running<T>(stage: &'static str, done: u64, total: u64, rate: f64) -> Option<Job<T>> {
    Some(Job::fixed(
        None,
        Meter {
            stage,
            progress: Some(Progress { done, total }),
            rate,
        },
    ))
}

const DAY: u64 = 86_400;
/// 14 Oct 2026.
const TODAY: u64 = 1_791_936_000;

fn stage(scene: &str, root: &Path) -> Result<App, String> {
    let install = Install::new(root.join("MeridianConflict.exe"), root.to_path_buf());
    let r1603 = manifest(1603, Channel::Release, TODAY - 2 * DAY, NOTES_NEW);
    let r1588 = manifest(1588, Channel::Release, TODAY - 9 * DAY, NOTES_OLD);
    let r1561 = manifest(1561, Channel::Release, TODAY - 16 * DAY, NOTES_OLDER);
    let p1622 = manifest(1622, Channel::Playtest, TODAY, NOTES_NEW);
    let installed: Vec<&Manifest> = match scene {
        "2-updating" => vec![&r1588],
        "4-builds" => vec![&r1603, &r1588, &r1561],
        "7-replay" => vec![&r1603],
        _ => vec![&r1603, &r1588],
    };
    for m in &installed {
        put(&install, m)?;
    }
    if let Some(first) = installed.first() {
        install
            .versions
            .set_current(first)
            .map_err(|e| e.to_string())?;
    }
    if scene == "5-playtest-install" {
        install.set_channel(Channel::Playtest);
    }
    let store = Store::new("https://builds.example.invalid").ok();
    let mut app = App::new(install, store);
    app.still = true;
    let history = |builds: &[&Manifest]| History {
        builds: builds
            .iter()
            .map(|m| Release {
                build: m.build.clone(),
                number: m.number,
                created: m.created,
                notes: m.notes.clone(),
            })
            .collect(),
    };
    let release = Remote {
        newest: r1603.clone(),
        history: history(&[&r1603, &r1588, &r1561]),
    };
    let playtest = Remote {
        newest: p1622.clone(),
        history: history(&[&p1622]),
    };
    app.news = done(News {
        items: vec![
            NewsItem {
                date: "2026-10-14".into(),
                title: "Rimehold is out".into(),
                body: "A 20 km arctic map for four, made for survival.\n\n- Four defenders hold the ice shelf against the Progenitor.\n- Blizzards roll in between rounds and hide what comes next.".into(),
            },
            NewsItem {
                date: "2026-10-07".into(),
                title: "Playtest weekend".into(),
                body: "The playtest server is open all weekend. Pick Playtest under Channel to join in; it keeps its own build beside the release one.".into(),
            },
        ],
        links: vec![
            Link { label: "Website".into(), url: "https://example.invalid".into() },
            Link { label: "Discord".into(), url: "https://example.invalid".into() },
            Link { label: "Report a bug".into(), url: "https://example.invalid".into() },
        ],
    });
    for lane in &mut app.lanes {
        set_lane(lane, scene, &release, &playtest);
    }
    match scene {
        "3-news" => app.page = Page::News,
        "4-builds" => {
            app.page = Page::Builds;
            let key = r1588.key();
            app.files = Some(Files::Checking(
                key,
                running("Checking files", 286_000_000, 473_000_000, 0.0).unwrap(),
            ));
        }
        "7-replay" => {
            app.replay = Some(Replay {
                file: root.join("replays/2026-10-02-crosswater-4p.mcreplay"),
                build: r1561.build.clone(),
                step: ReplayStep::Fetching(
                    running("Downloading", 9_200_000, 21_400_000, 6_100_000.0).unwrap(),
                ),
            });
        }
        _ => {}
    }
    Ok(app)
}

fn set_lane(lane: &mut Lane, scene: &str, release: &Remote, playtest: &Remote) {
    let clone = |r: &Remote| Remote {
        newest: r.newest.clone(),
        history: r.history.clone(),
    };
    match lane.channel {
        Channel::Release => {
            lane.addr = Some("play.example.invalid".into());
            lane.server = done(true);
            lane.remote = match scene {
                "6-offline" => failed("could not resolve host builds.example.invalid"),
                _ => done(clone(release)),
            };
            if scene == "2-updating" {
                lane.update = running("Downloading", 118_000_000, 252_000_000, 14_600_000.0);
            }
        }
        _ => {
            lane.addr = Some("playtest.example.invalid".into());
            lane.server = done(false);
            lane.remote = done(clone(playtest));
        }
    }
}

fn draw(app: &mut App, scene: &str, scale: f32) -> Result<Vec<u8>, String> {
    let (w, h) = screens::SIZE;
    let (pw, ph) = ((w * scale) as u32, (h * scale) as u32);
    let mut painter = Painter::new(pw, ph, scale, Backdrop::new());
    let mut memory = Memory::default();
    // The pointer over Play on the first scene, to show its hover.
    let cursor = if scene == "1-home" {
        (226.0, 670.0)
    } else {
        (-1.0, -1.0)
    };
    let input = Input {
        cursor,
        ..Input::default()
    };
    // Two frames: the first settles what eases in.
    for _ in 0..2 {
        painter.begin(pw, ph, scale);
        let mut ui = Ui::new(&mut painter, &input, &mut memory, 1.3, 1.0);
        screens::frame(&mut ui, app);
    }
    encode(&painter)
}

fn encode(p: &Painter) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, p.px.width(), p.px.height());
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().map_err(|e| e.to_string())?;
        let rgb: Vec<u8> =
            p.px.pixels()
                .iter()
                .flat_map(|c| [c.red(), c.green(), c.blue()])
                .collect();
        w.write_image_data(&rgb).map_err(|e| e.to_string())?;
    }
    Ok(out)
}
