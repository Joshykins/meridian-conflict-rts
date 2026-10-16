//! What the launcher knows and does, apart from drawing it (`screens/`): each
//! channel's newest build and history, the update to it, the news, checking
//! and repairing files, and an old replay's build.
//!
//! The selected channel's update starts by itself once the store names a
//! build that is not installed, and Play waits for it: a network match needs
//! everyone on the same build.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use mc_builds::feed::{History, News};
use mc_builds::versions::Progress;
use mc_builds::{Entry, Manifest, Store};
use mc_core::Channel;

use crate::install::{Install, Log, CHANNELS};
use crate::work::{Job, Meter};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Page {
    Notes,
    News,
    Builds,
}

/// What the store says of a channel.
pub(crate) struct Remote {
    /// `None`: nothing is published on the channel yet (Release, before it
    /// comes out).
    pub(crate) newest: Option<Manifest>,
    pub(crate) history: History,
}

/// One channel: what the store says, and its update.
pub(crate) struct Lane {
    pub(crate) channel: Channel,
    pub(crate) remote: Option<Job<Remote>>,
    /// A later look at the store, while `remote` still shows the last one.
    recheck: Option<Job<Remote>>,
    pub(crate) update: Option<Job<Manifest>>,
    /// The update whose result was acted on, so it is acted on once.
    applied: bool,
}

impl Lane {
    fn new(channel: Channel) -> Lane {
        Lane {
            channel,
            remote: None,
            recheck: None,
            update: None,
            applied: false,
        }
    }

    /// The store's newest build of this channel, once known.
    pub(crate) fn newest(&self) -> Option<Manifest> {
        self.remote.as_ref()?.peek(|r| {
            r.and_then(|r| r.as_ref().ok())
                .and_then(|r| r.newest.clone())
        })
    }

    /// The store answered, and has nothing on this channel yet.
    pub(crate) fn unreleased(&self) -> bool {
        self.remote.as_ref().is_some_and(|j| {
            j.peek(|r| {
                r.and_then(|r| r.as_ref().ok())
                    .is_some_and(|r| r.newest.is_none())
            })
        })
    }

    pub(crate) fn remote_error(&self) -> Option<String> {
        self.remote
            .as_ref()?
            .peek(|r| r.and_then(|r| r.as_ref().err()).cloned())
    }

    pub(crate) fn updating(&self) -> bool {
        self.update.as_ref().is_some_and(Job::running)
    }

    pub(crate) fn update_error(&self) -> Option<String> {
        self.update
            .as_ref()?
            .peek(|r| r.and_then(|r| r.as_ref().err()).cloned())
    }
}

/// Checking an installed build's files, then repairing them.
pub(crate) enum Files {
    Checking(String, Job<Vec<Entry>>),
    Repairing(String, Job<usize>),
    /// The last check's word on it.
    Done(String, Result<String, String>),
}

/// An old replay being played in the build that recorded it.
pub(crate) struct Replay {
    pub(crate) file: PathBuf,
    pub(crate) build: String,
    pub(crate) step: ReplayStep,
}

pub(crate) enum ReplayStep {
    Fetching(Job<Manifest>),
    /// The old build is running; the window is hidden till it closes.
    Playing(Job<()>),
    Failed(String),
}

/// Where the player is in the right-hand panel.
#[derive(Default)]
pub(crate) struct Reader {
    /// How far down the page is scrolled, in points.
    pub(crate) scroll: f32,
    /// How far it can scroll, as the last frame measured it.
    pub(crate) room: f32,
    /// Builds whose patch notes the player opened or shut: the newest starts
    /// open and the rest shut, and a build here is the other way round.
    pub(crate) toggled: BTreeSet<String>,
}

impl Reader {
    pub(crate) fn notes_open(&self, build: &str, newest: bool) -> bool {
        newest != self.toggled.contains(build)
    }

    pub(crate) fn toggle_notes(&mut self, build: &str) {
        if !self.toggled.remove(build) {
            self.toggled.insert(build.to_owned());
        }
    }
}

/// What the window does for the app.
pub(crate) enum Action {
    /// Start this build with these arguments, and close.
    Play(Manifest, Vec<String>),
    /// Swap in the launcher's update and start it again.
    Relaunch,
    Open(String),
    Hide,
    Close,
}

pub(crate) struct App {
    pub(crate) install: Install,
    pub(crate) store: Option<Store>,
    pub(crate) channel: Channel,
    pub(crate) page: Page,
    pub(crate) lanes: Vec<Lane>,
    pub(crate) news: Option<Job<News>>,
    /// A later read of the news, while `news` still shows the last one.
    news_recheck: Option<Job<News>>,
    /// When the store was last looked at, for looking again while the window is open.
    looked: Instant,
    pub(crate) files: Option<Files>,
    /// Installed builds with the bytes only each holds, and the total.
    pub(crate) installed: (Vec<(Manifest, u64)>, u64),
    pub(crate) replay: Option<Replay>,
    /// A build waiting for a second press of Remove.
    pub(crate) removing: Option<String>,
    pub(crate) reader: Reader,
    /// Something to tell the player at the foot of the window.
    pub(crate) notice: Option<String>,
    /// The scenes for `--screenshot` start no work of their own.
    pub(crate) still: bool,
}

impl App {
    pub(crate) fn new(install: Install, store: Option<Store>) -> App {
        let channel = install.channel();
        let installed = install.versions.usage();
        App {
            install,
            store,
            channel,
            page: Page::Notes,
            lanes: CHANNELS.iter().map(|c| Lane::new(*c)).collect(),
            news: None,
            news_recheck: None,
            looked: Instant::now(),
            files: None,
            installed,
            replay: None,
            removing: None,
            reader: Reader::default(),
            notice: None,
            still: false,
        }
    }

    pub(crate) fn lane(&self) -> &Lane {
        self.lanes
            .iter()
            .find(|l| l.channel == self.channel)
            .unwrap_or(&self.lanes[0])
    }

    fn lane_mut(&mut self, channel: Channel) -> Option<&mut Lane> {
        self.lanes.iter_mut().find(|l| l.channel == channel)
    }

    /// The build Play starts on the selected channel.
    pub(crate) fn current(&self) -> Option<Manifest> {
        self.install.versions.current(self.channel)
    }

    pub(crate) fn refresh_installed(&mut self) {
        self.installed = self.install.versions.usage();
    }

    pub(crate) fn pick_channel(&mut self, channel: Channel) {
        if channel != self.channel {
            self.channel = channel;
            self.reader.scroll = 0.0;
            self.install.set_channel(channel);
        }
    }

    /// Opens `page` in the right-hand panel, at its top.
    pub(crate) fn show(&mut self, page: Page) {
        self.page = page;
        self.reader.scroll = 0.0;
    }

    /// Starts what needs starting and acts on what finished. Once a frame.
    pub(crate) fn step(&mut self, log: &mut Log) -> Option<Action> {
        if self.still {
            return None;
        }
        if let Some(action) = self.step_replay(log) {
            return Some(action);
        }
        let store = self.store.clone();
        if let Some(store) = &store {
            if self.news.is_none() {
                self.news = Some(read_news(store));
            }
            for lane in &mut self.lanes {
                if lane.remote.is_none() {
                    lane.remote = Some(check(store, lane.channel));
                }
            }
            self.look_again(store);
        }
        // The selected channel updates by itself.
        let channel = self.channel;
        let current = self.current();
        let lane = self.lanes.iter().find(|l| l.channel == channel)?;
        // A finished update of another build does not stand in the way of this one.
        let free = lane.update.as_ref().is_none_or(|j| {
            j.peek(|r| matches!(r, Some(Ok(m)) if Some(&m.build) != lane.newest().as_ref().map(|n| &n.build)))
        });
        if let (Some(newest), true, Some(store)) = (lane.newest(), free, &store) {
            let wanted = !self.install.versions.has(&newest.build)
                && current.as_ref().is_none_or(|c| newest.number > c.number);
            // An update; or a fresh install (the setup just ran), which gets
            // its picked channel's build straight away.
            let fresh = self.installed.0.is_empty();
            if wanted && (current.is_some() || fresh) {
                log.line(&format!("installing {}", newest.build));
                self.start_install(store.clone(), newest);
            }
        }
        self.apply_update(log)
    }

    /// Looks at the store again every [`LOOK_AGAIN`] while the window is open,
    /// so a build published meanwhile is fetched without a restart. What the
    /// window shows stays until the new answer is in; a failed look keeps it.
    fn look_again(&mut self, store: &Store) {
        let busy = self.lanes.iter().any(Lane::updating);
        if !busy && self.looked.elapsed() >= LOOK_AGAIN {
            self.looked = Instant::now();
            for lane in &mut self.lanes {
                lane.recheck = Some(check(store, lane.channel));
            }
            self.news_recheck = Some(read_news(store));
        }
        for lane in &mut self.lanes {
            let answer = lane
                .recheck
                .as_ref()
                .and_then(|j| j.peek(|r| r.map(Result::is_ok)));
            match answer {
                Some(true) => lane.remote = lane.recheck.take(),
                Some(false) => lane.recheck = None,
                None => {}
            }
        }
        let answer = self
            .news_recheck
            .as_ref()
            .and_then(|j| j.peek(|r| r.map(Result::is_ok)));
        match answer {
            Some(true) => self.news = self.news_recheck.take(),
            Some(false) => self.news_recheck = None,
            None => {}
        }
    }

    /// Installs `m` on its channel's lane.
    pub(crate) fn start_install(&mut self, store: Store, m: Manifest) {
        let Some(channel) = m.channel() else {
            return;
        };
        let root = self.install.root.clone();
        let Some(lane) = self.lane_mut(channel) else {
            return;
        };
        lane.applied = false;
        lane.update = Some(Job::spawn("update", move |r| {
            r.stage("Downloading");
            let versions = mc_builds::Versions::new(&root);
            versions
                .install(&store, &m, &mut |p| r.progress(p), r.cancel_flag())
                .map_err(|e| e.to_string())?;
            Ok(m)
        }));
    }

    /// An update that finished becomes its channel's current build, and the
    /// launcher it brought is swapped in.
    fn apply_update(&mut self, log: &mut Log) -> Option<Action> {
        let mut done = None;
        for lane in &mut self.lanes {
            if lane.applied {
                continue;
            }
            let Some(job) = &lane.update else {
                continue;
            };
            let finished = job.peek(|r| r.map(|r| r.clone().map(|m| m.build.clone())));
            match finished {
                None => {}
                Some(Ok(_)) => {
                    lane.applied = true;
                    done = job.peek(|r| r.and_then(|r| r.as_ref().ok()).cloned());
                }
                Some(Err(e)) => {
                    lane.applied = true;
                    log.line(&format!("update failed: {e}"));
                }
            }
        }
        let m = done?;
        log.line(&format!("installed {}", m.build));
        if let Err(e) = self.install.versions.set_current(&m) {
            log.line(&format!("could not make {} current: {e}", m.build));
        }
        self.refresh_installed();
        match self.install.stage_launcher(&m) {
            Ok(true) => Some(Action::Relaunch),
            Ok(false) => None,
            Err(e) => {
                log.line(&format!("could not stage the launcher's update: {e}"));
                None
            }
        }
    }

    /// Tries the update again after it failed.
    pub(crate) fn retry(&mut self) {
        let channel = self.channel;
        let store = self.store.clone();
        let Some(lane) = self.lane_mut(channel) else {
            return;
        };
        if lane.update_error().is_some() {
            lane.update = None;
        } else {
            lane.remote = None;
        }
        // Nothing installed: the check alone does not start an install.
        if let (Some(store), Some(newest)) = (store, lane.newest()) {
            if lane.update.is_none() && self.current().is_none() {
                self.start_install(store, newest);
            }
        }
    }

    pub(crate) fn cancel_update(&mut self) {
        let channel = self.channel;
        if let Some(lane) = self.lane_mut(channel) {
            if let Some(job) = lane.update.take() {
                job.cancel();
            }
            lane.applied = false;
        }
    }

    /// Checks every file of an installed build against its manifest.
    pub(crate) fn check_files(&mut self, m: &Manifest) {
        let (root, m, key) = (self.install.root.clone(), m.clone(), m.key());
        self.files = Some(Files::Checking(
            key,
            Job::spawn("check files", move |r| {
                r.stage("Checking files");
                mc_builds::Versions::new(&root)
                    .verify(&m, &mut |p| r.progress(p), r.cancel_flag())
                    .map_err(|e| e.to_string())
            }),
        ));
    }

    /// Advances checking into repairing, and a repair into its report.
    pub(crate) fn step_files(&mut self) {
        let next = match &self.files {
            Some(Files::Checking(key, job)) => job.peek(|r| {
                Some(match r? {
                    Err(e) => Files::Done(key.clone(), Err(e.clone())),
                    Ok(bad) if bad.is_empty() => {
                        Files::Done(key.clone(), Ok("Every file is sound".into()))
                    }
                    Ok(bad) => self.repair(key, bad.clone()),
                })
            }),
            Some(Files::Repairing(key, job)) => job.peek(|r| {
                Some(Files::Done(
                    key.clone(),
                    r?.clone().map(|n| {
                        format!("Repaired {n} damaged file{}", if n == 1 { "" } else { "s" })
                    }),
                ))
            }),
            _ => None,
        };
        if let Some(next) = next {
            self.files = Some(next);
        }
    }

    fn repair(&self, key: &str, bad: Vec<Entry>) -> Files {
        let Some(store) = self.store.clone() else {
            return Files::Done(
                key.to_owned(),
                Err(format!(
                    "{} files are damaged, and there is no store to mend them from",
                    bad.len()
                )),
            );
        };
        let Some(m) = self
            .installed
            .0
            .iter()
            .map(|(m, _)| m)
            .find(|m| m.key() == key)
            .cloned()
        else {
            return Files::Done(key.to_owned(), Err("the build is gone".into()));
        };
        let root = self.install.root.clone();
        Files::Repairing(
            key.to_owned(),
            Job::spawn("repair", move |r| {
                r.stage("Repairing");
                mc_builds::Versions::new(&root)
                    .repair(&store, &m, &bad, &mut |p| r.progress(p), r.cancel_flag())
                    .map_err(|e| e.to_string())?;
                Ok(bad.len())
            }),
        )
    }

    /// Removes an installed build that no channel starts.
    pub(crate) fn remove(&mut self, key: &str) {
        if let Err(e) = self.install.versions.remove(key) {
            self.notice = Some(format!("Could not remove it: {e}"));
        }
        self.removing = None;
        self.refresh_installed();
    }

    /// Plays `file` in the build that recorded it, fetching that build first.
    pub(crate) fn start_replay(&mut self, file: PathBuf, log: &mut Log) {
        let origin = match mc_net::Origin::peek_file(&file) {
            Ok(p) => p.origin,
            Err(e) => {
                self.replay = Some(Replay {
                    file: file.clone(),
                    build: String::new(),
                    step: ReplayStep::Failed(format!("{}: {e}", file.display())),
                });
                return;
            }
        };
        log.line(&format!(
            "replay {} recorded in {}",
            file.display(),
            origin.build
        ));
        let build = origin.build.clone();
        let fetch = match &self.store {
            _ if self.install.versions.has(&build) => {
                let m = self
                    .install
                    .versions
                    .installed()
                    .into_iter()
                    .find(|m| m.build == build);
                match m {
                    Some(m) => Job::fixed(Some(Ok(m)), Meter::default()),
                    None => Job::fixed(Some(Err("the build vanished".into())), Meter::default()),
                }
            }
            Some(store) => {
                let (store, root, build) =
                    (store.clone(), self.install.root.clone(), build.clone());
                Job::spawn("replay build", move |r| {
                    r.stage("Downloading");
                    let m = store.manifest(&build).map_err(|e| e.to_string())?;
                    mc_builds::Versions::new(&root)
                        .install(&store, &m, &mut |p| r.progress(p), r.cancel_flag())
                        .map_err(|e| e.to_string())?;
                    Ok(m)
                })
            }
            None => Job::fixed(
                Some(Err(format!(
                    "{build} is not installed, and this launcher has no store"
                ))),
                Meter::default(),
            ),
        };
        self.replay = Some(Replay {
            file,
            build,
            step: ReplayStep::Fetching(fetch),
        });
    }

    fn step_replay(&mut self, log: &mut Log) -> Option<Action> {
        let replay = self.replay.as_mut()?;
        match &replay.step {
            ReplayStep::Fetching(job) => {
                let got = job.peek(|r| r.cloned());
                match got? {
                    Err(e) => replay.step = ReplayStep::Failed(e),
                    Ok(m) => {
                        let args = vec![
                            "--replay".to_owned(),
                            replay.file.to_string_lossy().into_owned(),
                            "--replay-only".to_owned(),
                        ];
                        log.line(&format!("playing {} in {}", replay.file.display(), m.build));
                        let child = self.install.game(&m, &args).spawn();
                        match child {
                            Ok(mut child) => {
                                replay.step =
                                    ReplayStep::Playing(Job::spawn("old build", move |_| {
                                        child.wait().map(drop).map_err(|e| e.to_string())
                                    }));
                                return Some(Action::Hide);
                            }
                            Err(e) => replay.step = ReplayStep::Failed(format!("{}: {e}", m.build)),
                        }
                    }
                }
                None
            }
            ReplayStep::Playing(job) => {
                if job.running() {
                    return None;
                }
                log.line("the old build closed");
                // Back to the newest build, on the list the replay came from.
                let current = self.current();
                self.replay = None;
                match current {
                    Some(m) => Some(Action::Play(m, vec!["--open".into(), "history".into()])),
                    None => Some(Action::Close),
                }
            }
            ReplayStep::Failed(_) => None,
        }
    }
}

/// How often an open launcher looks for a newer build.
const LOOK_AGAIN: Duration = Duration::from_secs(10 * 60);

/// What the store says of `channel`: its newest build and its history.
fn check(store: &Store, channel: Channel) -> Job<Remote> {
    let s = store.clone();
    Job::spawn("check", move |_| {
        let newest = match s.newest(channel) {
            Ok(m) => m,
            Err(mc_builds::Error::Missing(_)) => {
                return Ok(Remote {
                    newest: None,
                    history: History::default(),
                })
            }
            Err(e) => return Err(e.to_string()),
        };
        // A store without a history yet still has builds.
        let history = s.history(channel).unwrap_or_default();
        Ok(Remote {
            newest: Some(newest),
            history,
        })
    })
}

fn read_news(store: &Store) -> Job<News> {
    let s = store.clone();
    Job::spawn("news", move |_| s.news().map_err(|e| e.to_string()))
}

/// "84.2 MB".
pub(crate) fn megabytes(bytes: u64) -> String {
    let mb = bytes as f64 / 1e6;
    if mb >= 1000.0 {
        format!("{:.2} GB", mb / 1000.0)
    } else if mb >= 10.0 {
        format!("{mb:.0} MB")
    } else {
        format!("{mb:.1} MB")
    }
}

/// "12 Oct 2026" from Unix seconds.
pub(crate) fn date(unix: u64) -> String {
    let days = (unix / 86_400) as i64;
    // Civil from days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    format!("{d} {} {y}", MONTHS[(m - 1) as usize])
}

/// "Downloading 45%   ·   120 of 260 MB   ·   12 MB/s   ·   0:42 left".
pub(crate) fn meter_line(meter: &Meter) -> String {
    let (head, tail) = meter_lines(meter);
    if tail.is_empty() {
        head
    } else {
        format!("{head}   \u{b7}   {tail}")
    }
}

/// The meter in two: "Downloading 45%", and "120 of 260 MB   ·   12 MB/s   ·   0:42 left".
pub(crate) fn meter_lines(meter: &Meter) -> (String, String) {
    let Some(p) = meter.progress else {
        return (format!("{}\u{2026}", meter.stage), String::new());
    };
    let Progress { done, total } = p;
    if total > 0 && done >= total {
        return ("Unpacking\u{2026}".to_owned(), String::new());
    }
    let head = format!(
        "{} {:.0}%",
        meter.stage,
        100.0 * meter.fraction().unwrap_or(0.0)
    );
    let mut line = format!("{} of {}", megabytes(done), megabytes(total));
    if meter.rate > 0.0 {
        line += &format!("   \u{b7}   {}/s", megabytes(meter.rate as u64));
    }
    if let Some(eta) = meter.eta() {
        let s = eta.round() as u64;
        line += &format!("   \u{b7}   {}:{:02} left", s / 60, s % 60);
    }
    (head, line)
}

/// "14 Oct 2026" from the news' "2026-10-14".
pub(crate) fn news_date(ymd: &str) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let mut parts = ymd.split('-').map(|p| p.parse::<usize>().ok());
    match (
        parts.next().flatten(),
        parts.next().flatten(),
        parts.next().flatten(),
    ) {
        (Some(y), Some(m @ 1..=12), Some(d)) => format!("{d} {} {y}", MONTHS[m - 1]),
        _ => ymd.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_and_sizes_read_plainly() {
        assert_eq!(date(0), "1 Jan 1970");
        assert_eq!(date(1_791_936_000), "14 Oct 2026");
        assert_eq!(megabytes(84_200_000), "84 MB");
        assert_eq!(megabytes(4_200_000), "4.2 MB");
        assert_eq!(megabytes(2_340_000_000), "2.34 GB");
    }
}
