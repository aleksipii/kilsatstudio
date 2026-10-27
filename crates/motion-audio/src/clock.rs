//! The canonical dialogue sample clock (ORK-G1-04).
//!
//! Speech placement, mouth evaluation, mixing and export all count whole samples
//! at [`DIALOGUE_SAMPLE_RATE`]. The rounding rules are the ones A04 recorded in
//! `docs/editable-timeline.md`:
//!
//! - Authored time is finite, non-negative `f64` seconds from the scene start.
//! - Seconds → sample rounds to the nearest sample, halves away from zero.
//! - Frame → sample is `floor(frame * 24000 / fps)` in exact integer arithmetic.
//! - A sample is shown by the last frame whose first sample is not after it.
//! - Scene offsets are sums of whole-sample durations, so seeks never drift.
use std::fmt;

/// Canonical dialogue, sequence and export sample rate.
pub const DIALOGUE_SAMPLE_RATE: u32 = 24_000;
const RATE: u128 = DIALOGUE_SAMPLE_RATE as u128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClockError {
    /// Negative, non-finite or out-of-range seconds.
    InvalidTime,
    /// A frame rate of zero.
    InvalidFrameRate,
    /// The result does not fit in a `u64` sample or frame index.
    Overflow,
}
impl fmt::Display for ClockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidTime => "time must be finite, non-negative and within the sample range",
            Self::InvalidFrameRate => "frame rate must be positive",
            Self::Overflow => "sample or frame index overflow",
        })
    }
}
impl std::error::Error for ClockError {}

/// Nearest sample for a finite, non-negative time in seconds.
pub fn seconds_to_sample(seconds: f64) -> Result<u64, ClockError> {
    let value = (seconds * f64::from(DIALOGUE_SAMPLE_RATE)).round();
    if !seconds.is_finite() || seconds < 0.0 || value >= u64::MAX as f64 {
        return Err(ClockError::InvalidTime);
    }
    Ok(value as u64)
}

/// Seconds from the scene start for a sample.
pub fn sample_to_seconds(sample: u64) -> f64 {
    sample as f64 / f64::from(DIALOGUE_SAMPLE_RATE)
}

/// First sample of a video frame: `floor(frame * 24000 / fps)`.
pub fn frame_to_sample(frame: u64, fps: u32) -> Result<u64, ClockError> {
    if fps == 0 {
        return Err(ClockError::InvalidFrameRate);
    }
    u64::try_from(u128::from(frame) * RATE / u128::from(fps)).map_err(|_| ClockError::Overflow)
}

/// Evaluation time of a frame. Bit-identical to `frame / fps` when fps divides 24000.
pub fn frame_to_seconds(frame: u64, fps: u32) -> Result<f64, ClockError> {
    frame_to_sample(frame, fps).map(sample_to_seconds)
}

/// The frame that displays `sample`: the largest `f` with `frame_to_sample(f) <= sample`.
pub fn sample_to_frame(sample: u64, fps: u32) -> Result<u64, ClockError> {
    if fps == 0 {
        return Err(ClockError::InvalidFrameRate);
    }
    let shown = ((u128::from(sample) + 1) * u128::from(fps) - 1) / RATE;
    u64::try_from(shown).map_err(|_| ClockError::Overflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_round_half_away_from_zero_and_reject_bad_input() {
        assert_eq!(seconds_to_sample(0.0), Ok(0));
        assert_eq!(seconds_to_sample(-0.0), Ok(0));
        assert_eq!(seconds_to_sample(1.0), Ok(24_000));
        assert_eq!(seconds_to_sample(0.5 / 24_000.0), Ok(1));
        assert_eq!(seconds_to_sample(1.5 / 24_000.0), Ok(2));
        assert_eq!(seconds_to_sample(0.49 / 24_000.0), Ok(0));
        for bad in [-1e-9, f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e300] {
            assert_eq!(
                seconds_to_sample(bad),
                Err(ClockError::InvalidTime),
                "{bad}"
            );
        }
    }

    #[test]
    fn integer_fps_matches_frame_over_fps_bitwise() {
        for fps in [24, 25, 30, 48, 50, 60, 120] {
            for frame in 0..5_000 {
                let seconds = frame_to_seconds(frame, fps).unwrap();
                assert_eq!(seconds.to_bits(), (frame as f64 / fps as f64).to_bits());
                assert_eq!(seconds_to_sample(seconds), frame_to_sample(frame, fps));
            }
        }
    }

    #[test]
    fn frames_own_their_samples_for_non_dividing_rates() {
        for fps in [1, 7, 24, 29, 30, 60, 24_000] {
            for frame in 1..1_500 {
                let sample = frame_to_sample(frame, fps).unwrap();
                assert_eq!(sample_to_frame(sample, fps), Ok(frame));
                assert_eq!(sample_to_frame(sample - 1, fps), Ok(frame - 1));
            }
        }
        assert_eq!(frame_to_sample(1, 7), Ok(3_428));
        assert_eq!(frame_to_sample(1, 0), Err(ClockError::InvalidFrameRate));
        assert_eq!(sample_to_frame(1, 0), Err(ClockError::InvalidFrameRate));
        assert_eq!(frame_to_sample(u64::MAX, 1), Err(ClockError::Overflow));
    }
}
