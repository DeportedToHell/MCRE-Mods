// Smooth Swapping for Horizon: the mod's logic, with no game types, so the
// same file builds and tests on its own (`cargo test` at the repository root)
// and is what the bundle adds to the game as `crate::game::smoothswapping`.
//
// A port of Smooth Swapping by Schauweg (LGPL-3.0,
// https://github.com/Schauweg/Smooth-Swapping): items moved in an inventory
// fly from slot to slot instead of jumping.

use std::collections::HashMap;
use std::sync::Mutex;

pub const ID: &str = "smoothswapping";
pub const NAME: &str = "Smooth Swapping";
pub const VERSION: &str = "1.0.0";

pub fn russian(language: &str) -> bool {
    language.starts_with("ru")
}

pub fn description(ru: bool) -> &'static str {
    if ru {
        "Плавно анимирует перемещение предметов в инвентаре."
    } else {
        "Animates item movement inside inventories."
    }
}

// ---- the animation curve ---------------------------------------------------

/// The curves the settings offer in place of the original's curve editor.
/// Each is the editor's own kind of curve: points between the fixed (0, 0)
/// and (1, 1), joined by the same Catmull-Rom spline.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Curve {
    /// The original's default (no points of its own).
    Original,
    Linear,
    Smooth,
    FastStart,
    SlowStart,
    Spring,
}

impl Curve {
    pub const ALL: [Curve; 6] = [
        Curve::Original,
        Curve::Linear,
        Curve::Smooth,
        Curve::FastStart,
        Curve::SlowStart,
        Curve::Spring,
    ];

    fn stored(self) -> &'static str {
        match self {
            Curve::Original => "original",
            Curve::Linear => "linear",
            Curve::Smooth => "smooth",
            Curve::FastStart => "fast_start",
            Curve::SlowStart => "slow_start",
            Curve::Spring => "spring",
        }
    }

    fn label(self, ru: bool) -> &'static str {
        match (self, ru) {
            (Curve::Original, true) => "как в оригинале",
            (Curve::Linear, true) => "равномерно",
            (Curve::Smooth, true) => "плавно",
            (Curve::FastStart, true) => "быстрый старт",
            (Curve::SlowStart, true) => "медленный старт",
            (Curve::Spring, true) => "пружина",
            (Curve::Original, false) => "Original",
            (Curve::Linear, false) => "Linear",
            (Curve::Smooth, false) => "Smooth",
            (Curve::FastStart, false) => "Fast start",
            (Curve::SlowStart, false) => "Slow start",
            (Curve::Spring, false) => "Spring",
        }
    }

    /// The editor's points between (0, 0) and (1, 1), by x.
    pub fn points(self) -> &'static [(f64, f64)] {
        match self {
            Curve::Original | Curve::Linear => &[],
            Curve::Smooth => &[(0.3, 0.12), (0.7, 0.88)],
            Curve::FastStart => &[(0.2, 0.5), (0.5, 0.85)],
            Curve::SlowStart => &[(0.5, 0.15), (0.8, 0.5)],
            Curve::Spring => &[(0.65, 1.08)],
        }
    }

    /// How far along its way a flight is drawn at time `t` (0 to 1).
    pub fn ease(self, t: f64) -> f64 {
        match self {
            Curve::Linear => t.clamp(0.0, 1.0),
            _ => catmull_rom(self.points(), t),
        }
    }
}

/// The original's `CatmullRomWidget.getProgress`: a uniform Catmull-Rom
/// spline (alpha 0, tension 0) through (0, 0) twice, `points`, then (1, 1)
/// twice; `t` picks the segment by x and goes through it linearly. At t = 1,
/// which no segment's [x1, x2) holds, the last segment ends (the original
/// fell back to the first).
pub fn catmull_rom(points: &[(f64, f64)], t: f64) -> f64 {
    let mut p = vec![(0.0, 0.0), (0.0, 0.0)];
    p.extend_from_slice(points);
    p.push((1.0, 1.0));
    p.push((1.0, 1.0));
    let t = t.clamp(0.0, 1.0);
    let mut seg = p.len() - 3;
    for i in 1..p.len() - 2 {
        if t >= p[i].0 && t < p[i + 1].0 {
            seg = i;
            break;
        }
    }
    let (y0, y1, y2, y3) = (p[seg - 1].1, p[seg].1, p[seg + 1].1, p[seg + 2].1);
    let span = p[seg + 1].0 - p[seg].0;
    let u = if span > 0.0 { (t - p[seg].0) / span } else { 0.0 };
    // m1 = (p2 - p1) + ((p1 - p0) / t01 - (p2 - p0) / (t01 + t12)) * t12, with
    // every t 1 (alpha 0), and m2 likewise.
    let m1 = (y2 - y1) + ((y1 - y0) - (y2 - y0) / 2.0);
    let m2 = (y2 - y1) + ((y3 - y2) - (y3 - y1) / 2.0);
    let a = 2.0 * (y1 - y2) + m1 + m2;
    let b = -3.0 * (y1 - y2) - 2.0 * m1 - m2;
    a * u * u * u + b * u * u + m1 * u + y1
}

// ---- settings --------------------------------------------------------------

/// The animation speed in percent: the original's slider runs from 10 to
/// 500, 100 by default.
pub const SPEEDS: [u32; 8] = [25, 50, 75, 100, 150, 200, 300, 500];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Settings {
    /// An index into `SPEEDS`.
    pub speed: usize,
    pub curve: Curve,
    /// Items taken into the cursor fly to it (several at once: a double
    /// click gathering a kind).
    pub to_cursor: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            speed: 3,
            curve: Curve::Original,
            to_cursor: true,
        }
    }
}

impl Settings {
    pub fn read(get: impl Fn(&str) -> Option<String>) -> Settings {
        let mut s = Settings::default();
        if let Some(i) = get("speed").and_then(|v| v.parse::<u32>().ok()) {
            if let Some(p) = SPEEDS.iter().position(|&v| v == i) {
                s.speed = p;
            }
        }
        if let Some(c) = get("curve") {
            if let Some(c) = Curve::ALL.iter().find(|x| x.stored() == c) {
                s.curve = *c;
            }
        }
        match get("to_cursor").as_deref() {
            Some("true") => s.to_cursor = true,
            Some("false") => s.to_cursor = false,
            _ => {}
        }
        s
    }

    pub fn stored(&self) -> Vec<(&'static str, String)> {
        vec![
            ("speed", SPEEDS[self.speed].to_string()),
            ("curve", self.curve.stored().to_string()),
            ("to_cursor", self.to_cursor.to_string()),
        ]
    }

    pub fn labels(&self, ru: bool) -> Vec<String> {
        let on = |b: bool| match (b, ru) {
            (true, true) => "вкл",
            (false, true) => "выкл",
            (true, false) => "On",
            (false, false) => "Off",
        };
        let speed = SPEEDS[self.speed];
        if ru {
            vec![
                format!("Скорость анимации: {speed}%"),
                format!("Кривая: {}", self.curve.label(true)),
                format!("Полёт в курсор: {}", on(self.to_cursor)),
            ]
        } else {
            vec![
                format!("Animation speed: {speed}%"),
                format!("Curve: {}", self.curve.label(false)),
                format!("Fly to the cursor: {}", on(self.to_cursor)),
            ]
        }
    }

    pub fn click(&mut self, button: usize) {
        match button {
            0 => self.speed = (self.speed + 1) % SPEEDS.len(),
            1 => {
                let i = Curve::ALL.iter().position(|c| *c == self.curve).unwrap_or(0);
                self.curve = Curve::ALL[(i + 1) % Curve::ALL.len()];
            }
            2 => self.to_cursor = !self.to_cursor,
            _ => {}
        }
    }

    /// Progress per millisecond. The original moves a flight a tenth of its
    /// way each tick at 100%: half a second.
    pub fn rate_per_ms(&self) -> f64 {
        f64::from(SPEEDS[self.speed]) / 100.0 / 500.0
    }
}

static SETTINGS: Mutex<Option<Settings>> = Mutex::new(None);

pub fn settings(get: impl Fn(&str) -> Option<String>) -> Settings {
    let mut kept = SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
    *kept.get_or_insert_with(|| Settings::read(get))
}

pub fn set_settings(settings: Settings) {
    *SETTINGS.lock().unwrap_or_else(|e| e.into_inner()) = Some(settings);
}

// ---- which items flew where ------------------------------------------------

/// A slot's stack as the matcher sees it: its kind (the same item with the
/// same components) and its count.
pub type Held = Option<(u32, u32)>;

/// One slot that changed since the last frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Change {
    pub slot: usize,
    pub old: Held,
    pub new: Held,
    /// A result slot (crafting, a furnace's output): computed, so its own
    /// changes are no moves, but what is taken from it flies from it.
    pub result: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum To {
    Slot(usize),
    Cursor,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Flight {
    pub from: usize,
    pub to: To,
    pub kind: u32,
    pub amount: u32,
}

/// The flights that explain one frame's changes, as the original pairs the
/// slots that gained items with those that lost them (`assignI2ISwaps`), by
/// kind: each slot that gained a kind takes it from the slots that lost it,
/// in slot order; what nothing lost came from a result slot holding the kind
/// (a shift-clicked craft), or from the cursor (no flight); what was lost and
/// nothing gained went to the cursor if the cursor gained it (a pickup, a
/// double click), or was used up (no flight).
pub fn flights(changes: &[Change], cursor_old: Held, cursor_new: Held, to_cursor: bool) -> Vec<Flight> {
    let mut kinds: Vec<u32> = Vec::new();
    let mut gains: HashMap<u32, Vec<(usize, u32)>> = HashMap::new();
    let mut losses: HashMap<u32, Vec<(usize, u32)>> = HashMap::new();
    let mut results: HashMap<u32, usize> = HashMap::new();
    let note = |k: u32, kinds: &mut Vec<u32>| {
        if !kinds.contains(&k) {
            kinds.push(k);
        }
    };
    for c in changes {
        if c.result {
            if let Some((k, _)) = c.old {
                results.entry(k).or_insert(c.slot);
                note(k, &mut kinds);
            }
            continue;
        }
        match (c.old, c.new) {
            (Some((ko, no)), Some((kn, nn))) if ko == kn => {
                note(ko, &mut kinds);
                if nn > no {
                    gains.entry(ko).or_default().push((c.slot, nn - no));
                } else if no > nn {
                    losses.entry(ko).or_default().push((c.slot, no - nn));
                }
            }
            (old, new) => {
                if let Some((k, n)) = old {
                    note(k, &mut kinds);
                    losses.entry(k).or_default().push((c.slot, n));
                }
                if let Some((k, n)) = new {
                    note(k, &mut kinds);
                    gains.entry(k).or_default().push((c.slot, n));
                }
            }
        }
    }
    let cursor_gain = |k: u32| -> u32 {
        match cursor_new {
            Some((kn, nn)) if kn == k => {
                let before = match cursor_old {
                    Some((ko, no)) if ko == k => no,
                    _ => 0,
                };
                nn.saturating_sub(before)
            }
            _ => 0,
        }
    };
    let mut out = Vec::new();
    for k in kinds {
        let mut lost = losses.remove(&k).unwrap_or_default();
        let mut li = 0;
        for (slot, mut need) in gains.remove(&k).unwrap_or_default() {
            while need > 0 && li < lost.len() {
                let take = need.min(lost[li].1);
                out.push(Flight { from: lost[li].0, to: To::Slot(slot), kind: k, amount: take });
                need -= take;
                lost[li].1 -= take;
                if lost[li].1 == 0 {
                    li += 1;
                }
            }
            if need > 0 {
                if let Some(&r) = results.get(&k) {
                    out.push(Flight { from: r, to: To::Slot(slot), kind: k, amount: need });
                }
            }
        }
        if !to_cursor {
            continue;
        }
        let mut gain = cursor_gain(k);
        for &(slot, left) in &lost[li..] {
            if gain == 0 {
                break;
            }
            let take = left.min(gain);
            if take > 0 {
                out.push(Flight { from: slot, to: To::Cursor, kind: k, amount: take });
                gain -= take;
            }
        }
        if gain > 0 && lost[li..].iter().all(|l| l.1 == 0) {
            if let Some(&r) = results.get(&k) {
                out.push(Flight { from: r, to: To::Cursor, kind: k, amount: gain });
            }
        }
    }
    out
}

/// How much of a slot's stack shows while `incoming` items still fly to it:
/// what is already there; nothing until the first one lands into an empty
/// slot.
pub fn shown_count(count: u32, incoming: u32) -> u32 {
    count.saturating_sub(incoming)
}

/// Where a flight is drawn at `progress` (0 to 1), from and to being the
/// slots' top left corners.
pub fn position(from: [f32; 2], to: [f32; 2], progress: f64, curve: Curve) -> [f32; 2] {
    let e = curve.ease(progress) as f32;
    [from[0] + (to[0] - from[0]) * e, from[1] + (to[1] - from[1]) * e]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(slot: usize, old: Held, new: Held) -> Change {
        Change { slot, old, new, result: false }
    }

    #[test]
    fn the_original_curve_is_its_default_spline() {
        // p0 = p1 = (0, 0), p2 = p3 = (1, 1): y = -u^3 + 1.5u^2 + 0.5u.
        for i in 0..=20 {
            let u = f64::from(i) / 20.0;
            let want = -u * u * u + 1.5 * u * u + 0.5 * u;
            assert!((Curve::Original.ease(u) - want).abs() < 1e-12, "{u}");
        }
    }

    #[test]
    fn every_curve_starts_at_0_and_lands_at_1() {
        for c in Curve::ALL {
            assert!(c.ease(0.0).abs() < 1e-12, "{c:?}");
            assert!((c.ease(1.0) - 1.0).abs() < 1e-12, "{c:?}");
            if c != Curve::Spring {
                let mut last = 0.0;
                for i in 0..=200 {
                    let y = c.ease(f64::from(i) / 200.0);
                    assert!(y >= last - 1e-9, "{c:?} at {i}");
                    last = y;
                }
            }
        }
        assert!(Curve::Spring.ease(0.85) > 1.0);
    }

    #[test]
    fn a_shift_click_flies_to_each_destination() {
        // 40 of kind 1 leave slot 9: 24 top up slot 300, 16 land in empty 302.
        let changes = [
            change(9, Some((1, 40)), None),
            change(300, Some((1, 40)), Some((1, 64))),
            change(302, None, Some((1, 16))),
        ];
        let f = flights(&changes, None, None, true);
        assert_eq!(
            f,
            vec![
                Flight { from: 9, to: To::Slot(300), kind: 1, amount: 24 },
                Flight { from: 9, to: To::Slot(302), kind: 1, amount: 16 },
            ]
        );
    }

    #[test]
    fn a_hotbar_swap_flies_both_ways() {
        let changes = [change(0, Some((1, 5)), Some((2, 3))), change(20, Some((2, 3)), Some((1, 5)))];
        let f = flights(&changes, None, None, true);
        assert!(f.contains(&Flight { from: 0, to: To::Slot(20), kind: 1, amount: 5 }));
        assert!(f.contains(&Flight { from: 20, to: To::Slot(0), kind: 2, amount: 3 }));
        assert_eq!(f.len(), 2);
    }

    #[test]
    fn pickups_fly_to_the_cursor_and_placing_does_not() {
        let pick = [change(5, Some((3, 10)), Some((3, 5)))];
        assert_eq!(
            flights(&pick, None, Some((3, 5)), true),
            vec![Flight { from: 5, to: To::Cursor, kind: 3, amount: 5 }]
        );
        assert!(flights(&pick, None, Some((3, 5)), false).is_empty());
        let place = [change(5, None, Some((3, 5)))];
        assert!(flights(&place, Some((3, 5)), None, true).is_empty());
        // A double click gathers from three slots.
        let gather = [
            change(1, Some((3, 10)), None),
            change(2, Some((3, 20)), None),
            change(3, Some((3, 30)), Some((3, 16))),
        ];
        let f = flights(&gather, Some((3, 20)), Some((3, 64)), true);
        assert_eq!(f.iter().map(|x| x.amount).sum::<u32>(), 44);
        assert!(f.iter().all(|x| x.to == To::Cursor));
    }

    #[test]
    fn crafted_items_fly_from_the_result_and_ingredients_vanish() {
        // Shift-clicking the result: the grid loses planks, the inventory
        // gains sticks, the result slot still shows sticks.
        let changes = [
            Change { slot: 200, old: Some((7, 4)), new: Some((7, 4)), result: true },
            change(100, Some((6, 2)), Some((6, 1))),
            change(101, Some((6, 2)), Some((6, 1))),
            change(9, None, Some((7, 4))),
        ];
        let f = flights(&changes, None, None, true);
        assert_eq!(f, vec![Flight { from: 200, to: To::Slot(9), kind: 7, amount: 4 }]);
        // Taking it into the cursor.
        let take = [
            Change { slot: 200, old: Some((7, 4)), new: None, result: true },
            change(100, Some((6, 1)), None),
        ];
        assert_eq!(
            flights(&take, None, Some((7, 4)), true),
            vec![Flight { from: 200, to: To::Cursor, kind: 7, amount: 4 }]
        );
    }

    #[test]
    fn items_from_nowhere_do_not_fly() {
        // A container's contents arriving, or the server filling a slot.
        let changes = [change(300, None, Some((1, 64))), change(301, None, Some((2, 1)))];
        assert!(flights(&changes, None, None, true).is_empty());
    }

    #[test]
    fn a_sort_is_a_permutation_of_flights() {
        // Three stacks change places: every item flies once.
        let changes = [
            change(300, Some((1, 64)), Some((3, 10))),
            change(301, Some((2, 5)), Some((1, 64))),
            change(302, Some((3, 10)), Some((2, 5))),
        ];
        let f = flights(&changes, None, None, true);
        assert_eq!(f.len(), 3);
        for (from, to, kind, amount) in [(300, 301, 1, 64), (301, 302, 2, 5), (302, 300, 3, 10)] {
            assert!(f.contains(&Flight { from, to: To::Slot(to), kind, amount }), "{from}->{to}");
        }
    }

    #[test]
    fn speed_and_settings() {
        let s = Settings::default();
        assert_eq!(SPEEDS[s.speed], 100);
        assert!((s.rate_per_ms() * 500.0 - 1.0).abs() < 1e-12);
        let mut t = s;
        t.click(0);
        t.click(1);
        t.click(2);
        let stored: HashMap<_, _> = t.stored().into_iter().collect();
        assert_eq!(Settings::read(|k| stored.get(k).cloned()), t);
        assert_eq!(t.labels(true).len(), 3);
        assert_eq!(shown_count(64, 24), 40);
        assert_eq!(shown_count(16, 16), 0);
        assert_eq!(position([0.0, 0.0], [18.0, 36.0], 1.0, Curve::Original), [18.0, 36.0]);
        assert_eq!(position([0.0, 0.0], [18.0, 36.0], 0.0, Curve::Spring), [0.0, 0.0]);
    }
}
