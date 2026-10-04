//! Which audience a build is for. Stamped at compile time from
//! `MERIDIAN_CHANNEL` (`crates/mc-game/build.rs`), written into every replay,
//! and the key the build store files builds under (`docs/RELEASES.md`).

use std::fmt;
use std::str::FromStr;

/// The numbers are written into replays: never renumber one, never reuse a
/// retired number.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[repr(u8)]
pub enum Channel {
    /// Built from a working tree by hand; never published.
    Dev = 0,
    /// Published to playtesters: sees playtest-only content, plays on the
    /// playtest server.
    Playtest = 1,
    /// Published to everyone.
    Release = 2,
}

impl Channel {
    pub const ALL: [Channel; 3] = [Channel::Dev, Channel::Playtest, Channel::Release];

    pub const fn name(self) -> &'static str {
        match self {
            Channel::Dev => "dev",
            Channel::Playtest => "playtest",
            Channel::Release => "release",
        }
    }

    pub fn from_u8(v: u8) -> Option<Channel> {
        Channel::ALL.into_iter().find(|c| *c as u8 == v)
    }

    /// Units and maps marked playtest-only exist in this build.
    pub const fn has_playtest_content(self) -> bool {
        !matches!(self, Channel::Release)
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct UnknownChannel(pub String);

impl fmt::Display for UnknownChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown channel {:?} (dev, playtest or release)", self.0)
    }
}

impl std::error::Error for UnknownChannel {}

impl FromStr for Channel {
    type Err = UnknownChannel;

    fn from_str(s: &str) -> Result<Channel, UnknownChannel> {
        Channel::ALL
            .into_iter()
            .find(|c| c.name() == s)
            .ok_or_else(|| UnknownChannel(s.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_numbers_round_trip() {
        for c in Channel::ALL {
            assert_eq!(c.name().parse::<Channel>(), Ok(c));
            assert_eq!(Channel::from_u8(c as u8), Some(c));
        }
        assert!("beta".parse::<Channel>().is_err());
        assert_eq!(Channel::from_u8(3), None);
    }
}
