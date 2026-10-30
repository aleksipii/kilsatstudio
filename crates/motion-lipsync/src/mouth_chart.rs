//! Classic nine-shape mouth chart (Preston Blair / Rhubarb `A`–`H` + `X`).
//!
//! Kilsat evaluates 13 visemes; drawn replacement mouths usually come as the nine
//! classic shapes. [`MouthChart::from_viseme`] is the one projection 13 → 9 used by
//! the X-sheet and by `EstimatedVisemeTrack::to_mouth_cues`. It is the inverse of
//! `rhubarb::shape_viseme` on that function's image, so imported Rhubarb cues keep
//! their letter. The projection is lossy: Sibilant, Sh and R share `C` with E, and
//! TongueTeeth shares `H` with TongueUp. See docs/xsheet.md.
use crate::Viseme;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Every viseme in declaration order. [`all_visemes_listed`] keeps this complete.
pub const ALL_VISEMES: [Viseme; 13] = [
    Viseme::Rest,
    Viseme::Closed,
    Viseme::TeethLip,
    Viseme::TongueTeeth,
    Viseme::TongueUp,
    Viseme::Sibilant,
    Viseme::Sh,
    Viseme::R,
    Viseme::A,
    Viseme::E,
    Viseme::I,
    Viseme::O,
    Viseme::U,
];

/// Compile-time guard: adding a viseme variant breaks this match until
/// [`ALL_VISEMES`] (and the projection) are updated.
#[cfg(test)]
const fn all_visemes_listed(v: Viseme) -> usize {
    match v {
        Viseme::Rest => 0,
        Viseme::Closed => 1,
        Viseme::TeethLip => 2,
        Viseme::TongueTeeth => 3,
        Viseme::TongueUp => 4,
        Viseme::Sibilant => 5,
        Viseme::Sh => 6,
        Viseme::R => 7,
        Viseme::A => 8,
        Viseme::E => 9,
        Viseme::I => 10,
        Viseme::O => 11,
        Viseme::U => 12,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum MouthChart {
    /// Closed lips: P, B, M.
    A,
    /// Slightly open, teeth together: most consonants and EE.
    B,
    /// Open mouth: EH, AE; also S, SH and R in this projection.
    C,
    /// Wide open: AA.
    D,
    /// Slightly rounded: AO, ER.
    E,
    /// Puckered: UW, OW, W.
    F,
    /// Upper teeth on lower lip: F, V.
    G,
    /// Tongue raised or visible: L, T, D, N, TH.
    H,
    /// Idle / rest.
    X,
}

impl MouthChart {
    pub const ALL: [MouthChart; 9] = [
        Self::A,
        Self::B,
        Self::C,
        Self::D,
        Self::E,
        Self::F,
        Self::G,
        Self::H,
        Self::X,
    ];

    /// The documented projection 13 → 9. Total: every viseme has exactly one shape.
    pub const fn from_viseme(viseme: Viseme) -> Self {
        match viseme {
            Viseme::Rest => Self::X,
            Viseme::Closed => Self::A,
            Viseme::I => Self::B,
            Viseme::E | Viseme::Sibilant | Viseme::Sh | Viseme::R => Self::C,
            Viseme::A => Self::D,
            Viseme::O => Self::E,
            Viseme::U => Self::F,
            Viseme::TeethLip => Self::G,
            Viseme::TongueUp | Viseme::TongueTeeth => Self::H,
        }
    }

    pub const fn letter(self) -> char {
        match self {
            Self::A => 'A',
            Self::B => 'B',
            Self::C => 'C',
            Self::D => 'D',
            Self::E => 'E',
            Self::F => 'F',
            Self::G => 'G',
            Self::H => 'H',
            Self::X => 'X',
        }
    }

    pub fn from_letter(letter: char) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.letter() == letter)
    }
}

impl From<Viseme> for MouthChart {
    fn from(viseme: Viseme) -> Self {
        Self::from_viseme(viseme)
    }
}

impl fmt::Display for MouthChart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.letter())
    }
}

/// Stable snake_case name of a viseme (same spelling as its serde form).
pub const fn viseme_name(viseme: Viseme) -> &'static str {
    match viseme {
        Viseme::Rest => "rest",
        Viseme::Closed => "closed",
        Viseme::TeethLip => "teeth_lip",
        Viseme::TongueTeeth => "tongue_teeth",
        Viseme::TongueUp => "tongue_up",
        Viseme::Sibilant => "sibilant",
        Viseme::Sh => "sh",
        Viseme::R => "r",
        Viseme::A => "a",
        Viseme::E => "e",
        Viseme::I => "i",
        Viseme::O => "o",
        Viseme::U => "u",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rhubarb::shape_viseme;
    use std::collections::BTreeMap;

    #[test]
    fn all_visemes_is_complete_and_unique() {
        for (i, v) in ALL_VISEMES.iter().enumerate() {
            assert_eq!(all_visemes_listed(*v), i);
        }
    }

    #[test]
    fn every_viseme_has_exactly_one_target_and_every_shape_is_reached() {
        let mut targets: BTreeMap<&str, Vec<MouthChart>> = BTreeMap::new();
        for v in ALL_VISEMES {
            targets
                .entry(viseme_name(v))
                .or_default()
                .push(MouthChart::from_viseme(v));
        }
        assert_eq!(targets.len(), 13);
        assert!(targets.values().all(|t| t.len() == 1));
        for shape in MouthChart::ALL {
            assert!(
                ALL_VISEMES
                    .iter()
                    .any(|v| MouthChart::from_viseme(*v) == shape),
                "{shape} unreachable"
            );
        }
    }

    #[test]
    fn projection_inverts_rhubarb_import_mapping() {
        for shape in MouthChart::ALL {
            let viseme = shape_viseme(shape.letter()).expect("rhubarb letter");
            assert_eq!(MouthChart::from_viseme(viseme), shape);
            assert_eq!(MouthChart::from_letter(shape.letter()), Some(shape));
        }
        assert_eq!(MouthChart::from_letter('Z'), None);
    }

    #[test]
    fn documented_lossy_groups() {
        use Viseme::*;
        for (v, s) in [
            (Rest, 'X'),
            (Closed, 'A'),
            (TeethLip, 'G'),
            (TongueTeeth, 'H'),
            (TongueUp, 'H'),
            (Sibilant, 'C'),
            (Sh, 'C'),
            (R, 'C'),
            (A, 'D'),
            (E, 'C'),
            (I, 'B'),
            (O, 'E'),
            (U, 'F'),
        ] {
            assert_eq!(MouthChart::from_viseme(v).letter(), s, "{v:?}");
        }
    }

    #[test]
    fn viseme_names_match_serde() {
        for v in ALL_VISEMES {
            assert_eq!(
                serde_json::to_string(&v).unwrap(),
                format!("\"{}\"", viseme_name(v))
            );
        }
    }
}
