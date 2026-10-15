//! What the launcher shows beside the builds: each channel's recent builds
//! with their patch notes, and the news. Both are signed with the release key
//! like manifests, so the launcher trusts the links it opens from them.
//!
//! ```text
//! channels/<channel>/<platform>-history.json (+ .sig)   the channel's last builds, newest first
//! news.json (+ .sig)                                      news items and the launcher's links
//! ```
//!
//! The history is written by `mc-release pack --newest` with the pointer, the
//! news by `mc-release news` (scripts/release.sh news).

use std::path::Path;

use ed25519_dalek::SigningKey;
use mc_core::Channel;
use serde::{Deserialize, Serialize};

use crate::manifest::{Manifest, MAX_NOTES};
use crate::{Error, Result};

/// Builds a channel's history keeps.
pub const HISTORY_LEN: usize = 20;
/// The most a history or the news may be.
pub const MAX_FEED: u64 = 1 << 20;
const MAX_ITEMS: usize = 50;
const MAX_LINKS: usize = 8;
const MAX_TEXT: usize = 4 << 10;

/// A channel's last builds, newest first.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct History {
    pub builds: Vec<Release>,
}

/// One published build as the history names it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub build: String,
    pub number: u32,
    /// Unix seconds.
    pub created: u64,
    #[serde(default)]
    pub notes: String,
}

impl History {
    /// Where a channel's history is kept in a store.
    pub fn path(channel: Channel, platform: &str) -> String {
        format!("channels/{channel}/{platform}-history.json")
    }

    /// The history with `m` first: once, and no more than [`HISTORY_LEN`].
    pub fn with(mut self, m: &Manifest) -> History {
        self.builds.retain(|r| r.build != m.build);
        self.builds.insert(
            0,
            Release {
                build: m.build.clone(),
                number: m.number,
                created: m.created,
                notes: m.notes.clone(),
            },
        );
        self.builds.truncate(HISTORY_LEN);
        self
    }

    pub fn parse(bytes: &[u8]) -> Result<History> {
        let h: History = parse_json(bytes)?;
        if h.builds.len() > HISTORY_LEN
            || h.builds
                .iter()
                .any(|r| r.build.len() > 64 || r.notes.len() > MAX_NOTES)
        {
            return Err(Error::Malformed("history out of bounds".into()));
        }
        Ok(h)
    }
}

/// The news the launcher shows, and its row of links.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct News {
    #[serde(default)]
    pub items: Vec<NewsItem>,
    #[serde(default)]
    pub links: Vec<Link>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct NewsItem {
    /// `YYYY-MM-DD`.
    pub date: String,
    pub title: String,
    /// Patch-note text (docs/RELEASES.md, *Patch notes*).
    #[serde(default)]
    pub body: String,
}

/// A labelled `https://` address the launcher opens in the browser.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub label: String,
    pub url: String,
}

impl News {
    pub const PATH: &'static str = "news.json";

    /// Parses and checks: bounded text, dates that are dates, and only
    /// `https://` links.
    pub fn parse(bytes: &[u8]) -> Result<News> {
        let n: News = parse_json(bytes)?;
        let bad = |why: String| Err(Error::Malformed(format!("news: {why}")));
        if n.items.len() > MAX_ITEMS || n.links.len() > MAX_LINKS {
            return bad("too many items or links".into());
        }
        for i in &n.items {
            if !is_date(&i.date) {
                return bad(format!("{:?} is not a YYYY-MM-DD date", i.date));
            }
            if i.title.is_empty() || i.title.len() > 200 || i.body.len() > MAX_TEXT {
                return bad(format!("item {:?}: title or body length", i.title));
            }
        }
        for l in &n.links {
            let plain = l.url.starts_with("https://")
                && l.url.len() <= 500
                && !l.url.chars().any(|c| c.is_whitespace() || c == '"');
            if !plain || l.label.is_empty() || l.label.len() > 40 {
                return bad(format!(
                    "link {:?}: an https:// URL and a short label",
                    l.label
                ));
            }
        }
        Ok(n)
    }
}

fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

fn parse_json<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T> {
    if bytes.len() as u64 > MAX_FEED {
        return Err(Error::Malformed("larger than a feed may be".into()));
    }
    serde_json::from_slice(bytes).map_err(|e| Error::Malformed(e.to_string()))
}

/// Writes `value` to `path` in the store folder, with its signature beside it.
pub fn sign_into<T: Serialize>(
    store: &Path,
    path: &str,
    value: &T,
    key: &SigningKey,
) -> Result<()> {
    let mut bytes =
        serde_json::to_vec_pretty(value).map_err(|e| Error::Malformed(e.to_string()))?;
    bytes.push(b'\n');
    let to = store.join(path);
    if let Some(dir) = to.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&to, &bytes)?;
    std::fs::write(
        store.join(format!("{path}.sig")),
        Manifest::sign(&bytes, key),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::tests::sample;

    #[test]
    fn the_history_keeps_the_newest_once() {
        let mut h = History::default();
        for n in 1..=(HISTORY_LEN as u32 + 5) {
            let mut m = sample();
            m.number = n;
            m.build = format!("0.1.0-playtest+{n:010}");
            h = h.with(&m);
        }
        let again = sample();
        let mut m = again.clone();
        m.build = format!("0.1.0-playtest+{:010}", HISTORY_LEN + 5);
        m.number = HISTORY_LEN as u32 + 5;
        h = h.with(&m);
        assert_eq!(h.builds.len(), HISTORY_LEN);
        assert_eq!(h.builds[0].number, HISTORY_LEN as u32 + 5);
        assert_eq!(h.builds[1].number, HISTORY_LEN as u32 + 4);
    }

    /// publish/news.json, which `scripts/release.sh news` publishes, reads.
    #[test]
    fn the_checked_in_news_reads() {
        News::parse(include_bytes!("../../../publish/news.json")).unwrap();
    }

    #[test]
    fn news_links_are_https_only() {
        let ok = br#"{"items":[{"date":"2026-10-14","title":"Hello","body":"- one"}],"links":[{"label":"Site","url":"https://example.com"}]}"#;
        assert_eq!(News::parse(ok).unwrap().items.len(), 1);
        for bad in [
            r#"{"links":[{"label":"x","url":"http://example.com"}]}"#,
            r#"{"links":[{"label":"x","url":"file:///C:/Windows"}]}"#,
            r#"{"links":[{"label":"x","url":"https://a b"}]}"#,
            r#"{"items":[{"date":"14 Oct","title":"x"}]}"#,
            r#"{"items":[{"date":"2026-10-14","title":""}]}"#,
        ] {
            assert!(News::parse(bad.as_bytes()).is_err(), "{bad}");
        }
    }
}
