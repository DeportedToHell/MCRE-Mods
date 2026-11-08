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
pub const VERSION: &str = "1.1.0";

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

/// A point of the curve, (x, y): x is the flight's time, y how far along its
/// way it is drawn, both 0 to 1 between the fixed ends.
pub type Point = (f64, f64);

/// Named curves the mod's page steps through (the config screen's editor
/// makes any other). Each is the editor's own kind of curve: points between
/// the fixed (0, 0) and (1, 1), joined by the same Catmull-Rom spline.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Curve {
    /// The original's default (no points of its own, as Reset leaves it).
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

    /// The editor's points between (0, 0) and (1, 1), by x. Linear is as
    /// straight as the spline goes (its ends ease a little).
    pub fn points(self) -> Vec<Point> {
        match self {
            Curve::Original => vec![],
            Curve::Linear => vec![(0.25, 0.25), (0.5, 0.5), (0.75, 0.75)],
            Curve::Smooth => vec![(0.3, 0.12), (0.7, 0.88)],
            Curve::FastStart => vec![(0.2, 0.5), (0.5, 0.85)],
            Curve::SlowStart => vec![(0.5, 0.15), (0.8, 0.5)],
            Curve::Spring => vec![(0.65, 1.08)],
        }
    }

    /// The named curve these points are, if any.
    pub fn of(points: &[Point]) -> Option<Curve> {
        Curve::ALL.into_iter().find(|c| {
            let own = c.points();
            own.len() == points.len()
                && own.iter().zip(points).all(|(a, b)| (a.0 - b.0).abs() < 1e-4 && (a.1 - b.1).abs() < 1e-4)
        })
    }

}

/// The original's `CatmullRomSpline` segment from `y1` to `y2` (alpha 0,
/// tension 0) at `u` (0 to 1), on one axis: `a u^3 + b u^2 + c u + d`.
fn segment(y0: f64, y1: f64, y2: f64, y3: f64, u: f64) -> f64 {
    // m1 = (p2 - p1) + ((p1 - p0) / t01 - (p2 - p0) / (t01 + t12)) * t12, with
    // every t 1 (alpha 0), and m2 likewise.
    let m1 = (y2 - y1) + ((y1 - y0) - (y2 - y0) / 2.0);
    let m2 = (y2 - y1) + ((y3 - y2) - (y3 - y1) / 2.0);
    let a = 2.0 * (y1 - y2) + m1 + m2;
    let b = -3.0 * (y1 - y2) - 2.0 * m1 - m2;
    a * u * u * u + b * u * u + m1 * u + y1
}

/// The editor's whole list: (0, 0) twice, `points`, then (1, 1) twice.
pub fn with_ends(points: &[Point]) -> Vec<Point> {
    let mut p = vec![(0.0, 0.0), (0.0, 0.0)];
    p.extend_from_slice(points);
    p.push((1.0, 1.0));
    p.push((1.0, 1.0));
    p
}

/// `Segment.getPoint`: the spline from `p[1]` to `p[2]` at `u`, both axes,
/// as the editor draws it.
pub fn spline_point(p: [Point; 4], u: f64) -> Point {
    (segment(p[0].0, p[1].0, p[2].0, p[3].0, u), segment(p[0].1, p[1].1, p[2].1, p[3].1, u))
}

/// The original's `CatmullRomWidget.getProgress`: a uniform Catmull-Rom
/// spline (alpha 0, tension 0) through (0, 0) twice, `points`, then (1, 1)
/// twice; `t` picks the segment by x and goes through it linearly. At t = 1,
/// which no segment's [x1, x2) holds, the last segment ends (the original
/// fell back to the first).
pub fn catmull_rom(points: &[Point], t: f64) -> f64 {
    let p = with_ends(points);
    let t = t.clamp(0.0, 1.0);
    let mut seg = p.len() - 3;
    for i in 1..p.len() - 2 {
        if t >= p[i].0 && t < p[i + 1].0 {
            seg = i;
            break;
        }
    }
    let span = p[seg + 1].0 - p[seg].0;
    let u = if span > 0.0 { (t - p[seg].0) / span } else { 0.0 };
    segment(p[seg - 1].1, p[seg].1, p[seg + 1].1, p[seg + 2].1, u)
}

// ---- settings --------------------------------------------------------------

/// The speeds in percent the mod's page steps through (the config screen's
/// slider sets any ten from 10 to 500).
pub const SPEEDS: [u32; 8] = [25, 50, 75, 100, 150, 200, 300, 500];

/// The original's `Config`, and the port's own Fly to the cursor.
#[derive(Clone, PartialEq, Debug)]
pub struct Settings {
    /// `toggleMod`: off, nothing flies.
    pub enabled: bool,
    /// `animationSpeed` in percent: 10 to 500, 100 by default.
    pub speed: u32,
    /// `curvePoints`: the curve's points between the fixed ends, by x.
    pub points: Vec<Point>,
    /// Items taken into the cursor fly to it (several at once: a double
    /// click gathering a kind).
    pub to_cursor: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            enabled: true,
            speed: 100,
            points: Vec::new(),
            to_cursor: true,
        }
    }
}

/// "x,y;x,y" as the settings keep the points (floats, as `Config` saves them).
fn format_points(points: &[Point]) -> String {
    points
        .iter()
        .map(|&(x, y)| format!("{},{}", x as f32, y as f32))
        .collect::<Vec<_>>()
        .join(";")
}

fn parse_points(s: &str) -> Vec<Point> {
    let mut points: Vec<Point> = s
        .split(';')
        .filter_map(|p| {
            let (x, y) = p.split_once(',')?;
            let (x, y) = (x.trim().parse::<f32>().ok()?, y.trim().parse::<f32>().ok()?);
            (x.is_finite() && y.is_finite()).then_some((f64::from(x), f64::from(y)))
        })
        .collect();
    points.sort_by(|a, b| a.0.total_cmp(&b.0));
    points
}

impl Settings {
    pub fn read(get: impl Fn(&str) -> Option<String>) -> Settings {
        let mut s = Settings::default();
        let flag = |key: &str| match get(key).as_deref().map(str::trim) {
            Some("true") => Some(true),
            Some("false") => Some(false),
            _ => None,
        };
        if let Some(on) = flag("toggle_mod") {
            s.enabled = on;
        }
        // `Codec.intRange(10, 500)`.
        if let Some(v) = get("speed").and_then(|v| v.trim().parse::<u32>().ok()).filter(|v| (10..=500).contains(v)) {
            s.speed = v;
        }
        match get("curve").as_deref().map(str::trim) {
            Some("custom") => s.points = get("curve_points").map(|v| parse_points(&v)).unwrap_or_default(),
            Some(name) => {
                if let Some(c) = Curve::ALL.iter().find(|c| c.stored() == name) {
                    s.points = c.points();
                }
            }
            None => {}
        }
        if let Some(on) = flag("to_cursor") {
            s.to_cursor = on;
        }
        s
    }

    pub fn stored(&self) -> Vec<(&'static str, String)> {
        vec![
            ("toggle_mod", self.enabled.to_string()),
            ("speed", self.speed.to_string()),
            ("curve", Curve::of(&self.points).map_or("custom", Curve::stored).to_string()),
            ("curve_points", format_points(&self.points)),
            ("to_cursor", self.to_cursor.to_string()),
        ]
    }

    /// The page's buttons after its first.
    pub fn labels(&self, ru: bool) -> Vec<String> {
        let on = |b: bool| match (b, ru) {
            (true, true) => "вкл",
            (false, true) => "выкл",
            (true, false) => "On",
            (false, false) => "Off",
        };
        let custom = if ru { "своя" } else { "Custom" };
        let curve = Curve::of(&self.points).map_or(custom, |c| c.label(ru));
        let speed = self.speed;
        if ru {
            vec![
                format!("Скорость анимации: {speed}%"),
                format!("Кривая: {curve}"),
                format!("Полёт в курсор: {}", on(self.to_cursor)),
            ]
        } else {
            vec![
                format!("Animation speed: {speed}%"),
                format!("Curve: {curve}"),
                format!("Fly to the cursor: {}", on(self.to_cursor)),
            ]
        }
    }

    /// A click on the page's button `button` (after its first): the next
    /// speed, the next named curve, Fly to the cursor.
    pub fn click(&mut self, button: usize) {
        match button {
            0 => self.speed = SPEEDS.into_iter().find(|&v| v > self.speed).unwrap_or(SPEEDS[0]),
            1 => {
                let next = match Curve::of(&self.points).and_then(|c| Curve::ALL.iter().position(|x| *x == c)) {
                    Some(i) => Curve::ALL[(i + 1) % Curve::ALL.len()],
                    None => Curve::Original,
                };
                self.points = next.points();
            }
            2 => self.to_cursor = !self.to_cursor,
            _ => {}
        }
    }

    /// Progress per millisecond. The original moves a flight a tenth of its
    /// way each tick at 100%: half a second.
    pub fn rate_per_ms(&self) -> f64 {
        f64::from(self.speed) / 100.0 / 500.0
    }
}

static SETTINGS: Mutex<Option<Settings>> = Mutex::new(None);

/// The settings, read once from the game's store.
pub fn settings(get: impl Fn(&str) -> Option<String>) -> Settings {
    with_settings(get, Settings::clone)
}

/// `f` on the settings, read once from the game's store, without a copy.
pub fn with_settings<R>(get: impl Fn(&str) -> Option<String>, f: impl FnOnce(&Settings) -> R) -> R {
    let mut kept = SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
    f(kept.get_or_insert_with(|| Settings::read(get)))
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
/// slots' top left corners, along the curve through `points`.
pub fn position(from: [f32; 2], to: [f32; 2], progress: f64, points: &[Point]) -> [f32; 2] {
    let e = catmull_rom(points, progress) as f32;
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
            assert!((catmull_rom(&Curve::Original.points(), u) - want).abs() < 1e-12, "{u}");
        }
    }

    #[test]
    fn every_curve_starts_at_0_and_lands_at_1() {
        for c in Curve::ALL {
            assert!(catmull_rom(&c.points(), 0.0).abs() < 1e-12, "{c:?}");
            assert!((catmull_rom(&c.points(), 1.0) - 1.0).abs() < 1e-12, "{c:?}");
            if c != Curve::Spring {
                let mut last = 0.0;
                for i in 0..=200 {
                    let y = catmull_rom(&c.points(), f64::from(i) / 200.0);
                    assert!(y >= last - 1e-9, "{c:?} at {i}");
                    last = y;
                }
            }
        }
        assert!(catmull_rom(&Curve::Spring.points(), 0.85) > 1.0);
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
        assert_eq!(s.speed, 100);
        assert!((s.rate_per_ms() * 500.0 - 1.0).abs() < 1e-12);
        let mut t = s.clone();
        t.click(0);
        t.click(1);
        t.click(2);
        assert_eq!((t.speed, Curve::of(&t.points)), (150, Some(Curve::Linear)));
        let stored: HashMap<_, _> = t.stored().into_iter().collect();
        assert_eq!(Settings::read(|k| stored.get(k).cloned()), t);
        assert_eq!(t.labels(true).len(), 3);
        // A speed the slider set steps on to the next preset; the last wraps.
        t.speed = 130;
        t.click(0);
        assert_eq!(t.speed, 150);
        t.speed = 500;
        t.click(0);
        assert_eq!(t.speed, 25);
        assert_eq!(shown_count(64, 24), 40);
        assert_eq!(shown_count(16, 16), 0);
        assert_eq!(position([0.0, 0.0], [18.0, 36.0], 1.0, &[]), [18.0, 36.0]);
        assert_eq!(position([0.0, 0.0], [18.0, 36.0], 0.0, &Curve::Spring.points()), [0.0, 0.0]);
    }

    #[test]
    fn edited_curves_keep_their_points() {
        // The editor's own points: kept as floats, read back sorted.
        let mut s = Settings { enabled: false, speed: 230, ..Settings::default() };
        s.points = vec![(0.6, 0.9), (0.2, 0.1)];
        let stored: HashMap<_, _> = s.stored().into_iter().collect();
        assert_eq!(stored["curve"], "custom");
        let back = Settings::read(|k| stored.get(k).cloned());
        assert!(!back.enabled);
        assert_eq!(back.speed, 230);
        assert_eq!(back.points.len(), 2);
        assert!((back.points[0].0 - 0.2).abs() < 1e-6 && (back.points[1].1 - 0.9).abs() < 1e-6);
        // A custom curve steps on to the first named one; junk is ignored.
        s.click(1);
        assert_eq!(Curve::of(&s.points), Some(Curve::Original));
        let junk = Settings::read(|k| (k != "curve").then(|| "junk".to_owned()).or(Some("custom".into())));
        assert_eq!(junk, Settings::default());
        // The old named curves still read.
        let old = Settings::read(|k| (k == "curve").then(|| "smooth".to_owned()));
        assert_eq!(Curve::of(&old.points), Some(Curve::Smooth));
    }

    #[test]
    fn the_editor_draws_the_same_spline() {
        // Its segments, both axes, pass through their points.
        let p = with_ends(&[(0.4, 0.7)]);
        for i in 1..p.len() - 2 {
            let seg = [p[i - 1], p[i], p[i + 1], p[i + 2]];
            let (a, b) = (spline_point(seg, 0.0), spline_point(seg, 1.0));
            assert!((a.0 - p[i].0).abs() < 1e-12 && (a.1 - p[i].1).abs() < 1e-12);
            assert!((b.0 - p[i + 1].0).abs() < 1e-12 && (b.1 - p[i + 1].1).abs() < 1e-12);
        }
    }
}
