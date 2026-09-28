//! Plain-English differences between our render and a reference, ranked by
//! how much you would hear them. Mix differences come from the sound
//! measurements (loudness-matched where it matters); musical differences come
//! from the lead sheets, in numerals and degrees so a transposition is not
//! mistaken for a different song.

use crate::analyse::Report;
use crate::leadsheet::{progression, LeadSheet, SheetSection};
use crate::sound::WIDTH_BANDS;
use crate::theory;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Finding {
    /// Bigger = more audible / more important.
    pub score: f32,
    /// "mix", "tempo", "harmony", "bass", "drums", "melody", "form", "space".
    pub topic: String,
    pub text: String,
}

/// The differences as sentences, most audible first.
pub fn compare(ours: &Report, reference: &Report) -> Vec<String> {
    compare_ranked(ours, reference)
        .into_iter()
        .map(|f| f.text)
        .collect()
}

/// Tonal-balance bands for the comparison: (name, low, high).
const TONAL: [(&str, f32, f32); 8] = [
    ("sub 20-60 Hz", 20.0, 60.0),
    ("bass 60-150 Hz", 60.0, 150.0),
    ("low mids 150-400 Hz", 150.0, 400.0),
    ("mids 400 Hz-1 kHz", 400.0, 1000.0),
    ("upper mids 1-2 kHz", 1000.0, 2000.0),
    ("presence 2-5 kHz", 2000.0, 5000.0),
    ("brilliance 5-10 kHz", 5000.0, 10000.0),
    ("air 10-16 kHz", 10000.0, 16500.0),
];
const TONAL_WEIGHT: [f32; 8] = [0.8, 1.0, 1.0, 1.1, 1.2, 1.2, 1.0, 0.7];

fn band_db(spec: &[(f32, f32)], lo: f32, hi: f32) -> Option<f32> {
    let p: f32 = spec
        .iter()
        .filter(|(hz, _)| *hz >= lo && *hz < hi)
        .map(|(_, db)| 10f32.powf(db / 10.0))
        .sum();
    if p > 0.0 {
        Some(10.0 * p.log10())
    } else {
        None
    }
}

pub fn compare_ranked(ours: &Report, reference: &Report) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut add = |score: f32, topic: &str, text: String| {
        out.push(Finding {
            score,
            topic: topic.into(),
            text,
        })
    };
    let (o, r) = (&ours.sound, &reference.sound);

    // Loudness and dynamics.
    let dl = r.lufs - o.lufs;
    if dl.abs() >= 1.0 {
        add(
            dl.abs() * 1.5,
            "mix",
            format!(
                "reference is {:.1} LU {} ({:.1} vs {:.1} LUFS integrated)",
                dl.abs(),
                if dl > 0.0 { "louder" } else { "quieter" },
                r.lufs,
                o.lufs
            ),
        );
    }
    let dc = r.crest_db - o.crest_db;
    if dc.abs() >= 2.5 {
        add(
            dc.abs() * 0.9,
            "mix",
            format!(
                "reference crest factor is {:.1} dB {} ({:.1} vs {:.1} dB): {}",
                dc.abs(),
                if dc > 0.0 { "higher" } else { "lower" },
                r.crest_db,
                o.crest_db,
                if dc > 0.0 {
                    "ours is more squashed"
                } else {
                    "the reference is more compressed/limited"
                }
            ),
        );
    }
    let dr = r.lra - o.lra;
    if dr.abs() >= 2.0 {
        add(
            dr.abs() * 0.7,
            "mix",
            format!(
                "reference loudness range is {:.1} LU vs ours {:.1} LU ({})",
                r.lra,
                o.lra,
                if dr > 0.0 {
                    "it breathes more between sections"
                } else {
                    "ours moves more between sections"
                }
            ),
        );
    }
    // Tonal balance at matched loudness.
    for (i, &(name, lo, hi)) in TONAL.iter().enumerate() {
        if let (Some(a), Some(b)) = (
            band_db(&r.third_octave, lo, hi),
            band_db(&o.third_octave, lo, hi),
        ) {
            let d = (a - r.lufs) - (b - o.lufs);
            if d.abs() >= 2.0 {
                add(
                    d.abs() * TONAL_WEIGHT[i],
                    "mix",
                    format!(
                        "reference has {:.1} dB {} {} at matched loudness",
                        d.abs(),
                        if d > 0.0 { "more" } else { "less" },
                        name
                    ),
                );
            }
        }
    }
    // Stereo.
    for (k, &(name, _, _)) in WIDTH_BANDS.iter().enumerate() {
        let d = r.width[k] - o.width[k];
        if d.abs() >= 0.15 {
            add(
                d.abs() * 8.0 * if k == 0 { 1.3 } else { 1.0 },
                "mix",
                format!(
                    "reference {} is {} (side/mid {:.2} vs ours {:.2})",
                    name,
                    if d > 0.0 { "wider" } else { "narrower" },
                    r.width[k],
                    o.width[k]
                ),
            );
        }
    }
    if (r.mono_below_hz - o.mono_below_hz).abs() >= 40.0 {
        let say = |hz: f32, w: f32| {
            if hz > 0.0 {
                format!("mono below {hz:.0} Hz")
            } else {
                format!("not mono in the low end ({w:.2} wide below 120 Hz)")
            }
        };
        add(
            4.0,
            "mix",
            format!(
                "reference is {}, ours is {}",
                say(r.mono_below_hz, r.low_width),
                say(o.mono_below_hz, o.low_width)
            ),
        );
    }
    let db = (r.centroid_hz / o.centroid_hz.max(1.0)).log2();
    if db.abs() >= 0.2 {
        add(
            db.abs() * 12.0,
            "mix",
            format!(
                "reference is {} (spectral centroid {:.0} Hz vs {:.0} Hz)",
                if db > 0.0 { "brighter" } else { "darker" },
                r.centroid_hz,
                o.centroid_hz
            ),
        );
    }
    // Space and motion.
    if let (Some(a), Some(b)) = (r.decay_rt60, o.decay_rt60) {
        if (a / b).log2().abs() >= 0.4 {
            add(
                (a / b).log2().abs() * 5.0,
                "space",
                format!(
                    "reference decays {} after hits (RT60 ~{a:.2} s vs {b:.2} s, rough)",
                    if a > b {
                        "longer: more reverb/release"
                    } else {
                        "shorter: drier"
                    }
                ),
            );
        }
    }
    let dt = r.onsets_per_s - o.onsets_per_s;
    if dt.abs() >= 1.0 && dt.abs() / o.onsets_per_s.max(0.5) >= 0.25 {
        add(
            dt.abs() * 1.2,
            "drums",
            format!(
                "reference is {} busy: {:.1} onsets/s vs {:.1}",
                if dt > 0.0 { "more" } else { "less" },
                r.onsets_per_s,
                o.onsets_per_s
            ),
        );
    }
    match (r.pump.detected, o.pump.detected) {
        (true, false) => add(6.0 + r.pump.depth_db, "mix", format!("reference pumps to the kick ({:.1} dB dip, back in {:.0} ms); ours does not ({:.1} dB)", r.pump.depth_db, r.pump.recovery_ms, o.pump.depth_db)),
        (false, true) => add(6.0 + o.pump.depth_db, "mix", format!("ours pumps to the kick ({:.1} dB) and the reference does not ({:.1} dB)", o.pump.depth_db, r.pump.depth_db)),
        (true, true) if (r.pump.depth_db - o.pump.depth_db).abs() >= 2.0 => add((r.pump.depth_db - o.pump.depth_db).abs(), "mix", format!("reference pump is {:.1} dB deep, ours {:.1} dB", r.pump.depth_db, o.pump.depth_db)),
        _ => {}
    }
    for f in compare_charts(&ours.leadsheet, &reference.leadsheet) {
        out.push(f);
    }
    // Tempo from the charts may be exact on our side; the audio tempo is the fallback.
    out.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    out
}

fn grid_mask(s: &str) -> String {
    s.chars()
        .map(|c| if c == '.' || c == '-' { '.' } else { 'x' })
        .collect()
}

/// The primary section: the label that covers the most bars.
fn primary(s: &LeadSheet) -> Option<&SheetSection> {
    let mut best: Option<(&str, usize)> = None;
    for sec in &s.sections {
        let n: usize = s
            .sections
            .iter()
            .filter(|x| x.label == sec.label)
            .map(|x| x.bars)
            .sum();
        if best.map(|b| n > b.1).unwrap_or(true) {
            best = Some((&sec.label, n));
        }
    }
    let label = best?.0;
    s.sections.iter().find(|x| x.label == label)
}

/// Chart-level differences: tempo, key/mode, progression, harmonic rhythm,
/// bass rhythm and degrees, drum grid, melody, form.
pub fn compare_charts(ours: &LeadSheet, reference: &LeadSheet) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut add = |score: f32, topic: &str, text: String| {
        out.push(Finding {
            score,
            topic: topic.into(),
            text,
        })
    };
    let dt = reference.tempo - ours.tempo;
    if dt.abs() >= 1.0 {
        let ratio = reference.tempo / ours.tempo.max(1.0);
        let note = if (ratio - 2.0).abs() < 0.05 || (ratio - 0.5).abs() < 0.03 {
            " (a half/double-time reading: check by ear)"
        } else {
            ""
        };
        add(
            8.0 + dt.abs().min(20.0) * 0.3,
            "tempo",
            format!(
                "reference is {:.1} bpm, ours {:.1} ({:+.1}){}",
                reference.tempo, ours.tempo, dt, note
            ),
        );
    }
    if let (Some(rk), Some(ok)) = (reference.key, ours.key) {
        if rk.minor != ok.minor {
            add(
                9.0,
                "harmony",
                format!(
                    "reference is in a {} key ({}), ours {} ({})",
                    if rk.minor { "minor" } else { "major" },
                    rk.name(),
                    if ok.minor { "minor" } else { "major" },
                    ok.name()
                ),
            );
        } else if rk.root != ok.root {
            add(1.5, "harmony", format!("transposed: reference in {}, ours in {} (numerals and degrees below are comparable)", rk.name(), ok.name()));
        }
    }
    let (Some(rs), Some(os)) = (primary(reference), primary(ours)) else {
        return out;
    };
    let where_ = format!(
        "main section: ref {} / ours {}",
        rs.label,
        if os.name != os.label {
            format!("{} \"{}\"", os.label, os.name)
        } else {
            os.label.clone()
        }
    );
    let rp = progression(&rs.numerals);
    let op = progression(&os.numerals);
    if rp != op {
        let rset: std::collections::BTreeSet<&String> = rs.numerals.iter().flatten().collect();
        let oset: std::collections::BTreeSet<&String> = os.numerals.iter().flatten().collect();
        let shared = rset.intersection(&oset).count() as f32 / rset.len().max(1) as f32;
        add(
            5.0 + 3.0 * (1.0 - shared),
            "harmony",
            format!("progression ({where_}): reference {rp}; ours {op}"),
        );
    }
    let dh = rs.harmonic_rhythm - os.harmonic_rhythm;
    if dh.abs() >= 0.35 {
        add(
            4.0 + dh.abs() * 2.0,
            "harmony",
            format!(
                "harmonic rhythm: reference changes chord {:.2} times a bar, ours {:.2}",
                rs.harmonic_rhythm, os.harmonic_rhythm
            ),
        );
    }
    // Bass.
    let (rb, ob) = (grid_mask(&rs.bass_rhythm), grid_mask(&os.bass_rhythm));
    if rb.contains('x') || ob.contains('x') {
        if rb != ob {
            let diff = rb.chars().zip(ob.chars()).filter(|(a, b)| a != b).count() as f32
                / rb.len().max(1) as f32;
            add(
                4.0 + 8.0 * diff,
                "bass",
                format!(
                    "bass rhythm: reference plays {} ({}); ours {} ({})",
                    theory::hits_in_words(&rb),
                    rs.bass_rhythm,
                    theory::hits_in_words(&ob),
                    os.bass_rhythm
                ),
            );
        }
        if rs.bass_degrees != os.bass_degrees && !rs.bass_degrees.is_empty() {
            add(
                4.5,
                "bass",
                format!(
                    "bass degrees in its common bar: reference {} ; ours {}",
                    rs.bass_degrees, os.bass_degrees
                ),
            );
        }
        let dn = rs.bass_notes_per_bar - os.bass_notes_per_bar;
        if dn.abs() >= 1.5 {
            add(
                3.0 + dn.abs() * 0.5,
                "bass",
                format!(
                    "bass density: reference {:.1} notes a bar, ours {:.1}",
                    rs.bass_notes_per_bar, os.bass_notes_per_bar
                ),
            );
        }
    }
    // Drums.
    let weights = [8.0f32, 7.0, 5.0];
    for (k, name) in crate::drums::LANES.iter().enumerate() {
        let (a, b) = (grid_mask(&rs.drums[k]), grid_mask(&os.drums[k]));
        if a != b {
            let diff = a.chars().zip(b.chars()).filter(|(x, y)| x != y).count() as f32
                / a.len().max(1) as f32;
            add(
                weights[k] * (0.5 + diff * 2.0).min(1.2),
                "drums",
                match (a.contains('x'), b.contains('x')) {
                    (false, _) => format!(
                        "reference has no {name} in its main bar; ours plays it on {} ({})",
                        theory::hits_in_words(&b),
                        os.drums[k]
                    ),
                    (_, false) => format!(
                        "reference {name} is on {} ({}); ours has none in its main bar",
                        theory::hits_in_words(&a),
                        rs.drums[k]
                    ),
                    _ => format!(
                        "reference {name} is on {}; ours on {} (ref {} / ours {})",
                        theory::hits_in_words(&a),
                        theory::hits_in_words(&b),
                        rs.drums[k],
                        os.drums[k]
                    ),
                },
            );
        }
    }
    // Melody.
    if let (Some(rm), Some(om)) = (&rs.melody, &os.melody) {
        if rm.confident
            && (rm.contour != om.contour
                || rm.degrees.split(' ').next() != om.degrees.split(' ').next())
        {
            add(
                3.0,
                "melody",
                format!(
                    "melody: reference {} (range {}..{}); ours {} (range {}..{})",
                    rm.contour, rm.low, rm.high, om.contour, om.low, om.high
                ),
            );
        }
    }
    // Form.
    if reference.form != ours.form && !reference.form.is_empty() {
        add(
            2.5,
            "form",
            format!("form: reference {}; ours {}", reference.form, ours.form),
        );
    }
    out
}
