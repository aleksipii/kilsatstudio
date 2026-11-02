//! Resolved tracks (TrackSpec, ORK-G1-02).
//!
//! Authored spans give each `(entity, property)` pair a target: `Abs` (an absolute
//! value), `Rel` (a delta from the value when the span starts) or `Revert` (back to
//! the track's base value). Resolution runs once, when a script is prepared, and
//! turns every span into a concrete `{from, to}` interval. Evaluation then only
//! finds the active span and interpolates, so `value_at(t)` is a pure function of
//! `t` and never depends on which times were evaluated before.
//!
//! Ordering and overlap:
//! - Spans are ordered by start time (`f64::total_cmp`), then by written order.
//! - The span with the latest start at or before `t` is active: a later start wins.
//!   It interrupts an earlier span, whose value at that instant becomes `from`.
//! - On equal starts the later-written span wins; the earlier one has no effect.
//! - A finished span holds its `to` value until the next span starts.
//!
//! Resolved tracks are evaluated state. They are not serialized as a document
//! format; the authored form stays in `motion-script` (A02).
use crate::time::seconds_to_sample;
use crate::{Error, Vec2};
use motion_animation::MoveEase;

/// Upper bound on spans per track, so resolution work stays bounded.
pub const MAX_TRACK_SPANS: usize = 4096;

/// What an authored span moves towards.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrackTarget {
    /// Absolute value.
    Abs(Vec2),
    /// Delta from the track value at the span's start (after earlier spans).
    Rel(Vec2),
    /// The track's base value, explicitly.
    Revert,
}

/// The way a span travels from `from` to `to`.
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
    fn finite(&self) -> bool {
        let ok = |v: Vec2| v.x.is_finite() && v.y.is_finite();
        match *self {
            Self::Line => true,
            Self::Arc(h) => h.is_finite() && h.abs() <= 0.5,
            Self::Quadratic(c) => ok(c),
            Self::Cubic(a, b) => ok(a) && ok(b),
        }
    }

    /// Point at curve parameter `t` in `[0, 1]` between `from` and `to`.
    /// Arcs travel at constant speed; Bézier curves speed up where control
    /// points are far apart, as with any Bézier parameterization.
    pub fn point(&self, from: Vec2, to: Vec2, t: f64) -> Vec2 {
        let lerp = |a: f64, b: f64| a + (b - a) * t;
        match *self {
            Self::Line => Vec2::new(lerp(from.x, to.x), lerp(from.y, to.y)),
            Self::Arc(h) => arc_point(from, to, h, t),
            Self::Quadratic(c) => {
                let u = 1.0 - t;
                let (a, b, d) = (u * u, 2.0 * u * t, t * t);
                Vec2::new(
                    a * from.x + b * c.x + d * to.x,
                    a * from.y + b * c.y + d * to.y,
                )
            }
            Self::Cubic(c1, c2) => {
                let u = 1.0 - t;
                let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                Vec2::new(
                    a * from.x + b * c1.x + c * c2.x + d * to.x,
                    a * from.y + b * c1.y + c * c2.y + d * to.y,
                )
            }
        }
    }
}

/// Circular arc through `from` and `to` with sagitta `bulge * |to - from|`.
fn arc_point(from: Vec2, to: Vec2, bulge: f64, t: f64) -> Vec2 {
    let (cx, cy) = (to.x - from.x, to.y - from.y);
    let length = cx.hypot(cy);
    if length == 0.0 || bulge == 0.0 {
        return MovePath::Line.point(from, to, t);
    }
    // Unit normal toward screen up, or screen right for a vertical chord.
    let (mut nx, mut ny) = (cy / length, -cx / length);
    if ny > 0.0 || (ny == 0.0 && nx < 0.0) {
        (nx, ny) = (-nx, -ny);
    }
    let sagitta = bulge * length;
    let half = length / 2.0;
    let radius = (sagitta * sagitta + half * half) / (2.0 * sagitta.abs());
    // Centre lies on the far side of the chord from the bulge.
    let side = sagitta.signum();
    let offset = side * (radius - sagitta.abs());
    let centre = Vec2::new(
        from.x + cx / 2.0 - nx * offset,
        from.y + cy / 2.0 - ny * offset,
    );
    let sweep = 4.0 * (sagitta.abs() / half).atan();
    let (sx, sy) = (from.x - centre.x, from.y - centre.y);
    // Turn the way that passes the bulge: the arc's midpoint lies on the
    // normal's side given by `side`.
    let (sin, cos) = (sweep / 2.0).sin_cos();
    let mid_x = sx * cos - sy * sin;
    let mid_y = sx * sin + sy * cos;
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

/// One authored change of a single `(entity, property)` value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AuthoredSpan {
    /// Written order, used to break ties between equal starts.
    pub order: usize,
    /// Scene-local start in seconds.
    pub at: f64,
    /// Positive length in seconds.
    pub duration: f64,
    pub target: TrackTarget,
    /// Speed profile from `from` to `to`.
    pub ease: MoveEase,
    /// Route from `from` to `to`.
    pub path: MovePath,
}

/// A concrete interval: playback only interpolates `from → to`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedSpan {
    pub order: usize,
    pub at: f64,
    pub duration: f64,
    /// Start and end on the shared 24 kHz clock, nearest sample.
    pub start_sample: u64,
    pub end_sample: u64,
    pub from: Vec2,
    pub to: Vec2,
    pub ease: MoveEase,
    pub path: MovePath,
}

impl ResolvedSpan {
    /// Interpolation progress in `[0, 1]`; exactly 1 from the span's end on,
    /// so a span that touches its successor always hands over its `to` value.
    pub fn progress(&self, time: f64) -> f64 {
        if time >= self.at + self.duration {
            1.0
        } else {
            ((time - self.at) / self.duration).clamp(0.0, 1.0)
        }
    }
    /// Fraction of the distance covered at `time`, after easing.
    pub fn distance_progress(&self, time: f64) -> f64 {
        self.ease.apply(self.progress(time))
    }
    fn value(&self, time: f64) -> Vec2 {
        let progress = self.distance_progress(time);
        // A finished span holds exactly `to`; `from + (to - from) * 1` can be off by one ulp.
        if progress == 1.0 {
            return self.to;
        }
        if progress == 0.0 {
            return self.from;
        }
        self.path.point(self.from, self.to, progress)
    }
}

/// All spans of one `(entity, property)` pair, ordered and resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedTrack {
    base: Vec2,
    spans: Vec<ResolvedSpan>,
}

impl ResolvedTrack {
    /// Resolve authored spans against `base`. Times must be finite, starts
    /// non-negative and durations positive; targets must be finite.
    pub fn resolve(base: Vec2, authored: &[AuthoredSpan]) -> Result<Self, Error> {
        let finite = |v: Vec2| v.x.is_finite() && v.y.is_finite();
        if !finite(base) || authored.len() > MAX_TRACK_SPANS {
            return Err(Error::InvalidAnimation(
                "track base must be finite and spans at most MAX_TRACK_SPANS".into(),
            ));
        }
        let mut sorted = authored.to_vec();
        sorted.sort_by(|a, b| a.at.total_cmp(&b.at).then(a.order.cmp(&b.order)));
        let mut track = Self {
            base,
            spans: Vec::with_capacity(sorted.len()),
        };
        for span in sorted {
            let end = span.at + span.duration;
            let target_ok = match span.target {
                TrackTarget::Abs(v) | TrackTarget::Rel(v) => finite(v),
                TrackTarget::Revert => true,
            };
            if !span.at.is_finite()
                || span.at < 0.0
                || !span.duration.is_finite()
                || span.duration <= 0.0
                || !end.is_finite()
                || !target_ok
                || !span.path.finite()
            {
                return Err(Error::InvalidAnimation(format!(
                    "track span {}: start must be finite and non-negative, duration finite and positive, target finite",
                    span.order
                )));
            }
            let from = track.value_at(span.at);
            let to = match span.target {
                TrackTarget::Abs(value) => value,
                TrackTarget::Rel(delta) => Vec2::new(from.x + delta.x, from.y + delta.y),
                TrackTarget::Revert => base,
            };
            if !finite(to) {
                return Err(Error::InvalidAnimation(format!(
                    "track span {}: resolved value is not finite",
                    span.order
                )));
            }
            track.spans.push(ResolvedSpan {
                order: span.order,
                at: span.at,
                duration: span.duration,
                start_sample: seconds_to_sample(span.at)?,
                end_sample: seconds_to_sample(end)?,
                from,
                to,
                ease: span.ease,
                path: span.path,
            });
        }
        Ok(track)
    }

    pub fn base(&self) -> Vec2 {
        self.base
    }

    /// Spans in evaluation order.
    pub fn spans(&self) -> &[ResolvedSpan] {
        &self.spans
    }

    /// The span with the latest start at or before `time` (ties: later written).
    pub fn active(&self, time: f64) -> Option<&ResolvedSpan> {
        let next = self.spans.partition_point(|span| span.at <= time);
        next.checked_sub(1).map(|index| &self.spans[index])
    }

    /// Value at scene-local `time`; the base before the first span.
    pub fn value_at(&self, time: f64) -> Vec2 {
        self.active(time).map_or(self.base, |span| span.value(time))
    }
}

/// Resolve `Event::Move` for one actor: each move is an `Abs` target that starts
/// from where the previous move left the actor. Validation rejects overlapping
/// moves for one actor, so interruption never happens for authored moves.
pub fn resolve_moves(
    base: Vec2,
    events: &[motion_script::Event],
    actor: &str,
) -> Result<ResolvedTrack, Error> {
    let spans = events
        .iter()
        .enumerate()
        .filter_map(|(order, event)| match event {
            motion_script::Event::Move {
                at,
                actor: id,
                x,
                y,
                duration,
                ease,
                arc,
                bezier,
                ..
            } if id == actor => Some(AuthoredSpan {
                order,
                at: *at,
                duration: *duration,
                target: TrackTarget::Abs(Vec2::new(*x, *y)),
                // Validation guarantees a known ease.
                ease: ease
                    .as_deref()
                    .and_then(MoveEase::parse)
                    .unwrap_or_default(),
                path: move_path(*arc, bezier.as_deref()),
            }),
            _ => None,
        })
        .collect::<Vec<_>>();
    ResolvedTrack::resolve(base, &spans)
}

/// The authored route of a move; validation allows at most one of the two.
fn move_path(arc: Option<f64>, bezier: Option<&[motion_script::PathPoint]>) -> MovePath {
    let point = |p: &motion_script::PathPoint| Vec2::new(p.x, p.y);
    match (arc, bezier) {
        (_, Some([c])) => MovePath::Quadratic(point(c)),
        (_, Some([c1, c2])) => MovePath::Cubic(point(c1), point(c2)),
        (Some(h), _) if h != 0.0 => MovePath::Arc(h),
        _ => MovePath::Line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(order: usize, at: f64, duration: f64, target: TrackTarget) -> AuthoredSpan {
        AuthoredSpan {
            order,
            at,
            duration,
            target,
            ease: MoveEase::Linear,
            path: MovePath::Line,
        }
    }
    const BASE: Vec2 = Vec2::new(300.0, 400.0);

    #[test]
    fn rel_resolves_to_cumulative_from_to() {
        // The report's example: Rel(100, 0) from (300, 400) → {from (300,400), to (400,400)}.
        let track = ResolvedTrack::resolve(
            BASE,
            &[
                span(0, 0.0, 1.0, TrackTarget::Rel(Vec2::new(100.0, 0.0))),
                span(1, 2.0, 1.0, TrackTarget::Rel(Vec2::new(0.0, -50.0))),
            ],
        )
        .unwrap();
        let spans = track.spans();
        assert_eq!(
            (spans[0].from, spans[0].to),
            (BASE, Vec2::new(400.0, 400.0))
        );
        assert_eq!(
            (spans[1].from, spans[1].to),
            (Vec2::new(400.0, 400.0), Vec2::new(400.0, 350.0))
        );
        assert_eq!(
            (spans[1].start_sample, spans[1].end_sample),
            (48_000, 72_000)
        );
        assert_eq!(track.value_at(0.5), Vec2::new(350.0, 400.0));
        assert_eq!(track.value_at(1.5), Vec2::new(400.0, 400.0));
        assert_eq!(track.value_at(10.0), Vec2::new(400.0, 350.0));
    }

    #[test]
    fn abs_and_revert_targets() {
        let track = ResolvedTrack::resolve(
            BASE,
            &[
                span(0, 0.0, 1.0, TrackTarget::Abs(Vec2::new(0.0, 0.0))),
                span(1, 1.0, 1.0, TrackTarget::Rel(Vec2::new(10.0, 10.0))),
                span(2, 3.0, 2.0, TrackTarget::Revert),
            ],
        )
        .unwrap();
        assert_eq!(track.value_at(1.0), Vec2::new(0.0, 0.0));
        assert_eq!(track.value_at(2.5), Vec2::new(10.0, 10.0));
        // Revert goes to the base, not to the value before the Rel span.
        assert_eq!(track.spans()[2].to, BASE);
        assert_eq!(track.value_at(4.0), Vec2::new(155.0, 205.0));
        assert_eq!(track.value_at(9.0), BASE);
        assert_eq!(track.value_at(0.0), Vec2::new(300.0, 400.0));
    }

    #[test]
    fn later_start_interrupts_and_written_order_breaks_ties() {
        let track = ResolvedTrack::resolve(
            Vec2::new(0.0, 0.0),
            &[
                // Written order is intentionally not time order.
                span(2, 1.0, 2.0, TrackTarget::Rel(Vec2::new(0.0, 10.0))),
                span(0, 0.0, 2.0, TrackTarget::Abs(Vec2::new(100.0, 0.0))),
                span(1, 1.0, 2.0, TrackTarget::Abs(Vec2::new(-1.0, -1.0))),
            ],
        )
        .unwrap();
        let orders: Vec<_> = track.spans().iter().map(|s| s.order).collect();
        assert_eq!(orders, [0, 1, 2]);
        // At 1.0 span 0 is halfway; spans 1 and 2 start there, 2 is written later and wins.
        assert_eq!(track.active(1.0).unwrap().order, 2);
        assert_eq!(track.spans()[2].from, Vec2::new(50.0, 0.0));
        assert_eq!(track.spans()[2].to, Vec2::new(50.0, 10.0));
        assert_eq!(track.value_at(0.999), track.spans()[0].value(0.999));
        assert_eq!(track.value_at(5.0), Vec2::new(50.0, 10.0));
        // Shuffling the input slice does not change the result.
        let mut reversed = [
            span(0, 0.0, 2.0, TrackTarget::Abs(Vec2::new(100.0, 0.0))),
            span(1, 1.0, 2.0, TrackTarget::Abs(Vec2::new(-1.0, -1.0))),
            span(2, 1.0, 2.0, TrackTarget::Rel(Vec2::new(0.0, 10.0))),
        ];
        reversed.reverse();
        assert_eq!(
            ResolvedTrack::resolve(Vec2::new(0.0, 0.0), &reversed).unwrap(),
            track
        );
    }

    #[test]
    fn eased_spans_start_and_stop_gently_and_end_exactly() {
        let mut eased = span(0, 1.0, 2.0, TrackTarget::Abs(Vec2::new(500.0, 400.0)));
        eased.ease = MoveEase::InOut;
        let track = ResolvedTrack::resolve(BASE, &[eased]).unwrap();
        let linear = ResolvedTrack::resolve(
            BASE,
            &[span(0, 1.0, 2.0, TrackTarget::Abs(Vec2::new(500.0, 400.0)))],
        )
        .unwrap();
        // Slower than linear early, ahead in the middle, exact at the ends.
        assert!(track.value_at(1.1).x < linear.value_at(1.1).x);
        assert!((track.value_at(2.0).x - 400.0).abs() < 1e-9);
        assert_eq!(track.value_at(3.0), Vec2::new(500.0, 400.0));
        assert_eq!(track.value_at(1.0), BASE);
        // Monotonic, so the actor never steps back.
        let mut last = BASE.x;
        for i in 0..=200 {
            let x = track.value_at(1.0 + i as f64 / 100.0).x;
            assert!(x >= last - 1e-9);
            last = x;
        }
    }

    #[test]
    fn invalid_spans_are_typed_errors() {
        let ok = TrackTarget::Abs(Vec2::new(1.0, 1.0));
        for bad in [
            span(0, -0.1, 1.0, ok),
            span(0, f64::NAN, 1.0, ok),
            span(0, 0.0, 0.0, ok),
            span(0, 0.0, f64::INFINITY, ok),
            span(0, f64::MAX, f64::MAX, ok),
            span(0, 0.0, 1.0, TrackTarget::Abs(Vec2::new(f64::NAN, 0.0))),
            span(0, 0.0, 1.0, TrackTarget::Rel(Vec2::new(f64::MAX, 0.0))),
        ] {
            let base = Vec2::new(f64::MAX, 0.0);
            assert!(ResolvedTrack::resolve(base, &[bad]).is_err(), "{bad:?}");
        }
        assert!(ResolvedTrack::resolve(Vec2::new(f64::NAN, 0.0), &[]).is_err());
        let many = vec![span(0, 0.0, 1.0, ok); MAX_TRACK_SPANS + 1];
        assert!(ResolvedTrack::resolve(BASE, &many).is_err());
    }

    fn path_span(path: MovePath, to: Vec2) -> ResolvedTrack {
        let mut authored = span(0, 0.0, 1.0, TrackTarget::Abs(to));
        authored.path = path;
        ResolvedTrack::resolve(BASE, &[authored]).unwrap()
    }

    #[test]
    fn arcs_bulge_up_by_the_sagitta_and_keep_exact_ends() {
        let right = Vec2::new(500.0, 400.0);
        for to in [right, Vec2::new(100.0, 400.0)] {
            let track = path_span(MovePath::Arc(0.25), to);
            assert_eq!(track.value_at(0.0), BASE);
            assert_eq!(track.value_at(1.0), to);
            // Chord 200 px, sagitta 0.25 * 200 = 50 px above the chord's middle.
            let apex = track.value_at(0.5);
            assert!(
                (apex.x - 300.0 - (to.x - 300.0) / 2.0).abs() < 1e-9,
                "{apex:?}"
            );
            assert!((apex.y - 350.0).abs() < 1e-9, "{apex:?}");
            // Every sample stays on the circle and above the chord.
            for i in 1..10 {
                assert!(track.value_at(i as f64 / 10.0).y < 400.0);
            }
        }
        let down = path_span(MovePath::Arc(-0.5), right).value_at(0.5);
        assert!((down.y - 500.0).abs() < 1e-9, "half circle below: {down:?}");
        let vertical = path_span(MovePath::Arc(0.25), Vec2::new(300.0, 200.0)).value_at(0.5);
        assert!(
            (vertical.x - 350.0).abs() < 1e-9,
            "vertical bulges right: {vertical:?}"
        );
    }

    #[test]
    fn arc_speed_is_constant_along_the_curve() {
        let track = path_span(MovePath::Arc(0.4), Vec2::new(700.0, 300.0));
        let step = |t: f64| {
            let (a, b) = (track.value_at(t), track.value_at(t + 0.05));
            (b.x - a.x).hypot(b.y - a.y)
        };
        let first = step(0.0);
        for i in 1..19 {
            assert!((step(i as f64 * 0.05) - first).abs() < 1e-6);
        }
    }

    #[test]
    fn bezier_paths_follow_their_control_points() {
        let to = Vec2::new(500.0, 400.0);
        let quad = path_span(MovePath::Quadratic(Vec2::new(400.0, 200.0)), to);
        // B(0.5) = (P0 + 2C + P1) / 4.
        assert_eq!(quad.value_at(0.5), Vec2::new(400.0, 300.0));
        let cubic = path_span(
            MovePath::Cubic(Vec2::new(300.0, 200.0), Vec2::new(500.0, 200.0)),
            to,
        );
        // B(0.5) = (P0 + 3C1 + 3C2 + P1) / 8.
        assert_eq!(cubic.value_at(0.5), Vec2::new(400.0, 250.0));
        assert_eq!(cubic.value_at(1.0), to);
        assert_eq!(cubic.value_at(0.0), BASE);
    }

    #[test]
    fn straight_and_zero_arc_paths_match_the_line() {
        let to = Vec2::new(512.5, 333.25);
        let line = path_span(MovePath::Line, to);
        let flat = path_span(MovePath::Arc(0.0), to);
        let degenerate = path_span(MovePath::Arc(0.3), BASE);
        for i in 0..=20 {
            let t = i as f64 / 20.0;
            assert_eq!(line.value_at(t), flat.value_at(t));
            assert_eq!(degenerate.value_at(t), BASE);
        }
        let mut bad = span(0, 0.0, 1.0, TrackTarget::Abs(to));
        bad.path = MovePath::Arc(0.6);
        assert!(ResolvedTrack::resolve(BASE, &[bad]).is_err());
        bad.path = MovePath::Quadratic(Vec2::new(f64::NAN, 0.0));
        assert!(ResolvedTrack::resolve(BASE, &[bad]).is_err());
    }
}
