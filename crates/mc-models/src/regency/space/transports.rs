//! Protected cargo chambers in swept hulls. The full boarding lane stays empty.
use super::super::kit::{dark_plate, metal, seam, v3};
use super::hull::{coarse_lobes, crest, drive, lobe, mark};
use crate::builder::MeshBuilder;
use crate::material::GLOW_VIOLET;

#[derive(Clone, Copy)]
struct Hold {
    hinge: f32,
    lip: f32,
    front: f32,
    half: f32,
    clear: f32,
    roof: f32,
}

pub(super) fn light(b: &mut MeshBuilder, _tech: u8) {
    let hold = Hold {
        hinge: -28.0,
        lip: -40.0,
        front: 23.0,
        half: 13.0,
        clear: 29.0,
        roof: 32.0,
    };
    if b.coarse() {
        coarse_lobes(
            b,
            &[[-39.0, 16.0], [-15.0, 36.0], [36.0, 22.0], [22.0, 16.0]],
            2.0,
            26.0,
        );
        chamber(b, hold);
        crest(b, 13.0, 0.0, 34.0, 36.0, 23.0);
        mark(b, v3(12.0, 0.0, 36.05), 5.0);
        return;
    }
    chamber(b, hold);
    b.mirror_y(|b| {
        lobe(
            b,
            &[
                (v3(-39.0, 20.0, 11.0), 2.0, 3.0),
                (v3(-24.0, 28.0, 16.0), 8.0, 10.0),
                (v3(0.0, 29.0, 17.0), 9.0, 11.0),
                (v3(21.0, 25.0, 17.0), 8.0, 10.0),
                (v3(36.0, 18.0, 12.0), 3.0, 4.0),
            ],
        );
        drive(b, v3(17.0, 28.0, 1.2), 3.8);
        drive(b, v3(-20.0, 28.0, 1.2), 3.5);
        crest(b, 20.0, 19.0, 26.0, 34.0, 14.0);
    });
    crest(b, 13.0, 0.0, 34.0, 36.0, 23.0);
    mark(b, v3(10.0, 0.0, 34.08), 5.0);
}

fn chamber(b: &mut MeshBuilder, h: Hold) {
    dark_plate(b);
    if b.coarse() {
        // The void reads at the useful LODs; far away one roof is sufficient.
        b.block(v3(h.hinge, -h.half, h.clear), v3(h.front, h.half, h.roof));
        return;
    }
    b.with_facets(|b| {
        b.mirror_y(|b| b.block(v3(h.hinge, h.half, 0.0), v3(h.front, h.half + 3.0, h.roof)));
        b.extrude_x(
            &[
                [-h.half, h.clear],
                [-h.half, h.roof - 1.0],
                [-h.half * 0.65, h.roof + 2.0],
                [h.half * 0.65, h.roof + 2.0],
                [h.half, h.roof - 1.0],
                [h.half, h.clear],
            ],
            h.hinge,
            h.front,
        );
        b.extrude_y(
            &[
                [h.front, 0.0],
                [h.front + 10.0, 8.0],
                [h.front + 7.0, h.roof - 6.0],
                [h.front, h.roof + 2.0],
            ],
            -h.half,
            h.half,
        );
    });
    seam(b);
    b.block(v3(h.hinge, -h.half, 0.0), v3(h.front, h.half, 1.0));
    // The ramp follows the transport's deploy value through the existing RAMP part.
    b.with_part(crate::part::RAMP, |b| {
        metal(b);
        b.extrude_y(
            &[[h.lip, 0.0], [h.hinge, 1.0], [h.hinge, 1.35], [h.lip, 0.35]],
            -h.half,
            h.half,
        );
        if b.fine() {
            for y in [-h.half + 0.6, h.half - 0.6] {
                b.paint(GLOW_VIOLET);
                b.extrude_y(
                    &[[h.lip, 0.36], [h.hinge, 1.36], [h.hinge, 1.5], [h.lip, 0.5]],
                    y - 0.14,
                    y + 0.14,
                );
            }
        }
    });
    if b.fine() {
        b.mirror_y(|b| {
            for x in [h.hinge + 4.0, h.hinge + 18.0, h.front - 4.0] {
                metal(b);
                b.block(v3(x, h.half, 1.0), v3(x + 0.6, h.half + 0.5, h.clear));
                b.paint(GLOW_VIOLET);
                b.block(
                    v3(x, h.half - 0.08, 3.0),
                    v3(x + 0.4, h.half, h.clear - 3.0),
                );
            }
        });
    }
}
