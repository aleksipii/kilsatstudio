//! Exposure ("on ones, twos, threes") and blocking interpolation.
//!
//! Both are evaluation-time settings: they change which time and which curve
//! an animated pose is sampled with, never the stored keys. The scene crate
//! applies them once, before evaluation, so preview, seek and export agree.
use serde::{Deserialize, Serialize};

/// Largest supported exposure: each drawing held for up to three frames.
pub const MAX_EXPOSURE: u8 = 3;

/// How many frames each evaluated pose is held: 1 (on ones), 2 or 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct Exposure(u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("exposure `on` must be 1, 2 or 3 frames, got {0}")]
pub struct ExposureError(pub u8);

impl Exposure {
    pub const ONES: Self = Self(1);
    pub const TWOS: Self = Self(2);
    pub const THREES: Self = Self(3);

    pub fn new(frames: u8) -> Result<Self, ExposureError> {
        if (1..=MAX_EXPOSURE).contains(&frames) {
            Ok(Self(frames))
        } else {
            Err(ExposureError(frames))
        }
    }

    pub fn frames(self) -> u8 {
        self.0
    }

    /// The first frame of the exposure group containing `frame`; groups start
    /// at frame 0 of the scene. On ones this is `frame` itself.
    pub fn exposed_frame(self, frame: u64) -> u64 {
        frame - frame % u64::from(self.0)
    }
}

impl Default for Exposure {
    fn default() -> Self {
        Self::ONES
    }
}

impl TryFrom<u8> for Exposure {
    type Error = ExposureError;
    fn try_from(frames: u8) -> Result<Self, Self::Error> {
        Self::new(frames)
    }
}

impl From<Exposure> for u8 {
    fn from(exposure: Exposure) -> Self {
        exposure.0
    }
}

/// How keys are interpolated at evaluation time.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Interpolation {
    /// Each key's own easing shapes its outgoing interval.
    #[default]
    Spline,
    /// Blocking: every key holds until the next one, whatever its easing.
    Stepped,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposure_bounds_and_groups() {
        for bad in [0, 4, 24, u8::MAX] {
            assert_eq!(Exposure::new(bad), Err(ExposureError(bad)));
        }
        let groups: Vec<_> = (0..7).map(|f| Exposure::TWOS.exposed_frame(f)).collect();
        assert_eq!(groups, [0, 0, 2, 2, 4, 4, 6]);
        let groups: Vec<_> = (0..7).map(|f| Exposure::THREES.exposed_frame(f)).collect();
        assert_eq!(groups, [0, 0, 0, 3, 3, 3, 6]);
        assert!((0..100).all(|f| Exposure::ONES.exposed_frame(f) == f));
        assert_eq!(Exposure::TWOS.exposed_frame(u64::MAX), u64::MAX - 1);
    }

    #[test]
    fn exposure_and_interpolation_serialize_as_plain_values() {
        assert_eq!(serde_yaml::to_string(&Exposure::TWOS).unwrap().trim(), "2");
        let parsed: Exposure = serde_yaml::from_str("3").unwrap();
        assert_eq!(parsed, Exposure::THREES);
        assert!(serde_yaml::from_str::<Exposure>("0").is_err());
        assert!(serde_yaml::from_str::<Exposure>("5").is_err());
        let stepped: Interpolation = serde_yaml::from_str("stepped").unwrap();
        assert_eq!(stepped, Interpolation::Stepped);
        assert!(serde_yaml::from_str::<Interpolation>("smooth").is_err());
    }
}
