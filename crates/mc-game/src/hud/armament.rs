//! A blueprint's guns on the build card: what it puts out against each domain,
//! then one row per kind of gun, identical mounts folded into one ("Battery \u{d7}3"),
//! heaviest first, with the figures in columns under a single header.

use super::selection::{weapon_dps, weapon_tone};
use super::style::{AIR, LAND, NAVY};
use super::whole;
use crate::ui::{palette, rgb, type_scale, Rect, Ui};
use mc_data::{cat, Trajectory, Weapon};

/// Guns that differ only in where they sit and what they are called.
pub struct Group<'a> {
    pub weapon: &'a Weapon,
    pub count: usize,
    pub name: String,
    /// Damage per second of all of them together.
    pub dps: f32,
}

pub(super) fn same(a: &Weapon, b: &Weapon) -> bool {
    a.damage == b.damage
        && a.splash == b.splash
        && a.range_min == b.range_min
        && a.range_max == b.range_max
        && a.reload_ticks == b.reload_ticks
        && a.salvo == b.salvo
        && a.salvo_batch == b.salvo_batch
        && a.projectile_speed == b.projectile_speed
        && a.trajectory == b.trajectory
        && a.target_mask == b.target_mask
        && a.missile == b.missile
        && a.guided == b.guided
        && a.torpedo == b.torpedo
        && a.intercepts == b.intercepts
}

/// What a set of mounts has in common by name: "Fore Battery" and "Aft Battery" are
/// a "Battery". The words they share at the end, else at the start, else the first name.
pub(super) fn shared_name(names: &[&str]) -> String {
    if let [one] = names {
        return (*one).to_owned();
    }
    let words: Vec<Vec<&str>> = names.iter().map(|n| n.split_whitespace().collect()).collect();
    let shortest = words.iter().map(Vec::len).min().unwrap_or(0);
    let tail = (0..shortest)
        .take_while(|&i| words.iter().all(|w| w[w.len() - 1 - i] == words[0][words[0].len() - 1 - i]))
        .count();
    if tail > 0 {
        return words[0][words[0].len() - tail..].join(" ");
    }
    let head = (0..shortest).take_while(|&i| words.iter().all(|w| w[i] == words[0][i])).count();
    if head > 0 {
        return words[0][..head].join(" ");
    }
    names[0].to_owned()
}

/// The weapons folded into groups, most damage per second first.
pub fn groups(weapons: &[Weapon]) -> Vec<Group<'_>> {
    let mut sets: Vec<Vec<&Weapon>> = Vec::new();
    for w in weapons {
        match sets.iter_mut().find(|s| same(s[0], w)) {
            Some(s) => s.push(w),
            None => sets.push(vec![w]),
        }
    }
    let mut out: Vec<Group> = sets
        .into_iter()
        .map(|s| {
            let names: Vec<&str> = s.iter().map(|w| w.name.as_str()).collect();
            Group {
                weapon: s[0],
                count: s.len(),
                name: shared_name(&names),
                dps: weapon_dps(s[0]) * s.len() as f32,
            }
        })
        .collect();
    out.sort_by(|a, b| b.dps.total_cmp(&a.dps));
    out
}

const HEAD_H: f32 = 22.0;
const SUMMARY_H: f32 = 44.0;
const COLUMNS_H: f32 = 18.0;
const ROW_H: f32 = 40.0;

/// How tall `draw` is for these groups.
pub fn height(groups: &[Group]) -> f32 {
    if groups.is_empty() {
        return 0.0;
    }
    HEAD_H + SUMMARY_H + COLUMNS_H + groups.len() as f32 * ROW_H
}

fn kind(w: &Weapon) -> &'static str {
    if w.intercepts {
        "Interceptor"
    } else if w.torpedo {
        "Torpedo"
    } else if w.bore.is_some() {
        "Bore"
    } else if w.missile && w.guided {
        "Guided"
    } else if w.missile {
        "Rocket"
    } else if w.hitscan {
        "Beam"
    } else if w.rail {
        "Rail"
    } else if w.trajectory == Trajectory::Ballistic {
        "Ballistic"
    } else {
        "Direct"
    }
}

/// The domains a gun can shoot at, in their colours.
const DOMAINS: [(u32, &str, u32); 3] = [
    (cat::LAND | cat::STRUCTURE, "Land", LAND),
    (cat::NAVAL, "Naval", NAVY),
    (cat::AIR, "Air", AIR),
];

/// Where the figures on a row's second line start, off the row's text edge, and their headings.
const COLUMNS: [(f32, &str); 5] = [
    (0.0, "Per Shot"),
    (84.0, "Range"),
    (170.0, "Reload"),
    (226.0, "Splash"),
    (284.0, "Speed"),
];

/// The armament, from `y` down across `cw`. Returns the y below it.
pub fn draw(ui: &mut Ui, groups: &[Group], x: f32, y: f32, cw: f32) -> f32 {
    if groups.is_empty() {
        return y;
    }
    // Interceptors shoot torpedoes, not units: they are no part of the firepower.
    let hits = |g: &&Group| !g.weapon.intercepts;
    let total: f32 = groups.iter().filter(hits).map(|g| g.dps).sum();
    let mut y = y + 8.0;
    ui.section(x, y, cw - 80.0, "Armament");
    ui.text_right(x + cw, y, type_scale::VALUE, rgb(0xFFFFFF, 1.0), &whole(total));
    let tw = ui.text_width(type_scale::VALUE, &whole(total));
    ui.text_right(x + cw - tw - 5.0, y + 1.0, type_scale::MICRO, rgb(palette::FAINT, 1.0), "Total DPS");
    y += HEAD_H - 8.0;

    // Firepower against each domain, and the longest reach.
    let mut blocks: Vec<(String, String, u32)> = DOMAINS
        .iter()
        .filter_map(|&(mask, label, tone)| {
            let d: f32 = groups.iter().filter(hits).filter(|g| g.weapon.target_mask & mask != 0).map(|g| g.dps).sum();
            (d > 0.0).then(|| (format!("vs {label}"), whole(d), tone))
        })
        .collect();
    let reach = groups.iter().map(|g| g.weapon.range_max.to_f32()).fold(0.0, f32::max);
    blocks.push(("Reach".into(), format!("{} m", whole(reach)), palette::TEXT));
    let step = cw / 4.0;
    for (i, (label, value, tone)) in blocks.iter().enumerate() {
        let bx = x + i as f32 * step;
        ui.fill(Rect::new(bx, y + 4.0, 2.0, 28.0), rgb(*tone, 0.9));
        ui.text(bx + 10.0, y + 10.0, type_scale::MICRO, rgb(palette::FAINT, 1.0), label);
        ui.text(bx + 10.0, y + 26.0, type_scale::VALUE, rgb(*tone, 1.0), value);
    }
    y += SUMMARY_H;

    // Column headings, once for every row.
    let tx = x + 10.0;
    ui.fill(Rect::new(x, y - 2.0, cw, COLUMNS_H), rgb(palette::LINE, 0.04));
    for (dx, label) in COLUMNS {
        ui.text(tx + dx, y + 7.0, type_scale::MICRO, rgb(palette::FAINT, 1.0), label);
    }
    ui.text_right(x + cw - 4.0, y + 7.0, type_scale::MICRO, rgb(palette::FAINT, 1.0), "DPS");
    y += COLUMNS_H + 4.0;

    let most = groups.iter().map(|g| g.dps).fold(0.0, f32::max).max(1e-3);
    for g in groups {
        let w = g.weapon;
        let tone = weapon_tone(w);
        ui.fill(Rect::new(x, y - 6.0, 3.0, ROW_H - 10.0), rgb(tone, 1.0));

        // First line: the gun, how many, what it is and what it hits; its share of the firepower right.
        let dps = if w.intercepts { "\u{2014}".to_owned() } else { whole(g.dps) };
        let dps_w = ui.text_width(type_scale::VALUE, &dps);
        ui.text_right(x + cw - 4.0, y + 1.0, type_scale::VALUE, rgb(0xFFFFFF, 1.0), &dps);
        let mut chips: Vec<(&str, u32)> = vec![(kind(w), tone)];
        if w.intercepts {
            chips.push(("Torpedoes", palette::DIM));
        }
        for (mask, label, c) in DOMAINS {
            if w.target_mask & mask != 0 && !w.intercepts {
                chips.push((label, c));
            }
        }
        let chips_w: f32 = chips.iter().map(|(l, _)| ui.text_width(type_scale::MICRO, l) + 14.0).sum();
        let count = (g.count > 1).then(|| format!("\u{d7}{}", g.count));
        let count_w = count.as_ref().map_or(0.0, |c| ui.text_width(type_scale::CAPTION, c) + 6.0);
        let room = (cw - 10.0 - dps_w - 16.0 - chips_w - count_w).max(40.0);
        ui.text_fit_left(tx, y + 1.0, room, type_scale::CAPTION, rgb(palette::TEXT, 1.0), &g.name);
        let mut cx = tx + ui.text_width(type_scale::CAPTION, &g.name).min(room) + 6.0;
        if let Some(c) = &count {
            cx = ui.text(cx, y + 1.0, type_scale::CAPTION, rgb(tone, 1.0), c) + 8.0;
        }
        for (label, c) in chips {
            let lw = ui.text_width(type_scale::MICRO, label) + 10.0;
            let chip = Rect::new(cx, y - 6.0, lw, 14.0);
            ui.fill(chip, rgb(c, 0.16));
            ui.text(chip.x + 5.0, chip.mid_y(), type_scale::MICRO, rgb(c, 1.0), label);
            cx += lw + 4.0;
        }

        // Second line: the figures, under their headings.
        let shots = w.salvo.max(1) as u32 * w.salvo_batch.max(1) as u32;
        let dash = || "\u{2014}".to_owned();
        let figures = [
            if shots > 1 {
                format!("{} \u{d7} {shots}", whole(w.damage.to_f32()))
            } else {
                whole(w.damage.to_f32())
            },
            if w.range_min.to_f32() > 0.0 {
                format!("{:.0}\u{2013}{:.0} m", w.range_min.to_f32(), w.range_max.to_f32())
            } else {
                format!("{:.0} m", w.range_max.to_f32())
            },
            format!("{:.1} s", w.reload_ticks as f32 * 0.1),
            if w.splash.to_f32() > 0.0 { format!("{:.0} m", w.splash.to_f32()) } else { dash() },
            if w.projectile_speed.to_f32() > 0.0 && !w.hitscan {
                format!("{:.0} m/s", w.projectile_speed.to_f32())
            } else {
                dash()
            },
        ];
        for ((dx, _), value) in COLUMNS.iter().zip(&figures) {
            let faint = value == "\u{2014}";
            ui.text(
                tx + dx,
                y + 18.0,
                type_scale::MICRO,
                rgb(if faint { palette::FAINT } else { palette::DIM }, 1.0),
                value,
            );
        }
        // Its share of the unit's damage, as a bar along the row's foot.
        let track = Rect::new(tx, y + 26.0, cw - 14.0, 2.0);
        ui.fill(track, rgb(tone, 0.12));
        if !w.intercepts {
            ui.fill(Rect::new(track.x, track.y, track.w * (g.dps / most).clamp(0.02, 1.0), track.h), rgb(tone, 0.8));
        }
        y += ROW_H;
    }
    y
}

#[cfg(test)]
mod tests {
    use super::shared_name;

    #[test]
    fn mounts_share_a_name() {
        assert_eq!(shared_name(&["Fore Battery", "Second Battery", "Aft Battery"]), "Battery");
        assert_eq!(
            shared_name(&["Port Fore Secondary", "Port Aft Secondary", "Starboard Aft Secondary"]),
            "Secondary"
        );
        assert_eq!(shared_name(&["Rail Left", "Rail Right"]), "Rail");
        assert_eq!(shared_name(&["Cannon", "Gun"]), "Cannon");
        assert_eq!(shared_name(&["Light AA"]), "Light AA");
    }
}
