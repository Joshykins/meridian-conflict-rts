//! The Regency's marks as ARC draws them: a crown of backswept plates over
//! nothing, and in the nothing the red eye every Regency machine carries,
//! ringed by a broken, burning horizon. A regent rules in someone's place;
//! Command's artists drew the place. Design space as the eagle's: 1000 units
//! square, x = 500 the axis.

use super::paint::{circle, hex, linear, poly, radial, Canvas};
use glam::Vec2;
use std::f32::consts::{FRAC_PI_2, TAU};
use tiny_skia::{Path, PathBuilder};

const AXIS: f32 = 500.0;
const STEEL: u32 = 0x2A2E35;
const STEEL_DEEP: u32 = 0x0B0C0F;
const BRONZE: u32 = 0x9A6A36;
const VOID: u32 = 0x030203;
const RED: u32 = 0xFF1A1F;
const RED_DEEP: u32 = 0x6E0507;
const EMBER: u32 = 0xFF7A48;

/// The eye's lens, `w` by `h`, as one closed curve.
fn lens(c: Vec2, w: f32, h: f32) -> Option<Path> {
    let mut pb = PathBuilder::new();
    pb.move_to(c.x - w, c.y);
    pb.quad_to(c.x, c.y - h * 2.0, c.x + w, c.y);
    pb.quad_to(c.x, c.y + h * 2.0, c.x - w, c.y);
    pb.close();
    pb.finish()
}

fn pupil(c: Vec2, w: f32, h: f32) -> Option<Path> {
    let mut pb = PathBuilder::new();
    pb.move_to(c.x, c.y - h);
    pb.quad_to(c.x + w, c.y, c.x, c.y + h);
    pb.quad_to(c.x - w, c.y, c.x, c.y - h);
    pb.close();
    pb.finish()
}

/// Both sides of a right-hand outline, mirrored about the axis.
fn mirrored(right: &[(f32, f32)]) -> [Vec<Vec2>; 2] {
    [1.0f32, -1.0].map(|side| {
        right
            .iter()
            .map(|&(x, y)| Vec2::new(AXIS + (x - AXIS) * side, y))
            .collect()
    })
}

/// The crown's plates, back to front: two swept tines a side, the tall centre
/// tine, then the band they are all bolted to.
fn plates() -> Vec<Vec<Vec2>> {
    let mut out = Vec::new();
    out.extend(mirrored(&[
        (630.0, 390.0),
        (700.0, 300.0),
        (805.0, 245.0),
        (760.0, 340.0),
        (745.0, 390.0),
    ]));
    out.extend(mirrored(&[
        (540.0, 390.0),
        (585.0, 270.0),
        (655.0, 190.0),
        (642.0, 300.0),
        (628.0, 390.0),
    ]));
    out.push(
        [
            (452.0, 392.0),
            (470.0, 250.0),
            (500.0, 135.0),
            (530.0, 250.0),
            (548.0, 392.0),
        ]
        .map(|(x, y)| Vec2::new(x, y))
        .to_vec(),
    );
    out.push(
        [
            (215.0, 440.0),
            (460.0, 440.0),
            (500.0, 474.0),
            (540.0, 440.0),
            (785.0, 440.0),
            (760.0, 385.0),
            (240.0, 385.0),
        ]
        .map(|(x, y)| Vec2::new(x, y))
        .to_vec(),
    );
    out
}

/// The seat's front edge under the empty place: a bronze-edged chevron.
fn step() -> Vec<Vec2> {
    [
        (320.0, 800.0),
        (500.0, 872.0),
        (680.0, 800.0),
        (648.0, 782.0),
        (500.0, 834.0),
        (352.0, 782.0),
    ]
    .map(|(x, y)| Vec2::new(x, y))
    .to_vec()
}

pub fn crest(size: [usize; 2]) -> Vec<u8> {
    let mut c = Canvas::new(size, Vec2::new(1000.0, 1000.0));
    let centre = Vec2::new(AXIS, 520.0);
    // The broken horizon: a burning ring, open at the top where the crown rises.
    let gap = 0.34;
    let ring: Vec<Vec2> = (0..=90)
        .map(|i| -FRAC_PI_2 + gap + (TAU - 2.0 * gap) * i as f32 / 90.0)
        .map(|a| centre + Vec2::new(a.cos(), a.sin()) * 430.0)
        .collect();
    let mut pb = PathBuilder::new();
    pb.move_to(ring[0].x, ring[0].y);
    for q in &ring[1..] {
        pb.line_to(q.x, q.y);
    }
    let ring = pb.finish();
    c.stroke(&ring, 40.0, 1.0, hex(RED_DEEP, 0.55));
    c.stroke(&ring, 14.0, 1.0, hex(RED, 1.0));
    c.stroke(&ring, 4.0, 0.0, hex(EMBER, 1.0));

    // The empty place under the crown, and the eye in it.
    let eye = Vec2::new(AXIS, 615.0);
    // Darkness with no edge: a place, not an object.
    c.fill_with(
        &circle(eye, 200.0),
        radial(
            eye,
            200.0,
            &[
                (0.0, hex(RED_DEEP, 0.9)),
                (0.45, hex(VOID, 1.0)),
                (0.75, hex(VOID, 0.85)),
                (1.0, hex(VOID, 0.0)),
            ],
        ),
        None,
    );
    c.fill_with(
        &circle(eye, 150.0),
        radial(eye, 150.0, &[(0.0, hex(RED, 0.45)), (1.0, hex(RED, 0.0))]),
        None,
    );
    c.fill_with(
        &lens(eye, 118.0, 34.0),
        linear(
            eye - Vec2::Y * 34.0,
            eye + Vec2::Y * 34.0,
            &[
                (0.0, hex(EMBER, 1.0)),
                (0.5, hex(RED, 1.0)),
                (1.0, hex(RED_DEEP, 1.0)),
            ],
        ),
        None,
    );
    c.stroke(&lens(eye, 118.0, 34.0), 6.0, 1.0, hex(VOID, 1.0));
    c.fill(&pupil(eye, 13.0, 60.0), hex(VOID, 1.0));

    // The crown: dark plates edged in bronze, a red seam lit along the band.
    let metal = || {
        linear(
            Vec2::new(AXIS, 135.0),
            Vec2::new(AXIS, 474.0),
            &[(0.0, hex(STEEL, 1.0)), (1.0, hex(STEEL_DEEP, 1.0))],
        )
    };
    for plate in plates().iter().chain([&step()]) {
        let path = poly(plate);
        c.stroke(&path, 22.0, 1.0, hex(STEEL_DEEP, 1.0));
        c.fill_with(&path, metal(), None);
        c.stroke(&path, 7.0, 1.0, hex(BRONZE, 1.0));
    }
    let seam = poly(&[Vec2::new(262.0, 412.0), Vec2::new(738.0, 412.0)]);
    c.stroke(&seam, 16.0, 1.0, hex(RED_DEEP, 0.8));
    c.stroke(&seam, 5.0, 1.0, hex(RED, 1.0));
    c.into_rgba()
}

/// The crown over the eye, in white, for tinting and small sizes.
pub fn badge(size: [usize; 2]) -> Vec<u8> {
    let mut c = Canvas::new(size, Vec2::new(1000.0, 1000.0));
    // Scaled up about the crown so crown and eye fill the square.
    c.within(Vec2::new(-400.0, -170.0), 1.8, |c| {
        for plate in plates() {
            c.fill(&poly(&plate), hex(0xFFFFFF, 1.0));
        }
    });
    let eye = Vec2::new(AXIS, 780.0);
    c.stroke(&lens(eye, 250.0, 78.0), 60.0, 1.0, hex(0xFFFFFF, 1.0));
    c.fill(&pupil(eye, 40.0, 150.0), hex(0xFFFFFF, 1.0));
    c.into_rgba()
}
