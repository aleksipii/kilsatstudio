//! Screen taps drive finger presses.
//!
//! Each screen cue's tap markers become scene times (`ScreenCue::tap_times`,
//! repeating with loops, ending where the next cue on the same slot starts).
//! At evaluation the actor's free arm presses the slot centre with
//! `motion_rig::tap_envelope` and `PreparedRig::tap_press`: a pure function of
//! scene time, so seeking equals playing and preview equals export. Authored
//! joint tracks on the tapping arm win: such taps are not animated. Taps that
//! cannot be animated are reported once, as `TapDiagnostic`s, at preparation.
use motion_animation::Transform2;
use motion_rig::{PreparedRig, TapArm, TapError};
use motion_script::Script;
use std::collections::BTreeMap;

/// Why a screen cue's taps are not animated.
#[derive(Debug, Clone, PartialEq)]
pub enum TapSkip {
    /// The rig has no free standard arm chain to press with.
    NoFreeArm(TapError),
    /// Authored joint keys drive the tapping arm's `joint`; they take precedence.
    AuthoredArm { joint: String },
}

/// One screen cue whose taps are not animated.
#[derive(Debug, Clone, PartialEq)]
pub struct TapDiagnostic {
    pub scene: String,
    pub actor: String,
    pub slot: String,
    /// Scene time the cue starts.
    pub at: f64,
    pub skip: TapSkip,
}
impl std::fmt::Display for TapDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: taps on {}.{} at {} s are not animated: ",
            self.scene, self.actor, self.slot, self.at
        )?;
        match &self.skip {
            TapSkip::NoFreeArm(error) => write!(f, "{error}"),
            TapSkip::AuthoredArm { joint } => write!(
                f,
                "authored joint keys on {joint} take precedence; remove them to animate the press"
            ),
        }
    }
}

/// A slot's presses on one actor.
#[derive(Debug, Clone)]
pub(crate) struct PreparedTaps {
    slot: String,
    arm: TapArm,
    /// Scene times of contact.
    times: Vec<f64>,
}

type SceneTaps = BTreeMap<String, Vec<PreparedTaps>>;

/// Resolve every scene's taps once.
pub(crate) fn prepare(
    script: &Script,
    rigs: &BTreeMap<String, PreparedRig>,
    joint_tracks: &[BTreeMap<String, Vec<(usize, motion_animation::Track)>>],
) -> (Vec<SceneTaps>, Vec<TapDiagnostic>) {
    let mut diagnostics = Vec::new();
    let scenes = script
        .scenes
        .iter()
        .zip(joint_tracks)
        .map(|(scene, joint_tracks)| {
            let mut taps = SceneTaps::new();
            let cues = &scene.composition.screens;
            for cue in cues.iter().filter(|c| !c.taps.is_empty()) {
                let Some(rig) = rigs.get(&cue.actor) else {
                    continue;
                };
                let mut report = |skip| {
                    diagnostics.push(TapDiagnostic {
                        scene: scene.id.clone(),
                        actor: cue.actor.clone(),
                        slot: cue.slot.clone(),
                        at: cue.at,
                        skip,
                    })
                };
                let arm = match rig.tap_arm(&cue.slot) {
                    Ok(arm) => arm,
                    Err(error) => {
                        report(TapSkip::NoFreeArm(error));
                        continue;
                    }
                };
                let authored = joint_tracks.get(&cue.actor).and_then(|tracks| {
                    let arm_joints = [arm.limb.upper, arm.limb.lower, arm.limb.end];
                    tracks
                        .iter()
                        .find(|(joint, _)| arm_joints.contains(joint))
                        .map(|(joint, _)| rig.rig().joints()[*joint].name.clone())
                });
                if let Some(joint) = authored {
                    report(TapSkip::AuthoredArm { joint });
                    continue;
                }
                // The cue lasts until the next cue on the same slot.
                let until = cues
                    .iter()
                    .filter(|c| c.actor == cue.actor && c.slot == cue.slot && c.at > cue.at)
                    .map(|c| c.at)
                    .fold(scene.duration, f64::min);
                let times: Vec<f64> = cue
                    .tap_times(until)
                    .into_iter()
                    .filter(|t| *t < until || until == scene.duration)
                    .collect();
                if times.is_empty() {
                    continue;
                }
                taps.entry(cue.actor.clone())
                    .or_default()
                    .push(PreparedTaps {
                        slot: cue.slot.clone(),
                        arm,
                        times,
                    });
            }
            taps
        })
        .collect();
    (scenes, diagnostics)
}

/// Append the press offsets active at `time` for one actor.
pub(crate) fn apply(
    rig: &PreparedRig,
    taps: &[PreparedTaps],
    time: f64,
    offsets: &mut Vec<(usize, Transform2)>,
) -> Result<(), TapError> {
    for tap in taps {
        let weight = motion_rig::tap_envelope(time, &tap.times);
        if let Some(press) = rig.tap_press(tap.arm, &tap.slot, weight, offsets)? {
            offsets.extend(press);
        }
    }
    Ok(())
}
