//! Patch notes and news bodies: plain text with a little shape
//! (docs/RELEASES.md, *Patch notes*). A line `# Heading` is a heading, a line
//! `- item` (or `* item`) a bullet, a blank line ends a paragraph, and any
//! other lines run together into a paragraph.

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Block {
    Heading(String),
    Bullet(String),
    Paragraph(String),
}

pub(crate) fn parse(text: &str) -> Vec<Block> {
    let mut out = Vec::new();
    let mut para = String::new();
    // The last line was a bullet's, so a plain line carries it on.
    let mut in_bullet = false;
    let flush = |para: &mut String, out: &mut Vec<Block>| {
        if !para.is_empty() {
            out.push(Block::Paragraph(std::mem::take(para)));
        }
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            flush(&mut para, &mut out);
            in_bullet = false;
        } else if let Some(h) = line.strip_prefix("# ") {
            flush(&mut para, &mut out);
            out.push(Block::Heading(h.trim().to_owned()));
            in_bullet = false;
        } else if let Some(b) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
            flush(&mut para, &mut out);
            out.push(Block::Bullet(b.trim().to_owned()));
            in_bullet = true;
        } else if let (Some(Block::Bullet(b)), true) = (out.last_mut(), in_bullet) {
            b.push(' ');
            b.push_str(line);
        } else {
            if !para.is_empty() {
                para.push(' ');
            }
            para.push_str(line);
        }
    }
    flush(&mut para, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_bullets_and_paragraphs() {
        let text = "# Units\n- Wardens turn faster\n  on the march\n* Flak reaches higher\n\nA paragraph\nthat runs on.\n\n# Maps\nRimehold is new.";
        assert_eq!(
            parse(text),
            vec![
                Block::Heading("Units".into()),
                Block::Bullet("Wardens turn faster on the march".into()),
                Block::Bullet("Flak reaches higher".into()),
                Block::Paragraph("A paragraph that runs on.".into()),
                Block::Heading("Maps".into()),
                Block::Paragraph("Rimehold is new.".into()),
            ]
        );
    }
}
