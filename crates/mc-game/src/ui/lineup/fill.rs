//! Fill with AI: empty seats made AI commanders at once, set up before they are
//! seated. The sheet lists kinds of AI, each with its faction, difficulty,
//! doctrine, forces, income and build rate, and how many seats it takes ("six
//! hard aggressive, four easy economic"). The kinds take turns down the seats,
//! so each one's commanders spread through the list rather than bunching at
//! the end; seats no kind takes stay closed.

use super::ai::{config_line, difficulty_at, DIFFICULTIES, DIFFICULTY_LABELS};
use super::settings::Sheet;
use super::{Ask, Catalog, Lineup, Mode, Table};
use crate::audio::Sfx;
use crate::ui::faction::Pick;
use crate::ui::race_picker;
use crate::ui::{id, palette, rgb, type_scale, ButtonKind, Rect, Ui};
use glam::Vec2;
use mc_sim::AiConfig;

/// Most kinds the sheet lists, so their rows fit its panel: Add Kind is off at
/// this many (a cap of the sheet's room, said by the button going dim).
const MOST_KINDS: usize = 6;
/// A kind's row on the sheet, and the room above the rows for the note.
const ROW_PITCH: f32 = 46.0;
const NOTE_H: f32 = 44.0;

/// One kind of AI the empty seats are filled with, and how many it takes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Kind {
    ai: AiConfig,
    race: Pick,
    count: usize,
}

/// The Fill with AI sheet and the kinds it was last set to, kept while the
/// set-up is open so a second fill starts from the first's mix.
pub(super) struct Fill {
    sheet: Sheet,
    kinds: Vec<Kind>,
}

impl Default for Fill {
    fn default() -> Fill {
        Fill {
            sheet: Sheet::default(),
            kinds: vec![Kind::default()],
        }
    }
}

impl Fill {
    /// Opens the sheet over `empty` empty seats. One kind takes them all; a
    /// mix kept from before gives up seats from its last kinds until it fits.
    pub(super) fn open(&mut self, empty: usize) {
        if let [only] = &mut self.kinds[..] {
            only.count = empty;
        }
        fit(&mut self.kinds, empty);
        self.sheet.open();
    }

    pub(super) fn is_open(&self) -> bool {
        self.sheet.is_open()
    }

    /// Seats the kinds take, all told.
    fn total(&self) -> usize {
        self.kinds.iter().map(|k| k.count).sum()
    }

    /// Puts the seats `opened` (in the order they were opened) in the kinds'
    /// settings, each kind taking its count.
    fn dress(&self, lineup: &mut Lineup, opened: &[usize]) {
        let counts: Vec<usize> = self.kinds.iter().map(|k| k.count).collect();
        for (&i, k) in opened.iter().zip(deal(&counts)) {
            let seat = &mut lineup.roster.seats[i];
            seat.ai = self.kinds[k].ai;
            seat.race = self.kinds[k].race;
            if lineup.mode == Mode::Survival {
                seat.team = 0;
            }
        }
    }
}

/// Takes seats from the last kinds until the kinds take no more than `empty`.
fn fit(kinds: &mut [Kind], empty: usize) {
    let mut over = kinds
        .iter()
        .map(|k| k.count)
        .sum::<usize>()
        .saturating_sub(empty);
    for kind in kinds.iter_mut().rev() {
        let cut = over.min(kind.count);
        kind.count -= cut;
        over -= cut;
    }
}

/// The Fill with AI sheet over the screen, when open. `live` is false while an
/// overlay lies over it.
pub(super) fn draw(
    ui: &mut Ui,
    lineup: &mut Lineup,
    catalog: &Catalog,
    table: &Table,
    live: bool,
) -> Option<Ask> {
    let mut fill = std::mem::take(&mut lineup.fill);
    let empty = lineup.roster.seats.iter().filter(|s| !s.open()).count();
    // A seat closed or taken under the open sheet (a lobby's) shrinks the mix with it.
    fit(&mut fill.kinds, empty);
    let size = Vec2::new(
        1240.0,
        76.0 + 64.0 + 72.0 + NOTE_H + ROW_PITCH * (fill.kinds.len() + 1) as f32 + 16.0,
    );
    let total = fill.total();
    let action = match total {
        0 => "Add AI".to_owned(),
        1 => "Add 1 AI".to_owned(),
        n => format!("Add {n} AIs"),
    };
    let host = table.host;
    let mut sheet = std::mem::take(&mut fill.sheet);
    let go = sheet.draw_with(
        ui,
        "Fill with AI",
        size,
        live,
        (&action, host && total > 0),
        |ui, body| kinds(ui, &mut fill.kinds, empty, host, body),
    );
    fill.sheet = sheet;
    let mut ask = None;
    if go {
        let zones = lineup.zones(catalog);
        match lineup
            .roster
            .fill_ai(fill.total(), zones, |j| table.occupied(j))
        {
            Ok(opened) => {
                fill.dress(lineup, &opened);
                fill.sheet.close();
                ui.audio.play(Sfx::Select);
            }
            Err(e) => {
                ui.audio.play(Sfx::Deny);
                ask = Some(Ask::Say(e));
            }
        }
    }
    lineup.fill = fill;
    ask
}

/// The sheet's body: a note on the seats, then a row per kind (its faction,
/// difficulty, settings and how many it takes), then Add Kind.
fn kinds(ui: &mut Ui, kinds: &mut Vec<Kind>, empty: usize, host: bool, r: Rect) {
    let total: usize = kinds.iter().map(|k| k.count).sum();
    let left = empty - total;
    let plural = |n: usize| if n == 1 { "" } else { "s" };
    let note = match (empty, left) {
        (0, _) => "Every seat is taken. Close a seat to fill it with an AI.".to_owned(),
        (n, 0) => format!("{n} empty seat{}: the AIs below take every one.", plural(n)),
        (n, l) => format!(
            "{n} empty seat{}: {total} for the AIs below, {l} left closed.",
            plural(n)
        ),
    };
    ui.text_fit_left(
        r.x,
        r.y + 8.0,
        r.w,
        type_scale::CAPTION,
        rgb(palette::DIM, 1.0),
        &note,
    );
    let races: Vec<Pick> = race_picker::choices();
    let race_labels: Vec<&str> = races.iter().map(|p| p.label()).collect();
    let (gap, h) = (6.0, 34.0);
    let (race_w, diff_w, count_w, remove_w) = (120.0, 130.0, 150.0, 90.0);
    let removable = kinds.len() > 1;
    let mut remove = None;
    let mut y = r.y + NOTE_H;
    let mut left = left;
    for (k, kind) in kinds.iter_mut().enumerate() {
        let mut x = r.x;
        let cell = |x: f32, w: f32| Rect::new(x, y, w, h);
        let step = ui.stepper(
            id("fill-count", k),
            cell(x, count_w),
            &format!("{} AI{}", kind.count, plural(kind.count)),
            rgb(
                if kind.count > 0 {
                    palette::ACCENT
                } else {
                    palette::DIM
                },
                1.0,
            ),
            host,
        );
        // Up only while seats are left: the kinds never take more than there are.
        let most = kind.count + left;
        let count = (kind.count as i64 + step as i64).clamp(0, most as i64) as usize;
        left = most - count;
        kind.count = count;
        x += count_w + gap;
        let at = races.iter().position(|p| *p == kind.race).unwrap_or(0);
        if let Some(pick) = ui.choice(
            id("fill-race", k),
            cell(x, race_w),
            "",
            &race_labels,
            at,
            host,
        ) {
            kind.race = races[pick];
        }
        x += race_w + gap;
        let at = difficulty_at(kind.ai.difficulty);
        if let Some(pick) = ui.choice(
            id("fill-difficulty", k),
            cell(x, diff_w),
            "",
            &DIFFICULTY_LABELS,
            at,
            host,
        ) {
            kind.ai.difficulty = DIFFICULTIES[pick];
        }
        x += diff_w + gap;
        let line_w = r.right() - x - (remove_w + gap);
        // Past every seat's key, so no seat's line shares the ids.
        let line = mc_core::MAX_PLAYERS + k;
        config_line(ui, &mut kind.ai, line, cell(x, line_w), host, true);
        x += line_w + gap;
        if removable
            && ui.button(
                id("fill-remove", k),
                cell(x, remove_w),
                "Remove",
                ButtonKind::Secondary,
                host,
            )
        {
            remove = Some(k);
        }
        y += ROW_PITCH;
    }
    if let Some(k) = remove {
        kinds.remove(k);
        ui.audio.play(Sfx::Back);
    }
    let more = host && kinds.len() < MOST_KINDS;
    if ui.button(
        id("fill-add", 0),
        Rect::new(r.x, y + 4.0, 180.0, h),
        "Add Kind",
        ButtonKind::Secondary,
        more,
    ) && more
    {
        add_kind(kinds, left);
        ui.audio.play(Sfx::Select);
    }
}

/// A new kind, set up as the last one so one change makes it different. It
/// takes the seats `left` over; with none left, half the last kind's.
fn add_kind(kinds: &mut Vec<Kind>, left: usize) {
    let mut next = kinds.last().copied().unwrap_or_default();
    next.count = left;
    if left == 0 {
        if let Some(last) = kinds.last_mut() {
            next.count = last.count / 2;
            last.count -= next.count;
        }
    }
    kinds.push(next);
}

/// Which kind (by index into `counts`) each seat gets, in seat order: as many
/// seats as the counts add up to, kind `k` taking `counts[k]` of them. Each
/// seat goes to the kind furthest behind its share of the seats dealt so far
/// (the first of a tie), so the kinds take turns down the list.
fn deal(counts: &[usize]) -> Vec<usize> {
    let total: usize = counts.iter().sum();
    let mut dealt = vec![0usize; counts.len()];
    (0..total)
        .map(|j| {
            // Kind k is owed (j + 1) * count / total seats by now; compare in whole numbers.
            let k = (0..counts.len())
                .max_by_key(|&k| {
                    let owed = ((j + 1) * counts[k]) as i64;
                    let had = (dealt[k] * total) as i64;
                    (owed - had, std::cmp::Reverse(k))
                })
                .unwrap_or(0);
            dealt[k] += 1;
            k
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::lineup::roster::{Control, Roster};

    fn tally(counts: &[usize]) -> Vec<usize> {
        let mut c = vec![0; counts.len()];
        for k in deal(counts) {
            c[k] += 1;
        }
        c
    }

    #[test]
    fn each_kind_gets_its_count() {
        for counts in [
            vec![7],
            vec![20, 10],
            vec![3, 2, 2],
            vec![1, 29],
            vec![0, 5, 0, 3],
            vec![],
        ] {
            assert_eq!(tally(&counts), counts);
        }
    }

    #[test]
    fn the_kinds_take_turns_down_the_list() {
        assert_eq!(deal(&[2, 2]), vec![0, 1, 0, 1]);
        assert_eq!(deal(&[4, 2]), vec![0, 1, 0, 0, 1, 0]);
    }

    #[test]
    fn the_mix_never_takes_more_seats_than_are_empty() {
        let kind = |count| Kind {
            count,
            ..Kind::default()
        };
        let mut kinds = vec![kind(4), kind(3), kind(2)];
        fit(&mut kinds, 5);
        assert_eq!(kinds.iter().map(|k| k.count).collect::<Vec<_>>(), [4, 1, 0]);
        // A new kind takes what is left, or half the last kind's.
        add_kind(&mut kinds, 0);
        assert_eq!(
            kinds.iter().map(|k| k.count).collect::<Vec<_>>(),
            [4, 1, 0, 0]
        );
        let mut kinds = vec![kind(6)];
        add_kind(&mut kinds, 0);
        assert_eq!(kinds.iter().map(|k| k.count).collect::<Vec<_>>(), [3, 3]);
        add_kind(&mut kinds, 2);
        assert_eq!(kinds[2].count, 2);
        // One kind takes every empty seat when the sheet opens.
        let mut fill = Fill::default();
        fill.open(9);
        assert_eq!(fill.total(), 9);
    }

    #[test]
    fn filled_seats_wear_their_kinds_settings() {
        let hard = AiConfig {
            difficulty: mc_sim::Difficulty::Hard,
            income: 2000,
            ..AiConfig::default()
        };
        // Three of the default and two hard randoms, on eight seats with six empty: one stays closed.
        let fill = Fill {
            sheet: Sheet::default(),
            kinds: vec![
                Kind {
                    count: 3,
                    ..Kind::default()
                },
                Kind {
                    ai: hard,
                    race: Pick::Random,
                    count: 2,
                },
            ],
        };
        let catalog = Catalog::new(Vec::new(), Vec::new(), Vec::new());
        let mut lineup = Lineup::new(&catalog, Mode::Skirmish, 0, 1, 0);
        lineup.roster = Roster::new(8, 1, 1, false);
        let opened = lineup.roster.fill_ai(fill.total(), 8, |_| false).unwrap();
        assert_eq!(opened.len(), 5);
        fill.dress(&mut lineup, &opened);
        let seats = &lineup.roster.seats;
        assert_eq!(
            seats
                .iter()
                .filter(|s| s.control == Control::Closed)
                .count(),
            1
        );
        let hard_seats = opened.iter().filter(|&&i| seats[i].ai == hard).count();
        assert_eq!(hard_seats, 2);
        assert!(opened
            .iter()
            .all(|&i| (seats[i].ai == hard) == (seats[i].race == Pick::Random)));
        assert_eq!(
            seats[1].ai,
            AiConfig::default(),
            "the AI already seated is left alone"
        );
    }
}
