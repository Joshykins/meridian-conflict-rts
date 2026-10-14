use super::*;
use crate::ui::maps::{Browser, BrowserAction, MapCard};
pub(super) struct Browse {
    pub(super) browser: Browser,
    maps: Vec<MapCard>,
}
impl Editor {
    pub(crate) fn open_browser(&mut self) -> Result<(), String> {
        if self.doc.dirty {
            return Err("Save the current map before opening another map.".into());
        }
        let maps: Vec<_> = crate::setup::list_maps()
            .iter()
            .filter_map(|path| {
                let config = mc_data::weather::MapConfig::for_map(path).unwrap_or_default();
                crate::ui::maps::open_card(path, &config)
            })
            .collect();
        let selected = maps
            .iter()
            .position(|c| c.map.path() == self.doc.source)
            .unwrap_or(0);
        let mut browser = Browser::new(&maps);
        browser.open(selected);
        self.browser = Some(Browse { browser, maps });
        Ok(())
    }
    pub(super) fn browser_frame(&mut self, ui: &mut Ui) -> Option<Event> {
        let browse = self.browser.as_mut()?;
        match browse
            .browser
            .draw(ui, &browse.maps, "OPEN MAP IN EDITOR", 0)
        {
            Some(BrowserAction::Pick(index)) => {
                let path = browse.maps.get(index)?.map.path().to_owned();
                match Editor::open(
                    &path,
                    self.blueprints.clone(),
                    self.pool.clone(),
                    self.camera.viewport,
                ) {
                    Ok(editor) => Some(Event::Open(Box::new(editor))),
                    Err(e) => {
                        self.message = e;
                        None
                    }
                }
            }
            _ => None,
        }
    }
}
