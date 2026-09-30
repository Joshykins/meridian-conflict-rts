//! The test range's map browser: the skirmish maps as cards with their charts,
//! the one under the pointer drawn large, so a map is picked by sight. Picking
//! one stands the range back up there with the same subject.
//!
//! The maps are read on a worker the first time the Range tab is shown. While
//! the browser is up it borrows two of the unit pictures' image slots (its
//! thumbnails and its large chart), so the rest of the HUD is not drawn, and the
//! pictures go back once it has faded out.

use crate::ui::lineup::Catalog;
use crate::ui::maps::BrowserAction;
use crate::ui::Ui;
use mc_map::MapFile;
use std::sync::mpsc::{channel, Receiver};

/// The unit pictures' slot the large chart is drawn into (the thumbnails take slot 3).
const CHART_SLOT: usize = 2;

#[derive(Default)]
pub struct RangeMaps {
    /// The maps once read, and their browser.
    catalog: Option<Catalog>,
    loading: Option<Receiver<Catalog>>,
}

impl RangeMaps {
    /// Starts reading the maps, once.
    pub fn prepare(&mut self) {
        if self.catalog.is_some() || self.loading.is_some() {
            return;
        }
        let (tx, rx) = channel();
        let spawned = std::thread::Builder::new()
            .name("range-maps".into())
            .spawn(move || {
                let _ = tx.send(Catalog::load(false));
            });
        match spawned {
            Ok(_) => self.loading = Some(rx),
            Err(e) => log::warn!("no thread to read the maps: {e}"),
        }
    }

    /// Reads the maps at `paths` at once, their thumbnails drawn (shots and tests).
    pub(super) fn load_now(&mut self, paths: Vec<std::path::PathBuf>) {
        let mut catalog = Catalog::load_from(paths, false);
        catalog.browser.wait_for_thumbs();
        self.catalog = Some(catalog);
    }

    #[cfg(test)]
    pub(super) fn browser(&self) -> Option<&crate::ui::maps::Browser> {
        self.catalog.as_ref().map(|c| &c.browser)
    }

    /// Whether there are maps to browse; takes them in once read.
    pub fn ready(&mut self) -> bool {
        if let Some(catalog) = self.loading.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.catalog = Some(catalog);
            self.loading = None;
        }
        self.catalog.as_ref().is_some_and(|c| !c.maps.is_empty())
    }

    /// Opens the browser on `current` when it is one of the maps.
    pub fn open(&mut self, current: &MapFile) {
        if let Some(c) = &mut self.catalog {
            c.browser.open(at(c, current));
        }
    }

    /// Opens it fully arrived (shots).
    pub(super) fn open_now(&mut self, current: &MapFile) {
        if let Some(c) = &mut self.catalog {
            c.browser.open_now(at(c, current));
        }
    }

    pub fn is_open(&self) -> bool {
        self.catalog.as_ref().is_some_and(|c| c.browser.is_open())
    }

    /// Open, or still fading out: the image slots are the browser's.
    pub fn is_shown(&self) -> bool {
        self.catalog.as_ref().is_some_and(|c| c.browser.is_shown())
    }

    /// Draws the browser; the file stem of a map picked other than `current`.
    pub fn draw(&mut self, ui: &mut Ui, current: &MapFile) -> Option<String> {
        let c = self.catalog.as_mut()?;
        c.browser.pump(ui);
        match c.browser.draw(ui, &c.maps, "Test Range Map", CHART_SLOT)? {
            BrowserAction::Pick(i) => c
                .maps
                .get(i)
                .filter(|m| m.map.content_id() != current.content_id())
                .map(|m| m.stem.clone()),
            BrowserAction::Cancel => None,
        }
    }
}

/// Where `current` is among the maps, or the first.
fn at(c: &Catalog, current: &MapFile) -> usize {
    c.maps
        .iter()
        .position(|m| m.map.content_id() == current.content_id())
        .unwrap_or(0)
}
