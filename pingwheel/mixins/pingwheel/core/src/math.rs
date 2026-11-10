//! `MathUtils` and the numbers `PingView` and the renderers work out.

/// `MathUtils.calculateAngleRectIntersection`: where a ray from the middle of the rectangle
/// `left_top`..`right_bottom` at `angle` (radians, y down) leaves it.
pub fn angle_rect_intersection(angle: f32, left_top: [f32; 2], right_bottom: [f32; 2]) -> [f32; 2] {
    let (cos, sin) = (f64::from(angle).cos(), f64::from(angle).sin());
    let width = right_bottom[0] - left_top[0];
    let height = right_bottom[1] - left_top[1];
    // `direction.multiply(1 / width, 1 / height)` in floats, then compared as doubles.
    let dir = [cos * f64::from(1.0 / width), sin * f64::from(1.0 / height)];
    let (cx, cy) = (f64::from(width * 0.5), f64::from(height * 0.5));
    if dir[0].abs() < dir[1].abs() {
        if dir[1] < 0.0 {
            let t = -cy / sin;
            return [(cx + t * cos) as f32 + left_top[0], left_top[1]];
        }
        let t = cy / sin;
        return [(cx + t * cos) as f32 + left_top[0], right_bottom[1]];
    }
    if dir[0] < 0.0 {
        let t = -cx / cos;
        return [left_top[0], (cy + t * sin) as f32 + left_top[1]];
    }
    let t = cx / cos;
    [right_bottom[0], (cy + t * sin) as f32 + left_top[1]]
}

/// `PingView.calculateScale`: `max(1, 2 / distance^0.3) * 0.5 * pingSize / 100`.
pub fn ping_scale(distance: f64, ping_size: i32) -> f32 {
    let scale = 2.0 / distance.powf(0.3);
    (scale.max(1.0) as f32) * 0.5 * (ping_size as f32 / 100.0)
}

/// `"%,.1f".formatted(distance)`: one decimal, thousands grouped, in the locale's way (`,` and
/// `.` in English; a no-break space and `,` in Russian and most of Europe).
pub fn format_distance(distance: f64, group: &str, decimal: &str) -> String {
    // `%.1f` rounds half up.
    let tenths = (distance.max(0.0) * 10.0 + 0.5).floor();
    let whole = (tenths / 10.0).trunc() as u64;
    let frac = (tenths - whole as f64 * 10.0).round() as u64;
    let digits = whole.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push_str(group);
        }
        out.push(c);
    }
    format!("{out}{decimal}{frac}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges() {
        let (lt, rb) = ([5.0, 5.0], [315.0, 120.0]);
        // Straight right, left, up, down from the middle.
        assert_eq!(angle_rect_intersection(0.0, lt, rb), [315.0, 62.5]);
        let left = angle_rect_intersection(std::f32::consts::PI, lt, rb);
        assert_eq!(left[0], 5.0);
        assert!((left[1] - 62.5).abs() < 1e-3);
        let up = angle_rect_intersection(-std::f32::consts::FRAC_PI_2, lt, rb);
        assert_eq!(up[1], 5.0);
        assert!((up[0] - 160.0).abs() < 1e-3);
        let down = angle_rect_intersection(std::f32::consts::FRAC_PI_2, lt, rb);
        assert_eq!(down[1], 120.0);
        // 45 degrees on a wide rectangle meets the bottom edge.
        let d = angle_rect_intersection(std::f32::consts::FRAC_PI_4, lt, rb);
        assert_eq!(d[1], 120.0);
        assert!((d[0] - (155.0 + 5.0 + 57.5)).abs() < 1e-3);
    }

    #[test]
    fn scale() {
        // Close pings are drawn bigger, far ones at half size.
        assert_eq!(ping_scale(1000.0, 100), 0.5);
        assert!((ping_scale(1.0, 100) - 1.0).abs() < 1e-6);
        assert_eq!(ping_scale(1000.0, 200), 1.0);
    }

    #[test]
    fn distances() {
        assert_eq!(format_distance(2.14, ",", "."), "2.1");
        assert_eq!(format_distance(174.25, ",", "."), "174.3");
        assert_eq!(format_distance(1234.56, ",", "."), "1,234.6");
        assert_eq!(format_distance(1234567.0, "\u{a0}", ","), "1\u{a0}234\u{a0}567,0");
        assert_eq!(format_distance(0.04, ",", "."), "0.0");
        assert_eq!(format_distance(9.96, ",", "."), "10.0");
    }
}
