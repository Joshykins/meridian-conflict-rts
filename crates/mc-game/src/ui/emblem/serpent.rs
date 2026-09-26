//! The Naga's marks as ARC knows them: a hooded serpent in black hide, a
//! slit-pupilled eye burning red in the hood, ringed by a broken horizon.
//! Design space as the eagle's: 1000 units square, x = 500 the axis.

use super::paint::{circle, hex, linear, radial, Canvas};
use glam::Vec2;
use std::f32::consts::{FRAC_PI_2, TAU};
use tiny_skia::{Path, PathBuilder};

const AXIS: f32 = 500.0;
const HIDE: u32 = 0x0E0909;
const HIDE_HI: u32 = 0x2B1515;
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

/// The hood: a broad spade rising from a narrow neck to a small head.
fn hood(k: f32) -> Option<Path> {
    let p = |x: f32, y: f32| Vec2::new(AXIS + (x - AXIS) * k, 520.0 + (y - 520.0) * k);
    let mut pb = PathBuilder::new();
    let s = p(500.0, 930.0);
    pb.move_to(s.x, s.y);
    for side in [1.0f32, -1.0] {
        let q = |x: f32, y: f32| p(AXIS + (x - AXIS) * side, y);
        let pts = if side > 0.0 {
            [q(560.0, 860.0), q(610.0, 760.0), q(760.0, 600.0), q(790.0, 470.0), q(800.0, 330.0), q(640.0, 250.0), q(560.0, 200.0), q(560.0, 120.0), q(500.0, 96.0)]
        } else {
            [q(560.0, 120.0), q(560.0, 200.0), q(640.0, 250.0), q(800.0, 330.0), q(790.0, 470.0), q(760.0, 600.0), q(610.0, 760.0), q(560.0, 860.0), q(500.0, 930.0)]
        };
        for c in pts.chunks(3) {
            pb.cubic_to(c[0].x, c[0].y, c[1].x, c[1].y, c[2].x, c[2].y);
        }
    }
    pb.close();
    pb.finish()
}

pub fn crest(size: [usize; 2]) -> Vec<u8> {
    let mut c = Canvas::new(size, Vec2::new(1000.0, 1000.0));
    let centre = Vec2::new(AXIS, 520.0);
    // The broken horizon: a burning ring, open at the top where the hood rises.
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
    // The hood, rim-lit red.
    let body = hood(1.0);
    c.stroke(&body, 26.0, 1.0, hex(RED_DEEP, 1.0));
    c.fill_with(&body, radial(Vec2::new(AXIS, 470.0), 420.0, &[(0.0, hex(HIDE_HI, 1.0)), (1.0, hex(HIDE, 1.0))]), None);
    c.stroke(&body, 6.0, 1.0, hex(RED, 1.0));
    // Scales: rows of small crescents down the hood.
    let clip = c.mask(&hood(0.94));
    for row in 0..9 {
        let y = 300.0 + row as f32 * 60.0;
        for col in -6..=6 {
            let x = AXIS + col as f32 * 64.0 + if row % 2 == 0 { 0.0 } else { 32.0 };
            let mut pb = PathBuilder::new();
            pb.move_to(x - 26.0, y);
            pb.quad_to(x, y + 30.0, x + 26.0, y);
            c.stroke_with(&pb.finish(), 3.0, 0.0, tiny_skia::Shader::SolidColor(hex(RED_DEEP, 0.6)), clip.as_ref());
        }
    }
    // Ribs down the hood, following its edge in toward the neck.
    for side in [1.0f32, -1.0] {
        let q = |x: f32, y: f32| Vec2::new(AXIS + x * side, y);
        let mut pb = PathBuilder::new();
        let (a, b, e) = (q(150.0, 250.0), q(250.0, 460.0), q(70.0, 820.0));
        let s0 = q(70.0, 190.0);
        pb.move_to(s0.x, s0.y);
        pb.cubic_to(a.x, a.y, b.x, b.y, e.x, e.y);
        c.stroke_with(&pb.finish(), 7.0, 1.0, tiny_skia::Shader::SolidColor(hex(RED, 0.7)), clip.as_ref());
    }
    // The eye.
    let eye = Vec2::new(AXIS, 470.0);
    c.fill_with(&circle(eye, 260.0), radial(eye, 260.0, &[(0.0, hex(RED, 0.5)), (1.0, hex(RED, 0.0))]), None);
    c.fill_with(&lens(eye, 200.0, 62.0), linear(eye - Vec2::Y * 62.0, eye + Vec2::Y * 62.0, &[(0.0, hex(EMBER, 1.0)), (0.5, hex(RED, 1.0)), (1.0, hex(RED_DEEP, 1.0))]), None);
    c.stroke(&lens(eye, 200.0, 62.0), 8.0, 1.0, hex(HIDE, 1.0));
    c.fill(&pupil(eye, 22.0, 104.0), hex(0x050202, 1.0));
    c.into_rgba()
}

/// The eye alone in white, for tinting and small sizes.
pub fn badge(size: [usize; 2]) -> Vec<u8> {
    let mut c = Canvas::new(size, Vec2::new(1000.0, 1000.0));
    let eye = Vec2::splat(500.0);
    c.stroke(&lens(eye, 440.0, 150.0), 90.0, 1.0, hex(0xFFFFFF, 1.0));
    c.fill(&pupil(eye, 70.0, 300.0), hex(0xFFFFFF, 1.0));
    c.into_rgba()
}
