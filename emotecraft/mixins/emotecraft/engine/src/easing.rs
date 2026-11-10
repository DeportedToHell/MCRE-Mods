//! PlayerAnimationLibrary's `EasingType`: the 38 easings with their wire ids and names, the
//! `step(n)` function (MIT, Copyright (c) 2015 Boris Chumichev), Blockbench's bezier handles
//! (`BezierEasing`) and Catmull-Rom (`CatmullRomEasing`).
//!
//! Ported from PlayerAnimationLibrary (Copyright (c) 2025 ZigyTheBird, MIT; based on GeckoLib,
//! Copyright (c) 2024 GeckoLib, MIT), see NOTICE.

use crate::molang::{Exprs, Runtime};
use std::f32::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Easing {
    Linear,
    Constant,
    Step,
    InSine,
    OutSine,
    InOutSine,
    InQuad,
    OutQuad,
    InOutQuad,
    InCubic,
    OutCubic,
    InOutCubic,
    InQuart,
    OutQuart,
    InOutQuart,
    InQuint,
    OutQuint,
    InOutQuint,
    InExpo,
    OutExpo,
    InOutExpo,
    InCirc,
    OutCirc,
    InOutCirc,
    InBack,
    OutBack,
    InOutBack,
    InElastic,
    OutElastic,
    InOutElastic,
    InBounce,
    OutBounce,
    InOutBounce,
    CatmullRom,
    Bezier,
}

use Easing::*;

/// (easing, id, name), in `EasingType`'s declaration order.
const TABLE: [(Easing, u8, &str); 35] = [
    (Linear, 0, "linear"),
    (Constant, 1, "constant"),
    (Step, 37, "step"),
    (InSine, 6, "easeinsine"),
    (OutSine, 7, "easeoutsine"),
    (InOutSine, 8, "easeinoutsine"),
    (InQuad, 12, "easeinquad"),
    (OutQuad, 13, "easeoutquad"),
    (InOutQuad, 14, "easeinoutquad"),
    (InCubic, 9, "easeincubic"),
    (OutCubic, 10, "easeoutcubic"),
    (InOutCubic, 11, "easeinoutcubic"),
    (InQuart, 15, "easeinquart"),
    (OutQuart, 16, "easeoutquart"),
    (InOutQuart, 17, "easeinoutquart"),
    (InQuint, 18, "easeinquint"),
    (OutQuint, 19, "easeoutquint"),
    (InOutQuint, 20, "easeinoutquint"),
    (InExpo, 21, "easeinexpo"),
    (OutExpo, 22, "easeoutexpo"),
    (InOutExpo, 23, "easeinoutexpo"),
    (InCirc, 24, "easeincirc"),
    (OutCirc, 25, "easeoutcirc"),
    (InOutCirc, 26, "easeinoutcirc"),
    (InBack, 27, "easeinback"),
    (OutBack, 28, "easeoutback"),
    (InOutBack, 29, "easeinoutback"),
    (InElastic, 30, "easeinelastic"),
    (OutElastic, 31, "easeoutelastic"),
    (InOutElastic, 32, "easeinoutelastic"),
    (InBounce, 33, "easeinbounce"),
    (OutBounce, 34, "easeoutbounce"),
    (InOutBounce, 35, "easeinoutbounce"),
    (CatmullRom, 36, "catmullrom"),
    (Bezier, 38, "bezier"),
];

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

impl Easing {
    pub fn id(self) -> u8 {
        TABLE.iter().find(|e| e.0 == self).map_or(0, |e| e.1)
    }
    /// `EasingType.fromId`: unknown ids are linear.
    pub fn from_id(id: u8) -> Easing {
        TABLE.iter().find(|e| e.1 == id).map_or(Linear, |e| e.0)
    }
    /// The exact name (lower case), or none.
    pub fn from_name(name: &str) -> Option<Easing> {
        let lower = name.to_ascii_lowercase();
        TABLE.iter().find(|e| e.2 == lower).map(|e| e.0)
    }
    /// `EasingType.fromString`: unknown names are linear.
    pub fn from_string(name: &str) -> Easing {
        Easing::from_name(name).unwrap_or(Linear)
    }
    /// `PlayerAnimatorLoader.easingTypeFromString`: `inOutSine` is `easeinoutsine`.
    pub fn from_legacy(name: &str) -> Easing {
        let e = Easing::from_string(name);
        if e == Linear { Easing::from_string(&format!("ease{name}")) } else { e }
    }
    pub fn name(self) -> &'static str {
        TABLE.iter().find(|e| e.0 == self).map_or("linear", |e| e.2)
    }

    /// `buildTransformer(value)` applied to `t`.
    fn transform(self, value: Option<f32>, t: f32) -> f32 {
        let in_out = |f: &dyn Fn(f32) -> f32, t: f32| {
            if f64::from(t) < 0.5 { f(t * 2.0) / 2.0 } else { 1.0 - f((1.0 - t) * 2.0) / 2.0 }
        };
        let out = |f: &dyn Fn(f32) -> f32, t: f32| 1.0 - f(1.0 - t);
        let sine = |n: f32| 1.0 - (n * PI / 2.0).cos();
        let quad = |n: f32| n * n;
        let cubic = |n: f32| n * n * n;
        let quart = |n: f32| n.powf(4.0);
        let quint = |n: f32| n.powf(5.0);
        let expo = |n: f32| 2f32.powf(10.0 * (n - 1.0));
        let circ = |n: f32| 1.0 - (1.0 - n * n).sqrt();
        let back = move |t: f32| {
            let n2 = value.map_or(1.70158, |n| n * 1.70158);
            t * t * ((n2 + 1.0) * t - n2)
        };
        let elastic = move |t: f32| {
            let n2 = value.unwrap_or(1.0);
            1.0 - (t * PI / 2.0).cos().powf(3.0) * (t * n2 * PI).cos()
        };
        let bounce = move |t: f32| {
            let n2 = value.unwrap_or(0.5);
            let one = 121.0 / 16.0 * t * t;
            let two = 121.0 / 4.0 * n2 * (t - 6.0 / 11.0).powf(2.0) + 1.0 - n2;
            let three = 121.0 * n2 * n2 * (t - 9.0 / 11.0).powf(2.0) + 1.0 - n2 * n2;
            let four = 484.0 * n2 * n2 * n2 * (t - 10.5 / 11.0).powf(2.0) + 1.0 - n2 * n2 * n2;
            one.min(two).min(three.min(four))
        };
        match self {
            Linear | CatmullRom | Bezier => t,
            Constant => 0.0,
            Step => step(value, t),
            InSine => sine(t),
            OutSine => out(&sine, t),
            InOutSine => in_out(&sine, t),
            InQuad => quad(t),
            OutQuad => out(&quad, t),
            InOutQuad => in_out(&quad, t),
            InCubic => cubic(t),
            OutCubic => out(&cubic, t),
            InOutCubic => in_out(&cubic, t),
            InQuart => quart(t),
            OutQuart => out(&quart, t),
            InOutQuart => in_out(&quart, t),
            InQuint => quint(t),
            OutQuint => out(&quint, t),
            InOutQuint => in_out(&quint, t),
            InExpo => expo(t),
            OutExpo => out(&expo, t),
            InOutExpo => in_out(&expo, t),
            InCirc => circ(t),
            OutCirc => out(&circ, t),
            InOutCirc => in_out(&circ, t),
            InBack => back(t),
            OutBack => out(&back, t),
            InOutBack => in_out(&back, t),
            InElastic => elastic(t),
            OutElastic => out(&elastic, t),
            InOutElastic => in_out(&elastic, t),
            InBounce => bounce(t),
            OutBounce => out(&bounce, t),
            InOutBounce => in_out(&bounce, t),
        }
    }

    /// `EasingTypeTransformer.apply(start, end, lerp)`: no easing argument.
    pub fn apply_simple(self, start: f32, end: f32, t: f32) -> f32 {
        lerp(start, end, self.transform(None, t))
    }

    /// `EasingType.apply(env, start, end, length, lerp, args)`.
    pub fn apply(self, rt: &mut Runtime, start: f32, end: f32, length: f32, t: f32, args: &[Exprs]) -> f32 {
        match self {
            Bezier => bezier(rt, start, end, length, t, args),
            CatmullRom => {
                if t >= 1.0 {
                    return end;
                }
                if t.is_nan() {
                    return start;
                }
                if args.len() < 2 {
                    return lerp(start, end, t);
                }
                let p0 = rt.eval(&args[0]);
                let p3 = rt.eval(&args[1]);
                catmull_rom(t, p0, start, end, p3)
            }
            _ => {
                if t >= 1.0 {
                    return end;
                }
                if t.is_nan() {
                    return start;
                }
                let value = args.first().map(|a| rt.eval(a));
                lerp(start, end, self.transform(value, t))
            }
        }
    }
}

/// `EasingType.step`, MIT, Copyright (c) 2015 Boris Chumichev.
fn step(value: Option<f32>, t: f32) -> f32 {
    let n2 = value.unwrap_or(2.0);
    // Java throws for fewer than 2 steps; the keyframe then fails to play. Two is the closest.
    let steps = n2.max(2.0) as i32;
    if t < 0.0 {
        return 0.0;
    }
    let step_length = 1.0 / steps as f32;
    let last = (steps - 1) as f32 * step_length;
    if t > last {
        return last;
    }
    let (mut left, mut right) = (0, steps - 1);
    while right - left != 1 {
        let test = left + (right - left) / 2;
        if t >= test as f32 * step_length {
            left = test;
        } else {
            right = test;
        }
    }
    left as f32 * step_length
}

fn catmull_rom(d: f32, p0: f32, p1: f32, p2: f32, p3: f32) -> f32 {
    0.5 * (2.0 * p1 + (p2 - p0) * d + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * d * d + (3.0 * p1 - p0 - 3.0 * p2 + p3) * d * d * d)
}

fn cubic_bezier(t: f32, p0: f32, p1: f32, p2: f32, p3: f32) -> f32 {
    let k = 1.0 - t;
    k * k * k * p0 + 3.0 * k * k * t * p1 + 3.0 * (1.0 - t) * t * t * p2 + t * t * t * p3
}

/// `BezierEasing.apply`: Blockbench's handles, the curve sampled at 200 points.
fn bezier(rt: &mut Runtime, start: f32, end: f32, length: f32, t: f32, args: &[Exprs]) -> f32 {
    if t >= 1.0 {
        return end;
    }
    if t.is_nan() || t == 0.0 {
        return start;
    }
    if args.is_empty() {
        return lerp(start, end, t);
    }
    let mut left_value = rt.eval(&args[0]);
    let left_time = args.get(1).map_or(0.0, |a| rt.eval(a));
    let (mut right_value, right_time) = if args.len() > 3 { (rt.eval(&args[2]), rt.eval(&args[3])) } else { (0.0, 0.1) };
    let length = length / 20.0;
    let mut before = right_time / length;
    let mut after = left_time / length;
    if !(0.0..=1.0).contains(&before) {
        let unclamped = before;
        before = before.clamp(0.0, 1.0);
        right_value /= 1.0 + (before - unclamped).abs();
    }
    if after > 0.0 || after < -1.0 {
        let unclamped = after;
        after = after.clamp(-1.0, 0.0);
        left_value /= 1.0 + (after - unclamped).abs();
    }
    let p0 = (0.0, start);
    let p1 = (before, start + right_value);
    let p2 = (after + 1.0, end + left_value);
    let p3 = (1.0, end);
    const DIVISIONS: usize = 200;
    let points: Vec<(f32, f32)> = (0..=DIVISIONS)
        .map(|d| {
            let s = d as f32 / DIVISIONS as f32;
            (cubic_bezier(s, p0.0, p1.0, p2.0, p3.0), cubic_bezier(s, p0.1, p1.1, p2.1, p3.1))
        })
        .collect();
    let mut closest = 0usize;
    let mut diff = f32::INFINITY;
    for (i, p) in points.iter().enumerate() {
        let d = (p.0 - t).abs();
        if d < diff {
            diff = d;
            closest = i;
        }
    }
    let mut second = (0.0f32, 0.0f32);
    diff = f32::INFINITY;
    for p in &points[..closest] {
        let d = (p.0 - t).abs();
        if d < diff {
            diff = d;
            second = *p;
        }
    }
    let c = points[closest];
    lerp(c.1, second.1, lerp(c.0, second.0, t).clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_ids() {
        assert_eq!(Easing::from_legacy("inQuad"), InQuad);
        assert_eq!(Easing::from_legacy("EASEINOUTQUAD"), InOutQuad);
        assert_eq!(Easing::from_legacy("InOutSine"), InOutSine);
        assert_eq!(Easing::from_legacy("CONSTANT"), Constant);
        assert_eq!(Easing::from_id(38), Bezier);
        assert_eq!(Easing::from_id(99), Linear);
        assert_eq!(Step.id(), 37);
    }

    #[test]
    fn curves() {
        assert!((InOutSine.apply_simple(0.0, 10.0, 0.5) - 5.0).abs() < 1e-5);
        assert_eq!(Constant.apply_simple(1.0, 3.0, 0.9), 1.0);
        assert!((InQuad.apply_simple(0.0, 1.0, 0.5) - 0.25).abs() < 1e-6);
        assert!((OutQuad.apply_simple(0.0, 1.0, 0.5) - 0.75).abs() < 1e-6);
        assert_eq!(step(None, 0.6), 0.5);
    }
}
