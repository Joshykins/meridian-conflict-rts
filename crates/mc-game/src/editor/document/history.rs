//! Pointer drags retain sparse edits while publishing one undo entry.
use super::*;
impl Document {
    pub fn begin_stroke(&mut self) {
        self.stroke.get_or_insert_with(Vec::new);
    }
    pub fn stroking(&self) -> bool {
        self.stroke.is_some()
    }
    /// Returns whether the stroke changed anything.
    pub fn end_stroke(&mut self) -> bool {
        let Some(children) = self.stroke.take().filter(|s| !s.is_empty()) else {
            return false;
        };
        self.undo.push(Change {
            bounds: None,
            children,
            props: None,
            heights: vec![],
            placed: None,
            layers: vec![],
            environment: None,
        });
        true
    }
    pub(super) fn restore_children(
        &mut self,
        children: &[Change],
        forward: bool,
    ) -> Result<Update, String> {
        let mut out = Update::default();
        let indices: Box<dyn Iterator<Item = usize>> = if forward {
            Box::new(0..children.len())
        } else {
            Box::new((0..children.len()).rev())
        };
        for i in indices {
            let next = self.restore(&children[i], forward)?;
            if let Some(b) = next.terrain {
                out.terrain = Some(out.terrain.map_or(b, |a| {
                    [
                        a[0].min(b[0]),
                        a[1].min(b[1]),
                        a[2].max(b[2]),
                        a[3].max(b[3]),
                    ]
                }));
            }
            out.objects |= next.objects;
            out.props |= next.props;
            out.environment |= next.environment;
            out.water |= next.water;
            for (layer, b) in next.cover {
                if let Some((_, a)) = out.cover.iter_mut().find(|(l, _)| *l == layer) {
                    *a = [
                        a[0].min(b[0]),
                        a[1].min(b[1]),
                        a[2].max(b[2]),
                        a[3].max(b[3]),
                    ];
                } else {
                    out.cover.push((layer, b));
                }
            }
        }
        Ok(out)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_drag_undoes_as_one_step_including_optional_paint_layers() {
        let (folder, mut doc, bp) = super::super::tests::fixture();
        let before = doc.map.samples().to_vec();
        doc.begin_stroke();
        for x in [400.0, 424.0, 448.0] {
            doc.apply(
                &[
                    Command::Terrain {
                        at: [x, 400.0],
                        radius: 64.0,
                        strength: 8.0,
                        brush: Brush::Raise,
                    },
                    Command::Paint {
                        at: [x, 400.0],
                        radius: 64.0,
                        paint: Paint::Snow,
                        value: 255,
                        heading: 0.0,
                    },
                ],
                &bp,
            )
            .unwrap();
        }
        assert!(doc.end_stroke());
        assert!(!doc.stroking());
        let after = doc.map.samples().to_vec();
        assert_ne!(before, after);
        assert!(!doc.map.snow.is_empty());
        let update = doc.undo().unwrap();
        assert!(update.terrain.is_some());
        assert_eq!(update.cover.len(), 1);
        assert_eq!(doc.map.samples(), before);
        assert!(doc.map.snow.is_empty());
        assert!(!doc.can_undo());
        doc.redo().unwrap();
        assert_eq!(doc.map.samples(), after);
        assert!(!doc.map.snow.is_empty());
        std::fs::remove_dir_all(folder).unwrap();
    }
}
