//! The system clipboard, for copying ids and links out and pasting them into fields.

/// Puts `text` on the system clipboard.
pub fn copy(text: &str) -> Result<(), String> {
    arboard::Clipboard::new()
        .and_then(|mut c| c.set_text(text.to_owned()))
        .map_err(|e| e.to_string())
}

/// The clipboard's text, or `None` when it holds none (or cannot be opened).
pub fn paste() -> Option<String> {
    arboard::Clipboard::new()
        .and_then(|mut c| c.get_text())
        .ok()
}
