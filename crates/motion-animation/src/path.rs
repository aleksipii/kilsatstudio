//! Routes of a move between two points: a straight line, a circular arc or a
//! Bézier curve, all travelled at constant speed along the curve.
//!
//! Points are evaluated as pure functions of progress, so a seek gives the
//! same position as frame-by-frame playback.
use crate::Vec2;

/// Largest arc bulge, as a fraction of the straight distance (a half circle).
pub const MAX_ARC: f64 = 0.5;

/// Segments of the arc-length table used for Bézier curves.
const BEZIER_SEGMENTS: usize = 64;

/// The way a move travels from `from` to `to`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum MovePath {
    /// A straight line (the default).
    #[default]
    Line,
    /// A circular arc whose bulge (sagitta) is this fraction of the straight
    /// distance, in `-0.5..=0.5`. Positive bulges toward screen up (`-y`); for
    /// a vertical chord, toward screen right (`+x`).
    Arc(f64),
    /// A quadratic Bézier curve with one control point.
    Quadratic(Vec2),
    /// A cubic Bézier curve with two control points.
    Cubic(Vec2, Vec2),
}

impl MovePath {
    /// Finite control points and an arc bulge within [`MAX_ARC`].
    pub fn is_valid(&self) -> bool {
        let ok = |v: Vec2| v.x.is_finite() && v.y.is_finite();
        match *self {
            Self::Line => true,
            Self::Arc(h) => h.is_finite() && h.abs() <= MAX_ARC,
            Self::Quadratic(c) => ok(c),
            Self::Cubic(a, b) => ok(a) && ok(b),
        }
    }

    /// Point after the fraction `t` in `[0, 1]` of the route's length, so
    /// equal steps of `t` cover equal distances on every kind of route.
    pub fn point(&self, from: Vec2, to: Vec2, t: f64) -> Vec2 {
        match *self {
            Self::Line => lerp(from, to, t),
            Self::Arc(h) => arc_point(from, to, h, t),
            Self::Quadratic(_) | Self::Cubic(..) => {
                let table = self.bezier_table(from, to);
                let total = table[BEZIER_SEGMENTS];
                if total == 0.0 {
                    return self.bezier(from, to, t);
                }
                let target = t.clamp(0.0, 1.0) * total;
                // First table entry at or beyond the target distance.
                let next = table
                    .partition_point(|&d| d < target)
                    .clamp(1, BEZIER_SEGMENTS);
                let (a, b) = (table[next - 1], table[next]);
                let local = if b > a { (target - a) / (b - a) } else { 0.0 };
                let u = ((next - 1) as f64 + local) / BEZIER_SEGMENTS as f64;
                self.bezier(from, to, u)
            }
        }
    }

    /// Length of the route from `from` to `to`; exact for lines and arcs,
    /// within a small fraction of a pixel for Bézier curves.
    pub fn length(&self, from: Vec2, to: Vec2) -> f64 {
        let chord = (to.x - from.x).hypot(to.y - from.y);
        match *self {
            Self::Line => chord,
            Self::Arc(h) => chord * arc_length_ratio(h),
            Self::Quadratic(_) | Self::Cubic(..) => self.bezier_table(from, to)[BEZIER_SEGMENTS],
        }
    }

    /// Raw Bézier point at curve parameter `u`.
    fn bezier(&self, from: Vec2, to: Vec2, u: f64) -> Vec2 {
        let v = 1.0 - u;
        match *self {
            Self::Quadratic(c) => {
                let (a, b, d) = (v * v, 2.0 * v * u, u * u);
                Vec2::new(
                    a * from.x + b * c.x + d * to.x,
                    a * from.y + b * c.y + d * to.y,
                )
            }
            Self::Cubic(c1, c2) => {
                let (a, b, c, d) = (v * v * v, 3.0 * v * v * u, 3.0 * v * u * u, u * u * u);
                Vec2::new(
                    a * from.x + b * c1.x + c * c2.x + d * to.x,
                    a * from.y + b * c1.y + c * c2.y + d * to.y,
                )
            }
            Self::Line | Self::Arc(_) => lerp(from, to, u),
        }
    }

    /// Cumulative length at each of the evenly spaced curve parameters.
    fn bezier_table(&self, from: Vec2, to: Vec2) -> [f64; BEZIER_SEGMENTS + 1] {
        let mut table = [0.0; BEZIER_SEGMENTS + 1];
        let mut previous = from;
        for (i, slot) in table.iter_mut().enumerate().skip(1) {
            let point = self.bezier(from, to, i as f64 / BEZIER_SEGMENTS as f64);
            *slot = (point.x - previous.x).hypot(point.y - previous.y);
            previous = point;
        }
        for i in 1..=BEZIER_SEGMENTS {
            table[i] += table[i - 1];
        }
        table
    }
}

/// Arc length over straight distance for an arc of bulge `h` (sagitta over
/// chord): 1 for a line, π/2 for a half circle.
pub fn arc_length_ratio(h: f64) -> f64 {
    let h = if h.is_finite() {
        h.abs().min(MAX_ARC)
    } else {
        0.0
    };
    if h == 0.0 {
        return 1.0;
    }
    // Chord 1: radius r = (h² + 1/4) / 2h, sweep 4·atan(2h).
    let radius = (h * h + 0.25) / (2.0 * h);
    radius * 4.0 * (2.0 * h).atan()
}

fn lerp(from: Vec2, to: Vec2, t: f64) -> Vec2 {
    Vec2::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t)
}

/// Circular arc through `from` and `to` with sagitta `bulge * |to - from|`.
fn arc_point(from: Vec2, to: Vec2, bulge: f64, t: f64) -> Vec2 {
    let (cx, cy) = (to.x - from.x, to.y - from.y);
    let length = cx.hypot(cy);
    if length == 0.0 || bulge == 0.0 {
        return lerp(from, to, t);
    }
    // Unit normal toward screen up, or screen right for a vertical chord.
    let (mut nx, mut ny) = (cy / length, -cx / length);
    if ny > 0.0 || (ny == 0.0 && nx < 0.0) {
        (nx, ny) = (-nx, -ny);
    }
    let sagitta = bulge * length;
    let half = length / 2.0;
    let radius = (sagitta * sagitta + half * half) / (2.0 * sagitta.abs());
    // The centre lies on the far side of the chord from the bulge.
    let side = sagitta.signum();
    let offset = side * (radius - sagitta.abs());
    let centre = Vec2::new(
        from.x + cx / 2.0 - nx * offset,
        from.y + cy / 2.0 - ny * offset,
    );
    let sweep = 4.0 * (sagitta.abs() / half).atan();
    let (sx, sy) = (from.x - centre.x, from.y - centre.y);
    // Turn the way whose midpoint lies on the bulge side of the chord.
    let (sin, cos) = (sweep / 2.0).sin_cos();
    let (mid_x, mid_y) = (sx * cos - sy * sin, sx * sin + sy * cos);
    let toward_bulge =
        (mid_x + centre.x - from.x - cx / 2.0) * nx + (mid_y + centre.y - from.y - cy / 2.0) * ny;
    let direction = if toward_bulge * side >= 0.0 {
        1.0
    } else {
        -1.0
    };
    let (sin, cos) = (direction * sweep * t).sin_cos();
    Vec2::new(
        centre.x + sx * cos - sy * sin,
        centre.y + sx * sin + sy * cos,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const FROM: Vec2 = Vec2::new(0.0, 0.0);

    fn step_lengths(path: MovePath, to: Vec2) -> Vec<f64> {
        // Each step is measured along the curve with fine sub-steps, so a
        // tight bend does not shorten it to its chord.
        (0..40)
            .map(|i| {
                (0..25)
                    .map(|j| {
                        let t = (i * 25 + j) as f64 / 1000.0;
                        let a = path.point(FROM, to, t);
                        let b = path.point(FROM, to, t + 0.001);
                        (b.x - a.x).hypot(b.y - a.y)
                    })
                    .sum()
            })
            .collect()
    }

    #[test]
    fn every_route_moves_at_constant_speed() {
        let to = Vec2::new(400.0, 0.0);
        for path in [
            MovePath::Line,
            MovePath::Arc(0.35),
            MovePath::Quadratic(Vec2::new(50.0, -300.0)),
            MovePath::Cubic(Vec2::new(0.0, -300.0), Vec2::new(380.0, 100.0)),
        ] {
            let steps = step_lengths(path, to);
            let mean = steps.iter().sum::<f64>() / steps.len() as f64;
            for step in &steps {
                assert!(
                    (step / mean - 1.0).abs() < 0.01,
                    "{path:?}: {step} vs {mean}"
                );
            }
            assert!((path.length(FROM, to) / (mean * 40.0) - 1.0).abs() < 1e-3);
        }
    }

    #[test]
    fn arc_length_ratio_matches_known_shapes() {
        assert_eq!(arc_length_ratio(0.0), 1.0);
        assert!((arc_length_ratio(0.5) - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert_eq!(arc_length_ratio(-0.3), arc_length_ratio(0.3));
        assert_eq!(arc_length_ratio(f64::NAN), 1.0);
        let to = Vec2::new(300.0, 0.0);
        assert!((MovePath::Arc(0.5).length(FROM, to) - 150.0 * std::f64::consts::PI).abs() < 1e-9);
    }

    #[test]
    fn bezier_ends_and_degenerate_curves_are_safe() {
        let to = Vec2::new(10.0, 5.0);
        let cubic = MovePath::Cubic(Vec2::new(3.0, -4.0), Vec2::new(8.0, 9.0));
        for t in [0.0, 1.0] {
            let p = cubic.point(FROM, to, t);
            let expected = if t == 0.0 { FROM } else { to };
            assert!((p.x - expected.x).abs() < 1e-9 && (p.y - expected.y).abs() < 1e-9);
        }
        let still = MovePath::Quadratic(FROM);
        assert_eq!(still.point(FROM, FROM, 0.4), FROM);
        assert_eq!(still.length(FROM, FROM), 0.0);
        assert!(!MovePath::Arc(0.6).is_valid());
        assert!(!MovePath::Cubic(FROM, Vec2::new(f64::INFINITY, 0.0)).is_valid());
    }
}
