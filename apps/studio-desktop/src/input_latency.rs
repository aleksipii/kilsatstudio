//! Opt-in, bounded application-side input-to-frame measurement.
//!
//! For each discrete input (key, text, mouse button, wheel) the recorder stamps the
//! start of the `update` pass that first sees it, the end of that pass (the frame that
//! first reflects the input has been built) and the start of the next pass, which
//! eframe only begins after it has painted and swapped the previous frame. The last
//! interval is therefore an application-side upper-bound proxy for "reflected frame
//! submitted". It excludes OS/windowing queue delay before egui receives the event and
//! display scan-out/photon latency, so it is NOT end-to-end display latency.
use std::{collections::VecDeque, time::Instant};

pub const CAPACITY: usize = 128;
/// A pending input whose following frame never began within this window is dropped.
const ABANDON_AFTER_SECS: f64 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    /// Input seen at update start -> end of that update (UI built).
    pub update_ms: f64,
    /// Input seen at update start -> start of the next update (previous frame painted).
    pub next_frame_ms: f64,
}

struct Pending {
    seen: Instant,
    update_done: Option<Instant>,
}

#[derive(Default)]
pub struct InputLatency {
    pub enabled: bool,
    samples: VecDeque<Sample>,
    pending: Option<Pending>,
    abandoned: u64,
}

impl InputLatency {
    /// Call at the start of `update`. `has_input` is true if this pass received a
    /// discrete input event. Returns true when a repaint should be requested so the
    /// pending measurement can complete (at most one extra frame per input).
    pub fn begin_frame(&mut self, now: Instant, has_input: bool) -> bool {
        if !self.enabled {
            self.pending = None;
            return false;
        }
        if let Some(p) = &self.pending {
            let stale = now.duration_since(p.seen).as_secs_f64() > ABANDON_AFTER_SECS;
            match p.update_done {
                _ if stale => {
                    self.pending = None;
                    self.abandoned += 1;
                }
                Some(done) if now >= done => {
                    let seen = p.seen;
                    let update_ms = done.duration_since(seen).as_secs_f64() * 1000.0;
                    let next_frame_ms = now.duration_since(seen).as_secs_f64() * 1000.0;
                    self.pending = None;
                    if self.samples.len() == CAPACITY {
                        self.samples.pop_front();
                    }
                    self.samples.push_back(Sample {
                        update_ms,
                        next_frame_ms,
                    });
                }
                _ => {}
            }
        }
        if has_input && self.pending.is_none() {
            self.pending = Some(Pending {
                seen: now,
                update_done: None,
            });
        }
        self.pending.is_some()
    }
    /// Call at the end of `update`.
    pub fn end_frame(&mut self, now: Instant) {
        if let Some(p) = self.pending.as_mut().filter(|p| p.update_done.is_none()) {
            p.update_done = Some(now.max(p.seen));
        }
    }
    pub fn clear(&mut self) {
        self.samples.clear();
        self.pending = None;
        self.abandoned = 0;
    }
    pub fn len(&self) -> usize {
        self.samples.len()
    }
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
    fn summary(values: impl Iterator<Item = f64>) -> serde_json::Value {
        let mut values: Vec<f64> = values.collect();
        values.sort_by(f64::total_cmp);
        let n = values.len();
        if n == 0 {
            return serde_json::json!({"samples":0});
        }
        serde_json::json!({"samples":n,"mean_ms":values.iter().sum::<f64>() / n as f64,
            "p95_ms":values[(n * 95).div_ceil(100) - 1],"max_ms":values[n - 1]})
    }
    pub fn report(&self) -> serde_json::Value {
        serde_json::json!({"capacity":CAPACITY,"abandoned_inputs":self.abandoned,
            "input_to_update_end":Self::summary(self.samples.iter().map(|s| s.update_ms)),
            "input_to_next_frame_start":Self::summary(self.samples.iter().map(|s| s.next_frame_ms)),
            "limitations":"Application-side time from the update pass that first sees a key, text, mouse-button or wheel event until the end of that pass and the start of the next pass. Excludes OS/windowing delay before egui receives the event and display scan-out; not photon or end-to-end display latency. Pointer motion is not sampled."})
    }
}

/// Discrete events whose reflection is worth timing; pointer motion is excluded.
pub fn has_discrete_input(events: &[eframe::egui::Event]) -> bool {
    use eframe::egui::Event;
    events.iter().any(|e| {
        matches!(
            e,
            Event::Key { pressed: true, .. }
                | Event::Text(_)
                | Event::PointerButton { pressed: true, .. }
                | Event::MouseWheel { .. }
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    fn at(base: Instant, ms: u64) -> Instant {
        base + Duration::from_millis(ms)
    }
    #[test]
    fn disabled_records_nothing_and_never_requests_repaint() {
        let base = Instant::now();
        let mut l = InputLatency::default();
        assert!(!l.begin_frame(base, true));
        l.end_frame(at(base, 1));
        assert!(!l.begin_frame(at(base, 20), false));
        assert!(l.is_empty());
    }
    #[test]
    fn measures_update_and_next_frame_for_one_input() {
        let base = Instant::now();
        let mut l = InputLatency {
            enabled: true,
            ..Default::default()
        };
        assert!(l.begin_frame(base, true));
        l.end_frame(at(base, 3));
        assert!(!l.begin_frame(at(base, 20), false));
        let r = l.report();
        assert_eq!(l.len(), 1);
        assert_eq!(r["input_to_update_end"]["max_ms"], 3.0);
        assert_eq!(r["input_to_next_frame_start"]["max_ms"], 20.0);
        // A frame without input leaves nothing pending.
        assert!(!l.begin_frame(at(base, 40), false));
        assert_eq!(l.len(), 1);
    }
    #[test]
    fn input_during_pending_measurement_does_not_start_a_second_one() {
        let base = Instant::now();
        let mut l = InputLatency {
            enabled: true,
            ..Default::default()
        };
        l.begin_frame(base, true);
        l.end_frame(at(base, 2));
        // The next pass completes the first sample and starts a new one.
        assert!(l.begin_frame(at(base, 16), true));
        l.end_frame(at(base, 18));
        l.begin_frame(at(base, 32), false);
        assert_eq!(l.len(), 2);
    }
    #[test]
    fn samples_are_bounded_and_stale_pending_is_abandoned() {
        let base = Instant::now();
        let mut l = InputLatency {
            enabled: true,
            ..Default::default()
        };
        let mut t = 0;
        for _ in 0..CAPACITY + 50 {
            l.begin_frame(at(base, t), true);
            l.end_frame(at(base, t + 1));
            t += 10;
            l.begin_frame(at(base, t), false);
            t += 10;
        }
        assert_eq!(l.len(), CAPACITY);
        // Pending that never sees a later pass within the window is abandoned.
        l.begin_frame(at(base, t), true);
        assert!(!l.begin_frame(at(base, t + 3000), false));
        assert_eq!(l.report()["abandoned_inputs"], 1);
        l.clear();
        assert!(l.is_empty());
    }
}
