//! What this build is, as build.rs stamped it: its name, number, channel,
//! commit and simulation fingerprint. Replays carry it ([`origin`]), the relay
//! compares the name, and the build store files builds by it (docs/RELEASES.md).

use mc_core::Channel;
use mc_net::{ContentId, Origin};

/// `<version>+<commit>` for a release, `<version>-<channel>+<commit>`
/// otherwise: network players must match.
pub const BUILD: &str = env!("MERIDIAN_BUILD");

/// The number of commits up to this build, empty outside a git checkout.
pub const NUMBER: &str = env!("MERIDIAN_BUILD_NUMBER");

/// The full commit hash, empty outside a git checkout.
pub const COMMIT: &str = env!("MERIDIAN_COMMIT");

/// The simulation fingerprint, 16 hex digits: builds that share it play each
/// other's replays.
pub const SIM: &str = env!("MERIDIAN_SIM");

pub fn channel() -> Channel {
    env!("MERIDIAN_CHANNEL")
        .parse()
        .expect("build.rs stamps a known channel")
}

pub fn sim() -> u64 {
    u64::from_str_radix(SIM, 16).expect("build.rs stamps 16 hex digits")
}

/// The build number, when there is one.
pub fn number() -> Option<u32> {
    NUMBER.parse().ok()
}

/// This build, as a replay it records names it.
pub fn origin(content: ContentId) -> Origin {
    Origin {
        build: BUILD.to_owned(),
        commit: COMMIT.to_owned(),
        channel: Some(channel()),
        sim: sim(),
        content,
    }
}

/// `meridian --version`: one `key: value` per line, read by scripts/release.sh.
pub fn version_text() -> String {
    format!(
        "build: {BUILD}\nnumber: {NUMBER}\nchannel: {}\ncommit: {COMMIT}\nsim: {SIM}\n",
        channel()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stamp_parses() {
        assert_eq!(sim(), u64::from_str_radix(SIM, 16).unwrap());
        assert!(BUILD.contains(channel().name()) || channel() == Channel::Release);
        assert_eq!(origin(ContentId::default()).build, BUILD);
    }

    /// The fingerprint covers mc-sim's whole workspace closure: a crate it
    /// uses that is not in sim_crates.txt could change the simulation without
    /// changing the fingerprint.
    #[test]
    fn sim_crates_txt_lists_every_crate_the_simulation_uses() {
        let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let list = std::fs::read_to_string(here.join("sim_crates.txt")).unwrap();
        let listed: Vec<&str> = list
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect();
        assert!(listed.contains(&"mc-sim"));
        for krate in &listed {
            let toml = std::fs::read_to_string(here.join("../").join(krate).join("Cargo.toml"))
                .unwrap_or_else(|e| panic!("{krate}: {e}"));
            let deps = toml
                .split("[dependencies]")
                .nth(1)
                .unwrap_or("")
                .split("\n[")
                .next()
                .unwrap_or("");
            for dep in deps.lines().filter_map(|l| l.split_whitespace().next()) {
                if dep.starts_with("mc-") {
                    assert!(
                        listed.contains(&dep),
                        "{krate} uses {dep}, which sim_crates.txt does not list"
                    );
                }
            }
        }
    }
}
