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
    fn value(&self, time: f64) -> Vec2 {
        let progress = self.progress(time);
        // A finished span holds exactly `to`; `from + (to - from) * 1` can be off by one ulp.
        if progress == 1.0 {
            return self.to;
        }
        Vec2::new(
            self.from.x + (self.to.x - self.from.x) * progress,
            self.from.y + (self.to.y - self.from.y) * progress,
        )
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
                ..
            } if id == actor => Some(AuthoredSpan {
                order,
                at: *at,
                duration: *duration,
                target: TrackTarget::Abs(Vec2::new(*x, *y)),
            }),
            _ => None,
        })
        .collect::<Vec<_>>();
    ResolvedTrack::resolve(base, &spans)
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
}
