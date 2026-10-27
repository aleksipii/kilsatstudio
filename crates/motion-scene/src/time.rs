//! Time conversions on the canonical 24 kHz dialogue sample clock.
//!
//! This module owns no clock arithmetic (KS-002g, ORK-G1-A04): every function
//! delegates to [`motion_audio::clock`] and only maps [`ClockError`] onto this
//! crate's [`Error`], so sequence seeks, the evaluator, export and dialogue
//! placement share one implementation.
//!
//! - Authored time is finite `f64` seconds from the scene start.
//! - Seconds → samples rounds to the nearest sample, halves away from zero.
//! - Frame → sample is `floor(frame * 24000 / fps)` in exact integer arithmetic,
//!   the same rule `motion-export` uses for each video frame.
//! - A sample is shown by the last frame whose sample is not after it.
//! - Scene offsets are sums of whole-sample durations, so sequence seeks never
//!   accumulate floating-point drift.
use crate::Error;
use motion_audio::clock::{self, ClockError};

/// Canonical dialogue and sequence sample rate (ORK-G1-04). An alias of
/// [`motion_audio::clock::DIALOGUE_SAMPLE_RATE`], not a second definition.
pub const SEQUENCE_SAMPLE_RATE: u32 = clock::DIALOGUE_SAMPLE_RATE;

/// Maps the shared clock error onto the scene error kinds callers already match.
fn scene_error(error: ClockError) -> Error {
    match error {
        ClockError::InvalidTime => Error::InvalidTime,
        ClockError::InvalidFrameRate => Error::InvalidFrameClock,
        ClockError::Overflow => Error::FrameBudget,
    }
}

/// Nearest sample for a finite, non-negative time in seconds.
pub fn seconds_to_sample(seconds: f64) -> Result<u64, Error> {
    clock::seconds_to_sample(seconds).map_err(scene_error)
}

/// Scene-local seconds for a sample, as the evaluator receives them.
pub fn sample_to_seconds(sample: u64) -> f64 {
    clock::sample_to_seconds(sample)
}

/// First sample of a frame: `floor(frame * 24000 / fps)`.
pub fn frame_to_sample(frame: u64, fps: u32) -> Result<u64, Error> {
    clock::frame_to_sample(frame, fps).map_err(scene_error)
}

/// Scene-local evaluation time for a frame. When `fps` divides 24000
/// (24, 25, 30, 60, …) this is bit-identical to `frame / fps`.
pub fn frame_to_seconds(frame: u64, fps: u32) -> Result<f64, Error> {
    clock::frame_to_seconds(frame, fps).map_err(scene_error)
}

/// The frame that displays `sample`: the largest `f` with
/// `frame_to_sample(f, fps) <= sample`.
pub fn sample_to_frame(sample: u64, fps: u32) -> Result<u64, Error> {
    clock::sample_to_frame(sample, fps).map_err(scene_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_round_to_nearest_sample() {
        assert_eq!(seconds_to_sample(0.0).unwrap(), 0);
        assert_eq!(seconds_to_sample(-0.0).unwrap(), 0);
        assert_eq!(seconds_to_sample(1.0).unwrap(), 24_000);
        // 0.5 / 24000 s and 1.5 / 24000 s are halves: away from zero.
        assert_eq!(seconds_to_sample(0.5 / 24_000.0).unwrap(), 1);
        assert_eq!(seconds_to_sample(1.5 / 24_000.0).unwrap(), 2);
        assert_eq!(seconds_to_sample(0.49 / 24_000.0).unwrap(), 0);
        for bad in [-1e-9, f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e300] {
            assert!(
                matches!(seconds_to_sample(bad), Err(Error::InvalidTime)),
                "{bad}"
            );
        }
    }

    #[test]
    fn integer_fps_frames_match_frame_over_fps_bitwise() {
        for fps in [24, 25, 30, 48, 50, 60, 120] {
            for frame in 0..10_000 {
                let seconds = frame_to_seconds(frame, fps).unwrap();
                assert_eq!(
                    seconds.to_bits(),
                    (frame as f64 / fps as f64).to_bits(),
                    "fps {fps} frame {frame}"
                );
                let sample = frame_to_sample(frame, fps).unwrap();
                assert_eq!(seconds_to_sample(seconds).unwrap(), sample);
            }
        }
    }

    #[test]
    fn frame_sample_round_trip_and_ownership() {
        // 7 and 29 do not divide 24000, so frame starts are floored.
        for fps in [1, 7, 24, 29, 30, 60, 24_000] {
            let mut previous = None;
            for frame in 0..2_000 {
                let sample = frame_to_sample(frame, fps).unwrap();
                assert_eq!(sample_to_frame(sample, fps).unwrap(), frame);
                if let Some(previous) = previous {
                    assert!(sample > previous);
                    // Every sample before this frame belongs to the previous one.
                    assert_eq!(sample_to_frame(sample - 1, fps).unwrap(), frame - 1);
                }
                previous = Some(sample);
            }
        }
        assert_eq!(frame_to_sample(1, 7).unwrap(), 3_428);
        assert_eq!(sample_to_frame(3_427, 7).unwrap(), 0);
        assert!(matches!(
            frame_to_sample(1, 0),
            Err(Error::InvalidFrameClock)
        ));
        assert!(matches!(
            sample_to_frame(1, 0),
            Err(Error::InvalidFrameClock)
        ));
        assert!(matches!(
            frame_to_sample(u64::MAX, 1),
            Err(Error::FrameBudget)
        ));
    }

    #[test]
    fn export_frame_formula_is_the_shared_rule() {
        // motion-export: i.checked_mul(SAMPLE_RATE) / fps, then sample / SAMPLE_RATE.
        for fps in [7_u32, 24, 30] {
            for i in 0..5_000_u64 {
                let export = i * 24_000 / u64::from(fps);
                assert_eq!(frame_to_sample(i, fps).unwrap(), export);
                assert_eq!(
                    frame_to_seconds(i, fps).unwrap().to_bits(),
                    (export as f64 / 24_000.0).to_bits()
                );
            }
        }
    }

    /// Same value, or the same error kind after mapping the shared clock error.
    fn same<T, F>(scene: Result<T, Error>, audio: Result<T, ClockError>, eq: F) -> bool
    where
        F: Fn(&T, &T) -> bool,
    {
        match (scene, audio) {
            (Ok(a), Ok(b)) => eq(&a, &b),
            (Err(a), Err(b)) => {
                std::mem::discriminant(&a) == std::mem::discriminant(&scene_error(b))
            }
            _ => false,
        }
    }

    #[test]
    fn scene_and_audio_clock_paths_are_identical() {
        // KS-002g: both paths give the same samples, bit patterns and error kinds.
        assert_eq!(SEQUENCE_SAMPLE_RATE, clock::DIALOGUE_SAMPLE_RATE);
        let seconds = [
            0.0,
            -0.0,
            0.5 / 24_000.0,
            1.5 / 24_000.0,
            0.49 / 24_000.0,
            1.0 / 3.0,
            1_234.567_891,
            86_400.0,
            -1e-9,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            1e300,
        ];
        for value in seconds {
            assert!(
                same(
                    seconds_to_sample(value),
                    clock::seconds_to_sample(value),
                    |a, b| a == b
                ),
                "seconds_to_sample({value})"
            );
        }
        let samples = [0, 1, 3_427, 3_428, 23_999, 24_000, 1 << 40, u64::MAX];
        for sample in samples {
            assert_eq!(
                sample_to_seconds(sample).to_bits(),
                clock::sample_to_seconds(sample).to_bits()
            );
        }
        let bits = |a: &f64, b: &f64| a.to_bits() == b.to_bits();
        for fps in [0, 1, 7, 24, 25, 29, 30, 60, 24_000, u32::MAX] {
            for frame in (0..3_000).chain([u64::MAX / 2, u64::MAX]) {
                assert!(
                    same(
                        frame_to_sample(frame, fps),
                        clock::frame_to_sample(frame, fps),
                        |a, b| a == b
                    ),
                    "frame_to_sample({frame}, {fps})"
                );
                assert!(
                    same(
                        frame_to_seconds(frame, fps),
                        clock::frame_to_seconds(frame, fps),
                        bits
                    ),
                    "frame_to_seconds({frame}, {fps})"
                );
            }
            for sample in (0..3_000).chain(samples) {
                assert!(
                    same(
                        sample_to_frame(sample, fps),
                        clock::sample_to_frame(sample, fps),
                        |a, b| a == b
                    ),
                    "sample_to_frame({sample}, {fps})"
                );
            }
        }
    }
}
