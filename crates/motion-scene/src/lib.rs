//! Deterministic, graphics-independent scene evaluation shared by preview and export.
pub mod editable_timeline;
use motion_animation::HumanoidState;
use motion_animation::Transform2;
pub use motion_animation::{Gait, GAITS, GESTURES, POSTURE_BLEND_SECONDS};
pub use motion_animation::{Vec2, CHEST, HEAD, HIP, LEFT_FOOT, LEFT_HAND, RIGHT_FOOT, RIGHT_HAND};
use motion_lipsync::Viseme;
pub use motion_rig::FaceView;
pub use motion_script::FACINGS;
use motion_script::{Event, Script};
mod sequence;
pub use sequence::{SceneContribution, SceneSpan, SequenceTimeline};
pub mod time;
pub use time::SEQUENCE_SAMPLE_RATE;
mod taps;
pub mod tracks;
pub use taps::{TapDiagnostic, TapSkip};

/// Head dip, in local skeleton units, at a fully open mouth.
pub const SPEECH_HEAD_DIP: f64 = 3.0;

/// Rule-based mouth openness used for speech head motion; rest and closed are zero.
pub fn viseme_openness(viseme: Viseme) -> f64 {
    match viseme {
        Viseme::Rest | Viseme::Closed => 0.0,
        Viseme::A => 1.0,
        Viseme::O | Viseme::U => 0.7,
        Viseme::E | Viseme::I => 0.4,
        _ => 0.3,
    }
}

/// Applies speech head motion from the same per-actor visemes the mouth renders,
/// so preview and export share one audio sample clock. Missing actors are ignored;
/// actors without a viseme are unchanged.
///
/// Custom rigs with a `visemes` map also swap the mouth part to the variant for
/// the actor's viseme (`Rest` when none is supplied). Visemes the rig does not map
/// keep the base shape. The swap is a pure function of the evaluated pose and the
/// viseme, so repeated or out-of-order calls give identical parts.
pub fn apply_speech(scene: &mut EvaluatedScene, mouths: &[(&str, Viseme)]) {
    for actor in &mut scene.actors {
        let viseme = mouths.iter().find(|(id, _)| *id == actor.id).map(|m| m.1);
        if let Some(viseme) = viseme {
            actor.skeleton[HEAD].y += viseme_openness(viseme) * SPEECH_HEAD_DIP;
        }
        if let Some(mouth) = &actor.mouth {
            let wanted = viseme.unwrap_or(Viseme::Rest);
            if let Some((_, shape)) = mouth.shapes.iter().find(|(v, _)| *v == wanted) {
                actor.rigid_parts[mouth.part] = shape.clone();
            }
        }
    }
}

/// The snake_case name a custom rig's `visemes.map` uses for `viseme`.
pub fn viseme_name(viseme: Viseme) -> &'static str {
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
fn viseme_from_name(name: &str) -> Option<Viseme> {
    [
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
    ]
    .into_iter()
    .find(|v| viseme_name(*v) == name)
}

/// Mouth-part alternatives evaluated for one pose, one per viseme the rig maps.
/// `apply_speech` copies the match over `rigid_parts[part]`.
#[derive(Debug, Clone, PartialEq)]
pub struct MouthVariants {
    pub part: usize,
    pub shapes: Vec<(Viseme, motion_rig::EvaluatedPart)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActorPose {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub emotion: String,
    pub rotation: f64,
    pub scale: Vec2,
    /// Envelope of the active gesture in 0..=1; zero outside gesture events.
    pub wave: f64,
    /// Evaluated local-space skeletal joints shared by preview and export.
    pub skeleton: [Vec2; 7],
    /// Validated custom rigid parts in actor-local coordinates.
    pub rigid_parts: Vec<motion_rig::EvaluatedPart>,
    /// Viseme-selectable mouth shapes for a custom rig with `visemes`.
    pub mouth: Option<MouthVariants>,
    /// Which side faces the viewer. A left profile is the side view mirrored
    /// through a negative `scale.x`.
    pub view: FaceView,
}
impl ActorPose {
    /// Transform any local skeleton, facial, or artwork point through the root pose.
    pub fn local_to_world(&self, point: Vec2) -> Vec2 {
        motion_animation::Transform2 {
            translation: Vec2::new(self.x, self.y),
            rotation: self.rotation,
            scale: self.scale,
        }
        .affine()
        .point(point)
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraPose {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}
impl Default for CameraPose {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        }
    }
}
impl CameraPose {
    /// Transform a world point into canvas coordinates; preview and export share this rule.
    pub fn project(self, x: f64, y: f64, width: u32, height: u32) -> (f64, f64) {
        let cx = width as f64 * 0.5;
        let cy = height as f64 * 0.5;
        (
            cx + (x - cx - self.x) * self.zoom,
            cy + (y - cy - self.y) * self.zoom,
        )
    }
}
fn camera_at(keys: &[motion_script::CameraKey], time: f64) -> CameraPose {
    let Some(first) = keys.first() else {
        return CameraPose::default();
    };
    let mut left = first;
    for right in keys.iter().skip(1) {
        if time <= right.at {
            let t = ((time - left.at) / (right.at - left.at)).clamp(0.0, 1.0);
            return CameraPose {
                x: left.x + (right.x - left.x) * t,
                y: left.y + (right.y - left.y) * t,
                zoom: left.zoom + (right.zoom - left.zoom) * t,
            };
        }
        left = right;
    }
    CameraPose {
        x: left.x,
        y: left.y,
        zoom: left.zoom,
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct EvaluatedScene {
    /// Validated scene-local evaluation time in seconds.
    pub time: f64,
    pub camera: CameraPose,
    pub background_color: String,
    /// Whole-canvas opacity over black, for fades at scene edges.
    pub opacity: f64,
    pub width: u32,
    pub height: u32,
    pub actors: Vec<ActorPose>,
}
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid sequence: {0}")]
    InvalidSequence(String),
    #[error(transparent)]
    Script(#[from] motion_script::Error),
    #[error(transparent)]
    Animation(#[from] motion_animation::Error),
    #[error("invalid prepared animation reference: {0}")]
    InvalidAnimation(String),
    #[error("scene index {0} does not exist")]
    MissingScene(usize),
    #[error("evaluation time must be finite and within scene duration")]
    InvalidTime,
    #[error(
        "frame clock requires a finite positive duration, positive fps, and positive frame budget"
    )]
    InvalidFrameClock,
    #[error("frame count exceeds the configured export budget")]
    FrameBudget,
}

/// Count samples at `frame / fps` strictly before the scene endpoint.
///
/// The explicit budget bounds allocations and work before export starts.
/// Fractional final frames are included, while floating multiplication rounding
/// must not introduce an extra sample exactly at the scene endpoint.
pub fn frame_count(duration_seconds: f64, fps: u32, max_frames: u64) -> Result<u64, Error> {
    if !duration_seconds.is_finite() || duration_seconds <= 0.0 || fps == 0 || max_frames == 0 {
        return Err(Error::InvalidFrameClock);
    }
    let mut count = (duration_seconds * fps as f64).ceil();
    if !count.is_finite() || count >= u64::MAX as f64 {
        return Err(Error::FrameBudget);
    }
    if count >= 1.0 && (count - 1.0) / fps as f64 >= duration_seconds {
        count -= 1.0;
    }
    let count = count as u64;
    if count == 0 || count > max_frames {
        return Err(Error::FrameBudget);
    }
    Ok(count)
}

/// An immutable script validated once for repeated scene evaluation.
#[derive(Clone)]
pub struct ValidatedScript {
    script: std::sync::Arc<Script>,
    prepared: std::sync::Arc<PreparedAnimation>,
}
type BoneTracks = [Option<motion_animation::Track>; 7];
#[derive(Default)]
struct PreparedAnimation {
    rigid_parts: std::collections::BTreeMap<String, Vec<motion_rig::EvaluatedPart>>,
    rigs: std::collections::BTreeMap<String, motion_rig::PreparedRig>,
    /// Per scene and actor: joint index with its offset track.
    joint_tracks: Vec<std::collections::BTreeMap<String, Vec<(usize, motion_animation::Track)>>>,
    library: std::collections::BTreeMap<String, BoneTracks>,
    scenes: Vec<std::collections::BTreeMap<String, BoneTracks>>,
    /// Per scene and actor: `Event::Move` resolved once into `{from, to}` spans.
    moves: Vec<std::collections::BTreeMap<String, tracks::ResolvedTrack>>,
    /// Per scene and actor: screen taps the free hand presses.
    taps: Vec<std::collections::BTreeMap<String, Vec<taps::PreparedTaps>>>,
    tap_diagnostics: Vec<TapDiagnostic>,
}
/// Where every actor stands before its first move.
fn actor_base(script: &Script) -> Vec2 {
    Vec2::new(
        script.project.width as f64 / 2.0,
        script.project.height as f64 * 0.7,
    )
}
fn prepare_bones(
    tracks: &std::collections::BTreeMap<String, Vec<motion_script::ActorKey>>,
) -> Result<BoneTracks, Error> {
    let mut prepared: BoneTracks = std::array::from_fn(|_| None);
    for (bone, keys) in tracks {
        let joint = motion_animation::HUMANOID_BONES
            .iter()
            .position(|name| *name == bone.as_str())
            .ok_or_else(|| Error::InvalidAnimation(bone.clone()))?;
        prepared[joint] = Some(motion_animation::Track::new(
            keys.iter()
                .map(motion_script::ActorKey::animation_key)
                .collect(),
        )?);
    }
    Ok(prepared)
}
impl PreparedAnimation {
    fn new(script: &Script) -> Result<Self, Error> {
        // Unused definitions remain source data only. In particular, direct raw
        // evaluation of projects using copied tracks must not copy the library.
        let referenced = script
            .scenes
            .iter()
            .flat_map(|scene| scene.composition.clip_instances.iter())
            .map(|instance| instance.clip.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        let library = referenced
            .into_iter()
            .map(|id| {
                let clip = script
                    .animation_clips
                    .get(id)
                    .ok_or_else(|| Error::InvalidAnimation(id.into()))?;
                Ok((id.to_owned(), prepare_bones(&clip.bone_tracks)?))
            })
            .collect::<Result<_, Error>>()?;
        let scenes = script
            .scenes
            .iter()
            .map(|scene| {
                scene
                    .composition
                    .bone_tracks
                    .iter()
                    .map(|(actor, tracks)| Ok((actor.clone(), prepare_bones(tracks)?)))
                    .collect::<Result<_, Error>>()
            })
            .collect::<Result<_, Error>>()?;
        let mut rigid_parts = std::collections::BTreeMap::new();
        let mut rigs = std::collections::BTreeMap::new();
        for (id, character) in &script.characters {
            if let Some(rig) = &character.rig {
                let invalid = |error: motion_rig::Error| Error::InvalidAnimation(error.to_string());
                let prepared = rig.prepare().map_err(invalid)?;
                rigid_parts.insert(id.clone(), prepared.evaluate_parts(&[]).map_err(invalid)?);
                rigs.insert(id.clone(), prepared);
            }
        }
        let joint_tracks: Vec<_> = script
            .scenes
            .iter()
            .map(|scene| {
                scene
                    .composition
                    .joint_tracks
                    .iter()
                    .map(|(actor, tracks)| {
                        let rig = rigs
                            .get(actor)
                            .ok_or_else(|| Error::InvalidAnimation(actor.clone()))?;
                        let prepared = tracks
                            .iter()
                            .map(|(joint, keys)| {
                                let index = rig
                                    .joint_index(joint)
                                    .ok_or_else(|| Error::InvalidAnimation(joint.clone()))?;
                                let track = motion_animation::Track::new(
                                    keys.iter()
                                        .map(motion_script::ActorKey::animation_key)
                                        .collect(),
                                )?;
                                Ok((index, track))
                            })
                            .collect::<Result<Vec<_>, Error>>()?;
                        Ok((actor.clone(), prepared))
                    })
                    .collect::<Result<_, Error>>()
            })
            .collect::<Result<_, Error>>()?;
        let base = actor_base(script);
        let moves = script
            .scenes
            .iter()
            .map(|scene| {
                let movers = scene
                    .events
                    .iter()
                    .filter_map(|event| match event {
                        Event::Move { actor, .. } => Some(actor.as_str()),
                        _ => None,
                    })
                    .collect::<std::collections::BTreeSet<_>>();
                movers
                    .into_iter()
                    .map(|actor| {
                        let track = tracks::resolve_moves(base, &scene.events, actor)?;
                        Ok((actor.to_owned(), track))
                    })
                    .collect::<Result<_, Error>>()
            })
            .collect::<Result<_, Error>>()?;
        let (taps, tap_diagnostics) = taps::prepare(script, &rigs, &joint_tracks);
        Ok(Self {
            rigid_parts,
            rigs,
            joint_tracks,
            library,
            scenes,
            moves,
            taps,
            tap_diagnostics,
        })
    }
    fn instance_tracks<'a>(
        &'a self,
        script: &Script,
        instance: &motion_script::ClipInstance,
        time: f64,
    ) -> Result<(&'a BoneTracks, f64), Error> {
        let clip = script
            .animation_clips
            .get(&instance.clip)
            .ok_or_else(|| Error::InvalidAnimation(instance.clip.clone()))?;
        let tracks = self
            .library
            .get(&instance.clip)
            .ok_or_else(|| Error::InvalidAnimation(instance.clip.clone()))?;
        let local = time - instance.at;
        let clip_time = if instance.looping {
            local.rem_euclid(clip.duration)
        } else {
            local / instance.duration * clip.duration
        };
        if !clip_time.is_finite() || clip_time < 0.0 || clip_time > clip.duration {
            return Err(Error::InvalidTime);
        }
        Ok((tracks, clip_time))
    }
    fn normalized_offsets(
        &self,
        script: &Script,
        instances: &[motion_script::ClipInstance],
        time: f64,
        values: &mut [Option<motion_animation::Transform2>; 7],
    ) -> Result<(), Error> {
        // Stable IDs fix floating-point sum order independently of row order.
        let mut contributors = instances.iter().collect::<Vec<_>>();
        contributors.sort_unstable_by(|a, b| a.id.cmp(&b.id));
        for (joint, value) in values.iter_mut().enumerate() {
            let mut samples = Vec::with_capacity(contributors.len());
            for instance in &contributors {
                let weight = instance.sample_weight(time);
                if weight == 0.0 || !selected_joint(instance, joint) {
                    continue;
                }
                let (tracks, clip_time) = self.instance_tracks(script, instance, time)?;
                if let Some(track) = &tracks[joint] {
                    samples.push((weight, track.sample(clip_time)?));
                }
            }
            if samples.is_empty() {
                continue;
            }
            let total = samples.iter().map(|(weight, _)| weight).sum::<f64>();
            let residual = (1.0 - total).max(0.0);
            let divisor = total.max(1.0);
            let base = value.unwrap_or_default();
            // Keep the authored-base anchor as total weight crosses one so a
            // continuous envelope cannot introduce a rotation reference snap.
            let reference = base.rotation;
            let mut combined = motion_animation::Transform2 {
                translation: Vec2::new(
                    base.translation.x * residual,
                    base.translation.y * residual,
                ),
                rotation: reference,
                scale: Vec2::new(base.scale.x * residual, base.scale.y * residual),
            };
            for (weight, sample) in samples {
                let weight = weight / divisor;
                combined.translation.x += sample.translation.x * weight;
                combined.translation.y += sample.translation.y * weight;
                combined.scale.x += sample.scale.x * weight;
                combined.scale.y += sample.scale.y * weight;
                // Exact half turns consistently choose the positive arc.
                let mut delta = (sample.rotation - reference).rem_euclid(std::f64::consts::TAU);
                if delta > std::f64::consts::PI {
                    delta -= std::f64::consts::TAU;
                }
                combined.rotation += weight * delta;
            }
            *value = Some(motion_animation::blend_humanoid_offset(
                base, combined, 1.0,
            )?);
        }
        Ok(())
    }
    fn offsets(
        &self,
        script: &Script,
        scene_index: usize,
        actor: &str,
        time: f64,
    ) -> Result<Vec<(usize, motion_animation::Transform2)>, Error> {
        let mut values = [None; 7];
        if let Some(tracks) = self.scenes[scene_index].get(actor) {
            for (value, track) in values.iter_mut().zip(tracks) {
                if let Some(track) = track {
                    *value = Some(track.sample(time)?);
                }
            }
        }
        let instances = &script.scenes[scene_index].composition.clip_instances;
        let mut index = 0;
        while index < instances.len() {
            let instance = &instances[index];
            if let Some(group) = &instance.normalized_group {
                let end = index
                    + instances[index..]
                        .partition_point(|row| row.normalized_group.as_ref() == Some(group));
                if instance.actor == actor {
                    self.normalized_offsets(script, &instances[index..end], time, &mut values)?;
                }
                index = end;
                continue;
            }
            index += 1;
            if instance.actor != actor {
                continue;
            }
            let weight = instance.sample_weight(time);
            if weight == 0.0 {
                continue;
            }
            let (tracks, clip_time) = self.instance_tracks(script, instance, time)?;
            for (joint, (value, track)) in values.iter_mut().zip(tracks).enumerate() {
                if !selected_joint(instance, joint) {
                    continue;
                }
                if let Some(track) = track {
                    let sampled = track.sample(clip_time)?;
                    let base = value.unwrap_or_default();
                    *value = Some(match instance.blend_mode {
                        motion_script::ClipBlendMode::Override => {
                            motion_animation::blend_humanoid_offset(base, sampled, weight)?
                        }
                        motion_script::ClipBlendMode::Additive => {
                            motion_animation::add_humanoid_offset(base, sampled, weight)?
                        }
                    });
                }
            }
        }
        Ok(values
            .into_iter()
            .enumerate()
            .filter_map(|(joint, value)| value.map(|value| (joint, value)))
            .collect())
    }
}
fn selected_joint(instance: &motion_script::ClipInstance, joint: usize) -> bool {
    instance.joint_mask.is_empty()
        || instance
            .joint_mask
            .iter()
            .any(|name| name == motion_animation::HUMANOID_BONES[joint])
}
impl ValidatedScript {
    pub fn new(script: std::sync::Arc<Script>) -> Result<Self, Error> {
        script.validate()?;
        // Animation rules expand once, so preview, seek and export share one script.
        let mut script = if script.rules.is_empty() {
            script
        } else {
            std::sync::Arc::new(script.expand_rules().map_err(motion_script::Error::from)?)
        };
        // `holds:` props are attached once to the rigs every frame evaluates.
        if script.has_held_props() {
            std::sync::Arc::make_mut(&mut script).resolve_held_props()?;
        }
        let prepared = std::sync::Arc::new(PreparedAnimation::new(&script)?);
        Ok(Self { script, prepared })
    }
    /// Screen cues whose taps are not animated as finger presses, and why.
    pub fn tap_diagnostics(&self) -> &[TapDiagnostic] {
        &self.prepared.tap_diagnostics
    }
    /// `scene_at_time`: the evaluated scene at scene-local `time_seconds`, in
    /// `[0, duration]`. A pure function of its arguments: no playback history,
    /// so a direct seek equals frame-by-frame playback to the same time.
    pub fn evaluate(&self, scene_index: usize, time_seconds: f64) -> Result<EvaluatedScene, Error> {
        evaluate_validated(&self.script, &self.prepared, scene_index, time_seconds)
    }
    /// Evaluate at a scene-local sample on the shared 24 kHz clock.
    pub fn evaluate_sample(
        &self,
        scene_index: usize,
        local_sample: u64,
    ) -> Result<EvaluatedScene, Error> {
        self.evaluate(scene_index, time::sample_to_seconds(local_sample))
    }
}
pub fn evaluate(
    script: &Script,
    scene_index: usize,
    time_seconds: f64,
) -> Result<EvaluatedScene, Error> {
    script.validate()?;
    let expanded;
    let script = if script.rules.is_empty() && !script.has_held_props() {
        script
    } else {
        let mut copy = if script.rules.is_empty() {
            script.clone()
        } else {
            script.expand_rules().map_err(motion_script::Error::from)?
        };
        copy.resolve_held_props()?;
        expanded = copy;
        &expanded
    };
    let prepared = PreparedAnimation::new(script)?;
    evaluate_validated(script, &prepared, scene_index, time_seconds)
}
/// Distance, in actor units, at which a gaze target pulls the pupils to full travel.
const GAZE_FULL_DISTANCE: f64 = 100.0;
/// Point each gaze-tracked actor's pupils toward its target at `time`. Targets are
/// screen points or other actors' eyes (their origin when they have none), measured
/// in the looking actor's own space so rotation and mirroring are honoured.
fn apply_gaze(
    actors: &mut [ActorPose],
    scene: &motion_script::Scene,
    prepared: &PreparedAnimation,
    time: f64,
) {
    if scene.composition.gaze_tracks.is_empty() {
        return;
    }
    let eyes: Vec<(String, Vec2)> = actors
        .iter()
        .map(|actor| {
            let anchor = prepared
                .rigs
                .get(&actor.id)
                .and_then(|rig| rig.eye_anchor(&actor.rigid_parts))
                .unwrap_or_default();
            (actor.id.clone(), actor.local_to_world(anchor))
        })
        .collect();
    for actor in actors.iter_mut() {
        let (Some(keys), Some(rig)) = (
            scene.composition.gaze_tracks.get(&actor.id),
            prepared.rigs.get(&actor.id),
        ) else {
            continue;
        };
        let Some(anchor) = rig.eye_anchor(&actor.rigid_parts) else {
            continue;
        };
        let to_local = motion_animation::Transform2 {
            translation: Vec2::new(actor.x, actor.y),
            rotation: actor.rotation,
            scale: actor.scale,
        }
        .affine()
        .inverse();
        let direction = |key: &motion_script::GazeKey| -> [f64; 2] {
            let world = match (&key.target, key.x, key.y) {
                (Some(target), _, _) => eyes.iter().find(|(id, _)| id == target).map(|e| e.1),
                (None, Some(x), Some(y)) => Some(Vec2::new(x, y)),
                _ => None,
            };
            let Some(world) = world else {
                return [0.0, 0.0];
            };
            let local = to_local.point(world);
            let (dx, dy) = (local.x - anchor.x, local.y - anchor.y);
            let length = dx.hypot(dy);
            if !length.is_finite() || length < 1e-9 {
                return [0.0, 0.0];
            }
            let strength = (length / GAZE_FULL_DISTANCE).min(1.0) / length;
            [dx * strength, dy * strength]
        };
        let next = keys.partition_point(|key| key.at <= time);
        let gaze = match (next.checked_sub(1).map(|i| &keys[i]), keys.get(next)) {
            (Some(a), Some(b)) => {
                let w = ((time - a.at) / (b.at - a.at)).clamp(0.0, 1.0);
                let (da, db) = (direction(a), direction(b));
                [da[0] + (db[0] - da[0]) * w, da[1] + (db[1] - da[1]) * w]
            }
            (Some(only), None) | (None, Some(only)) => direction(only),
            (None, None) => [0.0, 0.0],
        };
        rig.apply_gaze(&mut actor.rigid_parts, gaze);
    }
}
/// Horizontal placement of the built-in figure's facial features for a view:
/// `x' = x * scale + shift`, or `None` when the face is hidden (back view).
/// The side view turns the face toward local +x, like custom-rig features.
pub fn procedural_face(view: FaceView) -> Option<(f64, f64)> {
    match view {
        FaceView::Front => Some((1.0, 0.0)),
        FaceView::Side => Some((0.55, 11.0)),
        FaceView::Back => None,
    }
}

/// Seconds a change of facing takes: the figure narrows, swaps side at the
/// midpoint and widens again.
pub const TURN_SECONDS: f64 = 0.2;
/// Narrowest width during a turn, as a fraction of the full width.
const TURN_MIN_WIDTH: f64 = 0.2;
fn facing(name: &str) -> (FaceView, f64) {
    match name {
        "left" => (FaceView::Side, -1.0),
        "right" => (FaceView::Side, 1.0),
        "back" => (FaceView::Back, 1.0),
        _ => (FaceView::Front, 1.0),
    }
}
/// View and horizontal scale factor from the actor's face events up to `time`.
/// `events` must be sorted by time; a pure function of its inputs.
fn facing_at(events: &[&Event], actor: &str, time: f64) -> (FaceView, f64) {
    let (mut previous, mut current, mut since) = ("front", "front", f64::INFINITY);
    for event in events {
        if let Event::Face {
            at,
            actor: a,
            value,
            ..
        } = event
        {
            if a == actor && *at <= time {
                previous = current;
                current = value;
                since = time - at;
            }
        }
    }
    let (from, to) = (facing(previous), facing(current));
    if from == to || since >= TURN_SECONDS {
        return to;
    }
    let progress = since / TURN_SECONDS;
    let width = (std::f64::consts::PI * progress)
        .cos()
        .abs()
        .max(TURN_MIN_WIDTH);
    let (view, sign) = if progress < 0.5 { from } else { to };
    (view, sign * width)
}
/// Add procedural offsets to the rig joints with standard names, merging with
/// any authored offset for the same joint (translations and rotations add).
fn add_rig_motion(
    rig: &motion_rig::PreparedRig,
    offsets: &mut Vec<(usize, Transform2)>,
    motion: &motion_animation::RigMotion,
) {
    for (name, delta) in motion_animation::RIG_MOTION_JOINTS.iter().zip(motion) {
        if *delta == Transform2::default() {
            continue;
        }
        let Some(joint) = rig.joint_index(name) else {
            continue;
        };
        match offsets.iter_mut().find(|(j, _)| *j == joint) {
            Some((_, offset)) => {
                offset.translation.x += delta.translation.x;
                offset.translation.y += delta.translation.y;
                offset.rotation += delta.rotation;
            }
            None => offsets.push((joint, *delta)),
        }
    }
}
fn evaluate_validated(
    script: &Script,
    prepared: &PreparedAnimation,
    scene_index: usize,
    time_seconds: f64,
) -> Result<EvaluatedScene, Error> {
    let scene = script
        .scenes
        .get(scene_index)
        .ok_or(Error::MissingScene(scene_index))?;
    if !time_seconds.is_finite() || time_seconds < 0.0 || time_seconds > scene.duration {
        return Err(Error::InvalidTime);
    }
    // Seeking to -0.0 and 0.0 must give bit-identical scenes.
    let time_seconds = if time_seconds == 0.0 {
        0.0
    } else {
        time_seconds
    };
    let mut events: Vec<_> = scene.events.iter().collect();
    // Stable ordering preserves source order for simultaneous expression events.
    events.sort_by(|a, b| a.actor_time().0.total_cmp(&b.actor_time().0));
    let mut actors: Vec<_> = script
        .characters
        .keys()
        .filter(|id| {
            scene
                .composition
                .layers
                .get(*id)
                .is_none_or(|layer| layer.visible)
        })
        .map(|id| {
            let base = actor_base(script);
            let mut pose = ActorPose {
                id: id.clone(),
                x: base.x,
                y: base.y,
                emotion: "neutral".into(),
                rotation: 0.0,
                scale: Vec2::new(1.0, 1.0),
                wave: 0.0,
                skeleton: motion_animation::humanoid(None, 0.0),
                rigid_parts: prepared.rigid_parts.get(id).cloned().unwrap_or_default(),
                mouth: None,
                view: FaceView::Front,
            };
            let mut walk_seconds = None;
            let mut gait = motion_animation::Gait::Walk;
            let mut world_travel = 0.0;
            let mut emotion_changes = Vec::new();
            let mut gesture = None;
            let mut set_emotion = |pose: &mut ActorPose, value: &String, at: f64| {
                if pose.emotion != *value {
                    pose.emotion = value.clone();
                    emotion_changes.push((value.clone(), time_seconds - at));
                }
            };
            for event in &events {
                let (at, actor) = event.actor_time();
                if actor != id || at > time_seconds {
                    continue;
                }
                match event {
                    // Positions come from the resolved move track below.
                    Event::Move { .. } => {}
                    Event::Emotion { value, .. } => set_emotion(&mut pose, value, at),
                    Event::Speak {
                        emotion: Some(value),
                        ..
                    } => set_emotion(&mut pose, value, at),
                    Event::Gesture {
                        value, duration, ..
                    } => {
                        // Validation guarantees a known gesture. The strongest active
                        // envelope wins; ties keep the earlier event.
                        let progress = (time_seconds - at) / duration;
                        let weight = motion_animation::gesture_envelope(progress);
                        if weight > pose.wave {
                            pose.wave = weight;
                            gesture = motion_animation::Gesture::parse(value)
                                .map(|g| (g, progress, weight));
                        }
                    }
                    Event::Speak { .. } | Event::Face { .. } => {}
                }
            }
            let (view, mirror) = facing_at(&events, id, time_seconds);
            pose.view = view;
            if let Some(track) = prepared.moves[scene_index].get(id) {
                // Each move starts where the previous one ended (resolved once);
                // the actor walks only while the active move is still changing.
                let position = track.value_at(time_seconds);
                (pose.x, pose.y) = (position.x, position.y);
                let active = track
                    .active(time_seconds)
                    .filter(|span| span.progress(time_seconds) < 1.0 && span.from != span.to);
                if let Some(span) = active {
                    walk_seconds = Some(time_seconds - span.at);
                    if let Some(Event::Move {
                        gait: Some(name), ..
                    }) = scene.events.get(span.order)
                    {
                        // Validation guarantees a known gait.
                        gait = motion_animation::Gait::parse(name).unwrap_or_default();
                    }
                    let dx = span.to.x - span.from.x;
                    world_travel = if dx > 0.0 {
                        1.0
                    } else if dx < 0.0 {
                        -1.0
                    } else {
                        0.0
                    };
                }
            }
            // Gait cycles are authored travelling toward local +x.
            let travel = world_travel * mirror.signum();
            let emotion_changes: Vec<_> = emotion_changes
                .iter()
                .map(|(emotion, since)| (emotion.as_str(), *since))
                .collect();
            let offsets = prepared.offsets(script, scene_index, id, time_seconds)?;
            let weights = scene.composition.sample_clip_weights(id, time_seconds);
            pose.wave *= weights.gesture;
            pose.skeleton = motion_animation::evaluate_humanoid_with_weights_and_offsets(
                &HumanoidState {
                    walk_seconds,
                    gait,
                    travel,
                    emotion_changes: &emotion_changes,
                    gesture,
                },
                &offsets,
                weights,
            )?;
            if let Some(rig) = prepared.rigs.get(id) {
                let tracks = prepared.joint_tracks[scene_index].get(id);
                let mut offsets = tracks
                    .map(|tracks| {
                        tracks
                            .iter()
                            .map(|(joint, track)| Ok((*joint, track.sample(time_seconds)?)))
                            .collect::<Result<Vec<_>, Error>>()
                    })
                    .transpose()?
                    .unwrap_or_default();
                // Procedural gait and gesture layers drive the standard limb
                // joints a rig defines, on top of authored joint keys.
                if let Some(seconds) = walk_seconds {
                    let travel = if travel == 0.0 { 1.0 } else { travel };
                    let motion = motion_animation::rig_gait(gait, seconds, travel, weights.walk);
                    add_rig_motion(rig, &mut offsets, &motion);
                }
                if let Some((gesture, progress, _)) = gesture {
                    // The rig layer shapes its own timing from progress.
                    let motion = motion_animation::rig_gesture(gesture, progress, weights.gesture);
                    add_rig_motion(rig, &mut offsets, &motion);
                }
                // Screen taps: the free hand presses the glass, last so it
                // reaches the screen wherever the pose above put it.
                if let Some(presses) = prepared.taps[scene_index].get(id) {
                    taps::apply(rig, presses, time_seconds, &mut offsets)
                        .map_err(|error| Error::InvalidAnimation(error.to_string()))?;
                }
                let invalid = |error: motion_rig::Error| Error::InvalidAnimation(error.to_string());
                if rig.has_acting() {
                    let input = motion_rig::ActingInput {
                        emotion: &pose.emotion,
                        blink: rig.blink_active(id, time_seconds),
                        view: pose.view,
                    };
                    let (parts, mouth) = rig.evaluate_acting(&offsets, input).map_err(invalid)?;
                    pose.rigid_parts = parts;
                    pose.mouth = mouth.map(|m| MouthVariants {
                        part: m.part,
                        shapes: m
                            .shapes
                            .into_iter()
                            .filter_map(|(name, part)| Some((viseme_from_name(name)?, part)))
                            .collect(),
                    });
                } else if !offsets.is_empty() {
                    pose.rigid_parts = rig.evaluate_parts(&offsets).map_err(invalid)?;
                }
                // Screen content is a pure function of scene time on the same
                // joint transforms, so preview, seek and export show one frame.
                for slot in rig.screen_ids() {
                    let content = scene.composition.screen_content(id, slot, time_seconds);
                    rig.apply_screen(&mut pose.rigid_parts, &offsets, slot, &content)
                        .map_err(invalid)?;
                }
            }
            if let Some(keys) = scene.composition.actor_tracks.get(id) {
                // Validation above guarantees this track and scene-clock sample are valid.
                let value = motion_animation::Track::new(
                    keys.iter()
                        .map(motion_script::ActorKey::animation_key)
                        .collect(),
                )
                .expect("validated actor track")
                .sample(time_seconds)
                .expect("validated scene time");
                pose.x += value.translation.x;
                pose.y += value.translation.y;
                pose.rotation = value.rotation;
                pose.scale = value.scale;
            }
            pose.scale.x *= mirror;
            Ok(pose)
        })
        .collect::<Result<Vec<_>, Error>>()?;
    apply_gaze(&mut actors, scene, prepared, time_seconds);
    actors.sort_by_key(|actor| {
        scene
            .composition
            .layers
            .get(&actor.id)
            .map_or(0, |layer| layer.z)
    });
    let composition = &scene.composition;
    let fade_in = if composition.fade_in > 0.0 {
        (time_seconds / composition.fade_in).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let fade_out = if composition.fade_out > 0.0 {
        ((scene.duration - time_seconds) / composition.fade_out).clamp(0.0, 1.0)
    } else {
        1.0
    };
    Ok(EvaluatedScene {
        time: time_seconds,
        camera: camera_at(&composition.camera, time_seconds),
        background_color: composition
            .background_keys
            .iter()
            .rev()
            .find(|key| key.at <= time_seconds)
            .map_or(&composition.background_color, |key| &key.color)
            .clone(),
        opacity: fade_in.min(fade_out),
        width: script.project.width,
        height: script.project.height,
        actors,
    })
}

#[cfg(test)]
mod acting_tests;
#[cfg(test)]
mod tests {
    use super::*;
    const SCRIPT: &str = r#"
project: {title: Hello Emma, fps: 24, width: 1920, height: 1080}
characters:
  EMMA:
    asset: characters/emma
    default_pose: idle
    voice: {provider: kokoro, preset: af_heart, language: en-us, speed: 1.0}
scenes:
  - id: intro
    duration: 8.0
    background: studio
    events:
      - {type: move, at: 0.0, actor: EMMA, x: 400, y: 700, duration: 2.0}
      - {type: emotion, at: 2.0, actor: EMMA, value: happy}
      - {type: gesture, at: 2.5, actor: EMMA, value: wave, duration: 1.0}
      - {type: speak, at: 3.0, actor: EMMA, text: Hello! Welcome to Kilsat Studio.}
      - {type: move, at: 4.0, actor: EMMA, x: 800, y: 700, duration: 2.0}
"#;
    #[test]
    fn validated_script_retains_immutable_snapshot_and_evaluation_boundaries() {
        let mut script = std::sync::Arc::new(motion_script::parse(SCRIPT).unwrap());
        let validated = ValidatedScript::new(script.clone()).unwrap();
        for time in [0.0, 2.5, 7.9] {
            assert_eq!(
                validated.evaluate(0, time).unwrap(),
                evaluate(&script, 0, time).unwrap()
            );
        }
        // Mutation through another owner copies rather than invalidating the snapshot.
        std::sync::Arc::make_mut(&mut script).scenes[0].duration = -1.0;
        assert!(evaluate(&script, 0, 0.0).is_err());
        assert!(validated.evaluate(0, 0.0).is_ok());
        assert!(validated.evaluate(1, 0.0).is_err());
        assert!(validated.evaluate(0, f64::NAN).is_err());
        assert!(ValidatedScript::new(script).is_err());
    }
    #[test]
    fn combined_script_deterministic_boundaries_and_seeking() {
        let script = motion_script::parse(SCRIPT).unwrap();
        assert_eq!(evaluate(&script, 0, 0.0).unwrap().actors[0].x, 960.0);
        assert_eq!(evaluate(&script, 0, 1.0).unwrap().actors[0].x, 680.0);
        let middle = evaluate(&script, 0, 3.0).unwrap();
        assert_eq!(middle.time, 3.0);
        for time in [8.0, 0.0, 1.25, 3.0] {
            assert_eq!(evaluate(&script, 0, time).unwrap().time, time);
        }
        assert_eq!(middle.actors[0].emotion, "happy");
        assert_eq!(middle.actors[0].wave, 1.0);
        assert_eq!(evaluate(&script, 0, 3.5).unwrap().actors[0].wave, 0.0);
        assert_eq!(evaluate(&script, 0, 5.0).unwrap().actors[0].x, 600.0);
        assert_eq!(evaluate(&script, 0, 8.0).unwrap().actors[0].x, 800.0);
        assert_eq!(middle, evaluate(&script, 0, 3.0).unwrap());
        assert_ne!(
            evaluate(&script, 0, 1.0 / 6.0).unwrap().actors[0].skeleton,
            evaluate(&script, 0, 2.0).unwrap().actors[0].skeleton
        );
        // Wave lifts the hand 65 units; the happy posture raises the chest 3 more.
        assert_eq!(middle.actors[0].skeleton[RIGHT_HAND].y, -88.0);
        assert!(evaluate(&script, 0, f64::NAN).is_err());
        assert!(evaluate(&script, 1, 0.0).is_err());
    }
    #[test]
    fn rule_layers_select_gesture_clip_and_blend_posture() {
        let script = motion_script::parse(&SCRIPT.replace("value: wave", "value: nod")).unwrap();
        let at = |t| evaluate(&script, 0, t).unwrap().actors[0].clone();
        // Nod moves the head, not the hand.
        assert!(at(2.75).skeleton[HEAD].y > at(2.4).skeleton[HEAD].y);
        assert_eq!(at(2.75).skeleton[RIGHT_HAND], at(2.4).skeleton[RIGHT_HAND]);
        assert_eq!(at(3.5).skeleton, at(3.9).skeleton);
        // Happy posture eases in after the emotion event at 2.0 s.
        let before = at(1.99).skeleton[CHEST].y;
        let during = at(2.0 + POSTURE_BLEND_SECONDS / 2.0).skeleton[CHEST].y;
        let after = at(2.0 + POSTURE_BLEND_SECONDS).skeleton[CHEST].y;
        assert!(before > during && during > after);
        assert_eq!(after, at(2.4).skeleton[CHEST].y);
        // Seeking backwards yields the same pose as evaluating directly.
        let forward = at(2.1);
        at(7.0);
        assert_eq!(at(2.1), forward);
        // A second emotion inside the blend window continues from the in-flight pose.
        let quick = motion_script::parse(&SCRIPT.replace(
            "      - {type: gesture",
            "      - {type: emotion, at: 2.1, actor: EMMA, value: sad}\n      - {type: gesture",
        ))
        .unwrap();
        let chest = |t| evaluate(&quick, 0, t).unwrap().actors[0].skeleton[CHEST].y;
        assert!((chest(2.1) - chest(2.0999)).abs() < 0.01);
        assert!(chest(2.1 + POSTURE_BLEND_SECONDS) > chest(1.99));
        for name in motion_animation::GESTURES {
            motion_script::parse(&SCRIPT.replace("value: wave", &format!("value: {name}")))
                .unwrap();
        }
    }
    #[test]
    fn speech_head_motion_follows_supplied_visemes_only() {
        let script = motion_script::parse(SCRIPT).unwrap();
        let base = evaluate(&script, 0, 3.0).unwrap();
        let mut open = base.clone();
        apply_speech(&mut open, &[("EMMA", Viseme::A)]);
        assert_eq!(
            open.actors[0].skeleton[HEAD].y,
            base.actors[0].skeleton[HEAD].y + SPEECH_HEAD_DIP
        );
        assert_eq!(
            open.actors[0].skeleton[CHEST],
            base.actors[0].skeleton[CHEST]
        );
        let mut silent = base.clone();
        apply_speech(&mut silent, &[("EMMA", Viseme::Rest), ("OTHER", Viseme::A)]);
        assert_eq!(silent, base);
        let mut none = base.clone();
        apply_speech(&mut none, &[]);
        assert_eq!(none, base);
    }
    #[test]
    fn rejects_invalid_commands_and_overlapping_moves() {
        assert!(motion_script::parse(&SCRIPT.replace("at: 4.0", "at: 1.0")).is_err());
        assert!(matches!(
            motion_script::parse(&SCRIPT.replace("value: wave", "value: dance")),
            Err(motion_script::Error::Unsupported(message)) if message.contains("wave, point, nod, shrug")
        ));
        assert!(motion_script::parse(&SCRIPT.replace("value: happy", "value: confused")).is_err());
        assert!(motion_script::parse(&SCRIPT.replace("x: 800", "x: .nan")).is_err());
        assert!(motion_script::parse(&SCRIPT.replace("duration: 8.0", "duration: 5.0")).is_err());
    }
    #[test]
    fn exclusive_frame_count_handles_decimal_and_fractional_durations() {
        assert_eq!(frame_count(0.28, 25, 1000).unwrap(), 7);
        assert_eq!(frame_count(8.3, 30, 1000).unwrap(), 249);
        assert_eq!(frame_count(0.3, 24, 1000).unwrap(), 8);
        assert_eq!(frame_count(1.0, 24, 24).unwrap(), 24);
        assert_eq!(frame_count(0.001, 24, 1).unwrap(), 1);
        assert!(matches!(frame_count(1.0, 24, 23), Err(Error::FrameBudget)));
        assert!(matches!(
            frame_count(f64::MAX, 24, 1000),
            Err(Error::FrameBudget)
        ));
        for duration in [f64::NAN, f64::INFINITY, 0.0, -1.0] {
            assert!(matches!(
                frame_count(duration, 24, 1000),
                Err(Error::InvalidFrameClock)
            ));
        }
        assert!(matches!(
            frame_count(1.0, 0, 1000),
            Err(Error::InvalidFrameClock)
        ));
        assert!(matches!(
            frame_count(1.0, 24, 0),
            Err(Error::InvalidFrameClock)
        ));
    }
}

#[cfg(test)]
mod composition_tests {
    use super::*;
    #[test]
    fn camera_layers_fades_seek_deterministically_on_scene_clock() {
        let script = motion_script::parse(
            r##"
project: {width: 640, height: 360}
characters: {A: {asset: a}, B: {asset: b}, C: {asset: c}}
scenes:
- id: shot
  duration: 4
  events: []
  composition:
    background_color: '#aabbcc'
    fade_in: 1
    fade_out: 1
    camera: [{at: 0, x: 0, y: 0, zoom: 1}, {at: 4, x: 100, y: 40, zoom: 3}]
    layers: {A: {z: 2}, B: {z: -1}, C: {visible: false}}
"##,
        )
        .unwrap();
        let middle = evaluate(&script, 0, 2.0).unwrap();
        assert_eq!(
            middle
                .actors
                .iter()
                .map(|a| a.id.as_str())
                .collect::<Vec<_>>(),
            vec!["B", "A"]
        );
        assert_eq!(
            middle.camera,
            CameraPose {
                x: 50.0,
                y: 20.0,
                zoom: 2.0
            }
        );
        assert_eq!(
            middle.camera.project(320.0, 180.0, 640, 360),
            (220.0, 140.0)
        );
        assert_eq!(middle.background_color, "#aabbcc");
        assert_eq!(middle.opacity, 1.0);
        assert_eq!(evaluate(&script, 0, 0.0).unwrap().opacity, 0.0);
        assert_eq!(evaluate(&script, 0, 0.5).unwrap().opacity, 0.5);
        assert_eq!(evaluate(&script, 0, 3.5).unwrap().opacity, 0.5);
        assert_eq!(evaluate(&script, 0, 4.0).unwrap().opacity, 0.0);
        assert_eq!(middle, evaluate(&script, 0, 2.0).unwrap());
    }
}

#[cfg(test)]
mod actor_track_tests {
    use super::*;
    #[test]
    fn root_tracks_layer_on_moves_share_clock_and_transform_face_points() {
        let script = motion_script::parse(
            r#"
project: {width: 640, height: 360}
characters: {A: {asset: a}, B: {asset: b}}
scenes:
- id: shot
  duration: 4
  events: [{type: move, at: 0, actor: A, x: 420, y: 252, duration: 4}]
  composition:
    actor_tracks:
      A:
      - {at: 0, x: 0, easing: smoothstep}
      - {at: 4, x: 80, rotation: 3.141592653589793, scale_x: 2, scale_y: 3}
      B:
      - {at: 1, y: 10, easing: hold}
      - {at: 3, y: 90}
"#,
        )
        .unwrap();
        let start = evaluate(&script, 0, 0.0).unwrap();
        assert_eq!(start.actors[0].x, 320.0);
        assert_eq!(start.actors[1].y, 262.0); // before first key clamps
        let quarter = evaluate(&script, 0, 1.0).unwrap();
        assert_eq!(quarter.actors[0].x, 345.0 + 12.5);
        assert_eq!(quarter.actors[0].scale, Vec2::new(1.15625, 1.3125));
        assert_eq!(evaluate(&script, 0, 2.99).unwrap().actors[1].y, 262.0);
        assert_eq!(evaluate(&script, 0, 3.0).unwrap().actors[1].y, 342.0);
        let end = evaluate(&script, 0, 4.0).unwrap();
        let a = &end.actors[0];
        assert_eq!(a.x, 500.0);
        let face = a.local_to_world(Vec2::new(0.0, -90.0));
        assert!((face.x - 500.0).abs() < 1e-9);
        assert!((face.y - 522.0).abs() < 1e-9);
        assert_eq!(quarter, evaluate(&script, 0, 1.0).unwrap());
    }
}

#[cfg(test)]
mod background_key_tests {
    use super::*;
    #[test]
    fn background_steps_are_inclusive_and_seek_order_independent() {
        let script = motion_script::parse(
            r##"
characters: {}
scenes:
- id: shot
  duration: 4
  events: []
  composition:
    background_color: '#112233'
    background_keys:
    - {at: 1, color: '#445566'}
    - {at: 3, color: '#778899'}
    - {at: 4, color: '#aabbcc'}
"##,
        )
        .unwrap();
        for (time, color) in [
            (4.0, "#aabbcc"),
            (0.0, "#112233"),
            (3.0, "#778899"),
            (0.999, "#112233"),
            (1.0, "#445566"),
            (2.999, "#445566"),
            (4.0, "#aabbcc"),
        ] {
            assert_eq!(evaluate(&script, 0, time).unwrap().background_color, color);
        }
        let mut at_zero = script.clone();
        at_zero.scenes[0].composition.background_keys[0].at = 0.0;
        assert_eq!(
            evaluate(&at_zero, 0, 0.0).unwrap().background_color,
            "#445566"
        );
    }
}

#[cfg(test)]
mod bone_track_scene_tests {
    use super::*;
    #[test]
    fn authored_local_keys_seek_interpolate_and_preserve_rule_gesture() {
        let source = "characters: {A: {asset: a}}\nscenes: [{id: shot, duration: 4, events: [{type: gesture, at: 0, actor: A, value: wave, duration: 4}]}]";
        let base = motion_script::parse(source).unwrap();
        let source = motion_script::apply_bone_track(
            source,
            0,
            "A",
            "right_hand",
            &[
                motion_script::ActorKey::default(),
                motion_script::ActorKey {
                    at: 4.0,
                    x: 40.0,
                    ..Default::default()
                },
            ],
        )
        .unwrap();
        let script = motion_script::parse(&source).unwrap();
        for time in [2.0, 0.0, 4.0, 1.0, 2.0] {
            let original = evaluate(&base, 0, time).unwrap();
            let actual = evaluate(&script, 0, time).unwrap();
            let joint = motion_animation::RIGHT_HAND;
            assert_eq!(
                actual.actors[0].skeleton[joint].x,
                original.actors[0].skeleton[joint].x + time * 10.0
            );
            assert_eq!(
                actual.actors[0].skeleton[joint].y,
                original.actors[0].skeleton[joint].y
            );
            assert_eq!(
                actual.actors[0].skeleton[motion_animation::HEAD],
                original.actors[0].skeleton[motion_animation::HEAD]
            );
        }
    }
}

#[cfg(test)]
mod clip_weight_tests {
    use super::*;

    #[test]
    fn actor_intensities_isolate_rules_and_keep_authored_and_speech_motion() {
        let source = "characters: {A: {asset: a}, B: {asset: b}}\nscenes: [{id: shot, duration: 4, events: [{type: gesture, actor: A, at: 0, value: wave, duration: 4}, {type: gesture, actor: B, at: 0, value: wave, duration: 4}, {type: emotion, actor: A, at: 0, value: happy}, {type: move, actor: A, at: 0, x: 500, y: 400, duration: 4}]}]";
        let source = motion_script::apply_bone_track(
            source,
            0,
            "A",
            "right_hand",
            &[motion_script::ActorKey {
                x: 20.0,
                ..Default::default()
            }],
        )
        .unwrap();
        let original = motion_script::parse(&source).unwrap();
        let off = motion_script::ClipWeights {
            posture: 0.0,
            walk: 0.0,
            gesture: 0.0,
        };
        let edited = motion_script::apply_clip_weights(&source, 0, "A", off).unwrap();
        let script = motion_script::parse(&edited).unwrap();
        let mut neutral = script.clone();
        neutral.scenes[0].events.clear();
        for time in [2.0, 0.0, 4.0, 0.02, 1.0, 2.0] {
            let before = evaluate(&original, 0, time).unwrap();
            let mut actual = evaluate(&script, 0, time).unwrap();
            let rest = evaluate(&neutral, 0, time).unwrap();
            assert_eq!(actual.actors[1], before.actors[1]);
            assert_eq!(actual.actors[0].skeleton, rest.actors[0].skeleton);
            assert_eq!(actual.actors[0].wave, 0.0);
            assert_eq!(actual.actors[0].x, before.actors[0].x);
            assert_eq!(actual.actors[0].y, before.actors[0].y);
            assert_eq!(actual.actors[0].emotion, before.actors[0].emotion);
            let head_y = actual.actors[0].skeleton[HEAD].y;
            apply_speech(&mut actual, &[("A", Viseme::A)]);
            assert_eq!(actual.actors[0].skeleton[HEAD].y, head_y + SPEECH_HEAD_DIP);
        }
        let half = motion_script::apply_clip_weights(
            &source,
            0,
            "A",
            motion_script::ClipWeights {
                gesture: 0.5,
                ..Default::default()
            },
        )
        .unwrap();
        let half = evaluate(&motion_script::parse(&half).unwrap(), 0, 2.0).unwrap();
        let full = evaluate(&original, 0, 2.0).unwrap();
        assert_eq!(half.actors[0].wave, full.actors[0].wave * 0.5);
        assert_eq!(half.actors[1].wave, full.actors[1].wave);
    }
}

#[cfg(test)]
mod animated_clip_weight_tests {
    use super::*;

    #[test]
    fn animated_strengths_match_static_evaluation_on_shared_scene_clock() {
        let source = "characters: {A: {asset: a}, B: {asset: b}}\nscenes: [{id: shot, duration: 4, events: [{type: gesture, actor: A, at: 0, value: wave, duration: 4}, {type: gesture, actor: B, at: 0, value: wave, duration: 4}, {type: emotion, actor: A, at: 0, value: happy}, {type: move, actor: A, at: 0, x: 500, y: 400, duration: 4}]}]";
        let keys = vec![
            motion_script::ClipWeightKeyframe {
                time: 0.0,
                weights: motion_script::ClipWeights {
                    posture: 0.0,
                    walk: 1.0,
                    gesture: 0.2,
                },
                easing: motion_script::ActorEasing::Smoothstep,
            },
            motion_script::ClipWeightKeyframe {
                time: 4.0,
                weights: motion_script::ClipWeights {
                    posture: 1.0,
                    walk: 0.0,
                    gesture: 0.8,
                },
                easing: motion_script::ActorEasing::Linear,
            },
        ];
        let tracked_source = motion_script::apply_clip_weight_track(source, 0, "A", keys).unwrap();
        let tracked = motion_script::parse(&tracked_source).unwrap();
        let original = motion_script::parse(source).unwrap();
        // Deliberately seek out of order: evaluation must not accumulate state.
        for time in [4.0, 1.0, 0.0, 2.5, 0.5, 1.0] {
            let t = time / 4.0;
            let eased = t * t * (3.0 - 2.0 * t);
            let expected_weights = motion_script::ClipWeights {
                posture: eased,
                walk: 1.0 - eased,
                gesture: 0.2 + (0.8 - 0.2) * eased,
            };
            let expected_source =
                motion_script::apply_clip_weights(source, 0, "A", expected_weights).unwrap();
            let expected =
                evaluate(&motion_script::parse(&expected_source).unwrap(), 0, time).unwrap();
            let actual = evaluate(&tracked, 0, time).unwrap();
            let before = evaluate(&original, 0, time).unwrap();
            assert_eq!(actual.actors, expected.actors);
            assert_eq!(actual.actors[1], before.actors[1]);
            assert_eq!(actual.actors[0].x, before.actors[0].x);
            assert_eq!(actual.actors[0].emotion, before.actors[0].emotion);
        }
    }
}

#[cfg(test)]
mod live_instance_tests {
    use super::*;
    use motion_animation::{Transform2, HUMANOID_BONES};
    fn script() -> Script {
        motion_script::parse("characters: {A: {asset: a}}\nanimation_clips:\n  first:\n    duration: 2\n    bone_tracks:\n      head: [{at: 0, x: 20}, {at: 2, x: 60}]\n  second:\n    duration: 1\n    bone_tracks:\n      head: [{at: 0, x: 100}]\n      right_hand: [{at: 0, y: 20}]\nscenes:\n  - id: shot\n    duration: 6\n    events: []\n    composition:\n      bone_tracks:\n        A:\n          head: [{at: 0, x: 8}]\n").unwrap()
    }
    fn instance(
        id: &str,
        clip: &str,
        at: f64,
        duration: f64,
        weight: f64,
    ) -> motion_script::ClipInstance {
        motion_script::ClipInstance {
            id: id.into(),
            clip: clip.into(),
            actor: "A".into(),
            at,
            duration,
            weight,
            blend_mode: motion_script::ClipBlendMode::Override,
            normalized_group: None,
            fade_in: 0.0,
            fade_out: 0.0,
            weight_keys: Vec::new(),
            looping: false,
            joint_mask: Vec::new(),
        }
    }
    fn offsets(script: &Script, time: f64) -> Vec<(usize, Transform2)> {
        script.validate().unwrap();
        PreparedAnimation::new(script)
            .unwrap()
            .offsets(script, 0, "A", time)
            .unwrap()
    }
    fn bone(offsets: &[(usize, Transform2)], name: &str) -> Transform2 {
        let joint = HUMANOID_BONES
            .iter()
            .position(|bone| *bone == name)
            .unwrap();
        offsets
            .iter()
            .find(|(i, _)| *i == joint)
            .map(|(_, v)| *v)
            .unwrap_or_default()
    }
    #[test]
    fn normalized_groups_are_order_invariant_with_per_joint_residual_and_stack_position() {
        let mut script = script();
        let mut first = instance("a", "first", 1.0, 2.0, 0.25);
        first.normalized_group = Some("reach".into());
        let mut second = instance("b", "second", 1.0, 2.0, 0.25);
        second.normalized_group = Some("reach".into());
        script.scenes[0].composition.clip_instances = vec![first.clone(), second.clone()];
        // t=2 samples first x=40, second x=100, base x=8.
        // Total .5 leaves .5*8, yielding 39. Right hand only has .25*20.
        let expected = offsets(&script, 2.0);
        assert_eq!(bone(&expected, "head").translation.x, 39.0);
        assert_eq!(bone(&expected, "right_hand").translation.y, 5.0);
        script.scenes[0].composition.clip_instances.reverse();
        assert_eq!(offsets(&script, 2.0), expected);
        first.weight = 1.0;
        second.weight = 1.0;
        script.scenes[0].composition.clip_instances = vec![first.clone(), second.clone()];
        assert_eq!(bone(&offsets(&script, 2.0), "head").translation.x, 70.0);
        second.joint_mask = vec!["right_hand".into()];
        script.scenes[0].composition.clip_instances = vec![first.clone(), second.clone()];
        assert_eq!(bone(&offsets(&script, 2.0), "head").translation.x, 40.0);
        let mut additive = instance("plus", "first", 1.0, 2.0, 0.5);
        additive.blend_mode = motion_script::ClipBlendMode::Additive;
        first.weight = 0.5;
        script.scenes[0].composition.clip_instances = vec![first.clone(), additive.clone()];
        assert_eq!(bone(&offsets(&script, 2.0), "head").translation.x, 44.0);
        script.scenes[0].composition.clip_instances = vec![additive, first];
        assert_eq!(bone(&offsets(&script, 2.0), "head").translation.x, 34.0);
    }

    #[test]
    fn normalized_group_rotation_reference_ties_and_scale_have_independent_results() {
        let mut script = script();
        let key = |rotation, scale_x| motion_script::ActorKey {
            rotation,
            scale_x,
            ..motion_script::ActorKey::default()
        };
        script
            .animation_clips
            .get_mut("first")
            .unwrap()
            .bone_tracks
            .insert("head".into(), vec![key(3.0, 2.0)]);
        script
            .animation_clips
            .get_mut("second")
            .unwrap()
            .bone_tracks
            .insert("head".into(), vec![key(-3.0, 4.0)]);
        let mut first = instance("a", "first", 0.0, 4.0, 1.0);
        first.normalized_group = Some("turn".into());
        let mut second = instance("b", "second", 0.0, 4.0, 1.0);
        second.normalized_group = Some("turn".into());
        script.scenes[0].composition.clip_instances = vec![first.clone(), second.clone()];
        let value = bone(&offsets(&script, 1.0), "head");
        assert!(value.rotation.abs() < 1e-12);
        assert_eq!(value.scale.x, 3.0);
        script.scenes[0].composition.clip_instances.reverse();
        assert_eq!(bone(&offsets(&script, 1.0), "head"), value);
        for weight in [0.4999, 0.5, 0.5001] {
            first.weight = weight;
            second.weight = weight;
            script.scenes[0].composition.clip_instances = vec![first.clone(), second.clone()];
            assert!(bone(&offsets(&script, 1.0), "head").rotation.abs() < 1e-12);
        }
        first.weight = 0.25;
        second.weight = 0.25;
        script.scenes[0].composition.clip_instances = vec![first.clone(), second.clone()];
        let value = bone(&offsets(&script, 1.0), "head");
        assert_eq!(value.rotation, 0.0);
        assert_eq!(value.scale.x, 2.0);
        script
            .animation_clips
            .get_mut("first")
            .unwrap()
            .bone_tracks
            .insert("head".into(), vec![key(-std::f64::consts::PI, 2.0)]);
        second.weight = 0.0;
        first.weight = 0.5;
        script.scenes[0].composition.clip_instances = vec![first, second];
        assert!(
            (bone(&offsets(&script, 1.0), "head").rotation - std::f64::consts::FRAC_PI_2).abs()
                < 1e-12
        );
    }

    #[test]
    fn normalized_groups_keep_zero_fallback_and_sample_loop_envelope_once_during_seeks() {
        let mut script = script();
        let mut first = instance("a", "first", 1.0, 4.0, 0.5);
        first.normalized_group = Some("reach".into());
        first.looping = true;
        first.fade_in = 2.0;
        first.fade_out = 1.0;
        first.weight_keys = vec![motion_script::InstanceWeightKey {
            time: 0.0,
            weight: 0.5,
            easing: motion_script::ActorEasing::Linear,
        }];
        let mut second = instance("b", "second", 0.0, 1.0, 0.0);
        second.normalized_group = Some("reach".into());
        script.scenes[0].composition.clip_instances = vec![first, second];
        script.validate().unwrap();
        let prepared = PreparedAnimation::new(&script).unwrap();
        // Effective weights .125/.25/.125; loop clip x40/20/50.
        for (time, x) in [
            (2.0, 12.0),
            (3.0, 11.0),
            (4.5, 13.25),
            (5.0, 8.0),
            (0.0, 8.0),
            (3.0, 11.0),
        ] {
            let actual = prepared.offsets(&script, 0, "A", time).unwrap();
            assert!(
                (bone(&actual, "head").translation.x - x).abs() < 1e-12,
                "{time}"
            );
        }
    }

    #[test]
    fn additive_accumulates_nonidentity_base_and_mixed_modes_keep_saved_order() {
        let mut script = script();
        let mut first = instance("a", "first", 1.0, 2.0, 0.5);
        first.blend_mode = motion_script::ClipBlendMode::Additive;
        let mut second = instance("b", "second", 1.0, 2.0, 0.5);
        second.blend_mode = motion_script::ClipBlendMode::Additive;
        script.scenes[0].composition.clip_instances = vec![first, second];
        // At local1 first samples40: base8 + .5*40 + .5*100 =78.
        let actual = offsets(&script, 2.0);
        assert_eq!(bone(&actual, "head").translation.x, 78.0);
        assert_eq!(bone(&actual, "right_hand").translation.y, 10.0);
        script.scenes[0].composition.clip_instances[1].blend_mode =
            motion_script::ClipBlendMode::Override;
        // Additive then override: (8 +20)*.5 +100*.5 =64.
        assert_eq!(bone(&offsets(&script, 2.0), "head").translation.x, 64.0);
        script.scenes[0].composition.clip_instances.reverse();
        // Override then additive: 8*.5 +100*.5 +20 =74.
        assert_eq!(bone(&offsets(&script, 2.0), "head").translation.x, 74.0);
    }

    #[test]
    fn additive_loops_masks_missing_joints_and_instance_envelopes_share_sampling() {
        let mut script = script();
        let mut layer = instance("add", "first", 1.0, 5.0, 0.8);
        layer.blend_mode = motion_script::ClipBlendMode::Additive;
        layer.looping = true;
        layer.fade_in = 4.0;
        layer.fade_out = 1.0;
        layer.joint_mask = vec!["head".into(), "right_hand".into()];
        layer.weight_keys = vec![motion_script::InstanceWeightKey {
            time: 0.0,
            weight: 0.5,
            easing: motion_script::ActorEasing::Linear,
        }];
        script.scenes[0].composition.clip_instances = vec![layer];
        // Static .8 * keyed .5 * entrance * exit, applied once to
        // the original-speed sample, including cycle resets at times3 and5.
        // Entrance occupies local0..4; exit occupies local4..5.
        for (time, expected) in [
            (0.0, 8.0),
            (1.0, 8.0),
            (2.0, 12.0),
            (3.0, 12.0),
            (4.5, 25.5),
            (5.0, 16.0),
            (5.5, 14.0),
            (6.0, 8.0),
            (2.0, 12.0),
        ] {
            let actual = offsets(&script, time);
            assert!((bone(&actual, "head").translation.x - expected).abs() < 1e-12);
            assert_eq!(bone(&actual, "right_hand"), Transform2::default());
        }
        let prepared = ValidatedScript::new(std::sync::Arc::new(script.clone())).unwrap();
        for time in [5.5, 6.0, 4.5, 0.0, 3.0, 5.0, 2.0, 1.0, 2.0] {
            assert_eq!(
                prepared.evaluate(0, time).unwrap(),
                evaluate(&script, 0, time).unwrap()
            );
        }
        script.scenes[0].composition.clip_instances[0].joint_mask = vec!["right_hand".into()];
        assert_eq!(bone(&offsets(&script, 3.0), "head").translation.x, 8.0);
    }

    #[test]
    fn additive_parent_rotation_and_translation_match_independent_authored_tracks() {
        let source = "characters: {A: {asset: a}}\nanimation_clips:\n  delta:\n    duration: 2\n    bone_tracks:\n      chest: [{at: 0, rotation: -40, x: 12}]\n      right_hand: [{at: 0, x: 10}]\nscenes:\n  - id: shot\n    duration: 2\n    events: []\n    composition:\n      bone_tracks:\n        A:\n          chest: [{at: 0, rotation: 30, x: 8}]\n          right_hand: [{at: 0, x: 6}]\n";
        let mut script = motion_script::parse(source).unwrap();
        let mut layer = instance("a", "delta", 0.0, 2.0, 0.5);
        layer.blend_mode = motion_script::ClipBlendMode::Additive;
        script.scenes[0].composition.clip_instances = vec![layer];
        let expected_source = source
            .replace("rotation: 30, x: 8", "rotation: 10, x: 14")
            .replace(
                "right_hand: [{at: 0, x: 6}]",
                "right_hand: [{at: 0, x: 11}]",
            );
        let expected = motion_script::parse(&expected_source).unwrap();
        let actual = evaluate(&script, 0, 1.0).unwrap();
        let expected = evaluate(&expected, 0, 1.0).unwrap();
        for (actual, expected) in actual.actors[0]
            .skeleton
            .iter()
            .zip(expected.actors[0].skeleton)
        {
            assert!((actual.x - expected.x).abs() < 1e-10);
            assert!((actual.y - expected.y).abs() < 1e-10);
        }
    }

    #[test]
    fn additive_combination_bounds_fail_without_publishing_a_scene() {
        let mut script = script();
        script
            .animation_clips
            .get_mut("second")
            .unwrap()
            .bone_tracks
            .get_mut("head")
            .unwrap()[0]
            .x = 1000.0;
        let mut layer = instance("large", "second", 0.0, 2.0, 1.0);
        layer.blend_mode = motion_script::ClipBlendMode::Additive;
        script.scenes[0].composition.clip_instances = vec![layer];
        let prepared = ValidatedScript::new(std::sync::Arc::new(script.clone())).unwrap();
        assert!(matches!(
            evaluate(&script, 0, 1.0),
            Err(Error::Animation(motion_animation::Error::InvalidPose))
        ));
        assert!(matches!(
            prepared.evaluate(0, 1.0),
            Err(Error::Animation(motion_animation::Error::InvalidPose))
        ));
        assert!(prepared.evaluate(0, 2.0).is_ok());
    }

    #[test]
    fn loops_repeat_original_seconds_and_masks_preserve_unselected_offsets() {
        let mut script = script();
        let mut layer = instance("loop", "first", 1.0, 4.0, 0.5);
        layer.looping = true;
        layer.joint_mask = vec!["head".into()];
        script.scenes[0].composition.clip_instances = vec![layer];
        // Clip x20→60 over two seconds. Base head x8; equal blend.
        for (time, expected) in [
            (1.0, 14.0),
            (1.5, 19.0),
            (3.0, 14.0),
            (4.5, 29.0),
            (2.0, 24.0),
            (5.0, 8.0),
        ] {
            assert_eq!(
                bone(&offsets(&script, time), "head").translation.x,
                expected
            );
        }
        script.scenes[0].composition.clip_instances[0].joint_mask = vec!["right_hand".into()];
        assert_eq!(bone(&offsets(&script, 2.0), "head").translation.x, 8.0);
        script.scenes[0].composition.clip_instances[0].clip = "second".into();
        assert_eq!(
            bone(&offsets(&script, 2.0), "right_hand").translation.y,
            10.0
        );
        assert_eq!(bone(&offsets(&script, 2.0), "head").translation.x, 8.0);
        script.scenes[0].composition.clip_instances[0]
            .joint_mask
            .clear();
        assert_eq!(bone(&offsets(&script, 2.0), "head").translation.x, 54.0);
        script.scenes[0].composition.clip_instances[0].clip = "first".into();
        script.scenes[0].composition.clip_instances[0].looping = false;
        assert_eq!(bone(&offsets(&script, 2.0), "head").translation.x, 19.0);
    }

    #[test]
    fn masked_layers_keep_source_order_and_fades_do_not_restart_each_loop() {
        let mut script = script();
        let mut looping = instance("loop", "first", 1.0, 4.0, 1.0);
        looping.looping = true;
        looping.fade_in = 4.0;
        looping.joint_mask = vec!["head".into()];
        let mut overlay = instance("overlay", "second", 1.0, 4.0, 0.5);
        overlay.joint_mask = vec!["head".into()];
        script.scenes[0].composition.clip_instances = vec![looping, overlay];
        // At local2: cycle reset x20 but fade=.5, so base8 ->14 ->57.
        assert_eq!(bone(&offsets(&script, 3.0), "head").translation.x, 57.0);
        assert_eq!(
            bone(&offsets(&script, 3.0), "right_hand"),
            Transform2::default()
        );
        script.scenes[0].composition.clip_instances.reverse();
        // Reversed: base8 ->54 ->37. Fades stay on instance time.
        assert_eq!(bone(&offsets(&script, 3.0), "head").translation.x, 37.0);
    }

    #[test]
    fn envelope_product_blends_once_and_matches_prepared_seek_evaluation() {
        let mut script = script();
        let mut layer = instance("envelope", "second", 1.0, 4.0, 0.8);
        layer.fade_in = 1.0;
        layer.fade_out = 1.0;
        layer.weight_keys = vec![
            motion_script::InstanceWeightKey {
                time: 0.0,
                weight: 0.5,
                easing: motion_script::ActorEasing::Linear,
            },
            motion_script::InstanceWeightKey {
                time: 4.0,
                weight: 0.5,
                easing: motion_script::ActorEasing::Linear,
            },
        ];
        script.scenes[0].composition.clip_instances = vec![layer];
        // Constant target100, base8; .8 static * .5 keyed * .5 fade = .2.
        for (time, expected) in [
            (1.5, 26.4),
            (3.0, 44.8),
            (4.5, 26.4),
            (1.0, 8.0),
            (5.0, 8.0),
            (1.5, 26.4),
        ] {
            let actual = bone(&offsets(&script, time), "head").translation.x;
            assert!((actual - expected).abs() < 1e-12);
        }
        let prepared = ValidatedScript::new(std::sync::Arc::new(script.clone())).unwrap();
        for time in [4.5, 1.0, 3.0, 5.0, 1.5] {
            assert_eq!(
                prepared.evaluate(0, time).unwrap(),
                evaluate(&script, 0, time).unwrap()
            );
        }
        script.scenes[0].composition.clip_instances[0].weight = 0.0;
        script.scenes[0].composition.clip_instances[0].fade_out = f64::NAN;
        assert!(matches!(evaluate(&script, 0, 0.0), Err(Error::Script(_))));
        assert!(matches!(
            ValidatedScript::new(std::sync::Arc::new(script)),
            Err(Error::Script(_))
        ));
    }

    #[test]
    fn independently_expected_weights_order_and_missing_bones() {
        let mut script = script();
        for (weight, expected) in [(0.0, 8.0), (0.5, 24.0), (1.0, 40.0)] {
            script.scenes[0].composition.clip_instances =
                vec![instance("a", "first", 1.0, 2.0, weight)];
            let actual = offsets(&script, 2.0);
            assert_eq!(bone(&actual, "head").translation.x, expected);
            assert_eq!(bone(&actual, "right_hand"), Transform2::default());
        }
        script.scenes[0].composition.clip_instances = vec![
            instance("a", "first", 1.0, 2.0, 0.5),
            instance("b", "second", 1.0, 2.0, 0.5),
        ];
        let actual = offsets(&script, 2.0);
        assert_eq!(bone(&actual, "head").translation.x, 62.0);
        assert_eq!(bone(&actual, "right_hand").translation.y, 10.0);
        script.scenes[0].composition.clip_instances.reverse();
        assert_eq!(bone(&offsets(&script, 2.0), "head").translation.x, 47.0);
    }
    #[test]
    fn half_open_compressed_adjacent_windows_endpoint_holding_and_seeks() {
        let mut script = script();
        script.scenes[0].composition.clip_instances = vec![
            instance("a", "first", 1.0, 0.5, 1.0),
            instance("b", "second", 1.5, 0.5, 1.0),
        ];
        for (time, expected) in [
            (1.25, 40.0),
            (0.0, 8.0),
            (1.0, 20.0),
            (1.5, 100.0),
            (2.0, 8.0),
            (6.0, 8.0),
            (1.25, 40.0),
        ] {
            assert_eq!(
                bone(&offsets(&script, time), "head").translation.x,
                expected
            );
        }
        script
            .animation_clips
            .get_mut("first")
            .unwrap()
            .bone_tracks
            .get_mut("head")
            .unwrap()[0]
            .at = 0.5;
        assert_eq!(bone(&offsets(&script, 1.0), "head").translation.x, 20.0);
        assert_eq!(bone(&offsets(&script, 1.125), "head").translation.x, 20.0);
        script
            .animation_clips
            .get_mut("first")
            .unwrap()
            .bone_tracks
            .get_mut("head")
            .unwrap()[1]
            .at = 1.0;
        assert_eq!(bone(&offsets(&script, 1.375), "head").translation.x, 60.0);
    }
    #[test]
    fn prepared_snapshot_shared_by_clone_reuses_bounded_storage_and_matches_direct() {
        let mut script = script();
        script.scenes[0].composition.clip_instances = vec![instance("a", "first", 1.0, 2.0, 0.5)];
        let raw = std::sync::Arc::new(script);
        let prepared = ValidatedScript::new(raw.clone()).unwrap();
        let clone = prepared.clone();
        assert!(std::sync::Arc::ptr_eq(&prepared.prepared, &clone.prepared));
        let track = prepared.prepared.library["first"][HEAD].as_ref().unwrap()
            as *const motion_animation::Track;
        for time in [2.0, 0.0, 3.0, 1.0, 2.000020833333333, 2.0] {
            assert_eq!(
                prepared.evaluate(0, time).unwrap(),
                evaluate(&raw, 0, time).unwrap()
            );
            assert_eq!(
                track,
                clone.prepared.library["first"][HEAD].as_ref().unwrap()
                    as *const motion_animation::Track
            );
        }
        assert_eq!(prepared.prepared.library.len(), 1);
        assert!(!prepared.prepared.library.contains_key("second"));
        assert_eq!(prepared.prepared.scenes.len(), 1);
    }
    #[test]
    fn unused_library_is_not_prepared_and_duplicate_references_share_one_definition() {
        let mut script = script();
        let unused = PreparedAnimation::new(&script).unwrap();
        assert!(unused.library.is_empty());
        assert_eq!(unused.scenes[0]["A"].iter().flatten().count(), 1);
        script.scenes[0].composition.clip_instances = vec![
            instance("a", "first", 1.0, 2.0, 0.5),
            instance("b", "first", 2.0, 2.0, 0.0),
        ];
        script.validate().unwrap();
        let prepared = ValidatedScript::new(std::sync::Arc::new(script)).unwrap();
        assert_eq!(prepared.prepared.library.len(), 1);
        assert!(prepared.prepared.library.contains_key("first"));
        assert!(!prepared.prepared.library.contains_key("second"));
        let cloned = prepared.clone();
        assert!(std::sync::Arc::ptr_eq(&prepared.prepared, &cloned.prepared));
        for time in [3.0, 0.0, 1.5, 2.0, 1.5] {
            prepared.evaluate(0, time).unwrap();
        }
        assert_eq!(prepared.prepared.library.len(), 1);
        assert_eq!(
            prepared.prepared.library["first"].iter().flatten().count(),
            1
        );
    }

    #[test]
    fn empty_instances_preserve_authored_geometry_and_independent_base_speech_root() {
        let script = script();
        let at = evaluate(&script, 0, 2.0).unwrap();
        assert_eq!(at.actors[0].skeleton[HEAD], Vec2::new(8.0, -90.0));
        let mut live = script.clone();
        live.scenes[0].composition.clip_instances = vec![instance("a", "first", 1.0, 2.0, 0.5)];
        let mut actual = evaluate(&live, 0, 2.0).unwrap();
        assert_eq!(actual.actors[0].skeleton[HEAD], Vec2::new(24.0, -90.0));
        assert_eq!(actual.actors[0].x, at.actors[0].x);
        assert_eq!(actual.actors[0].emotion, at.actors[0].emotion);
        apply_speech(&mut actual, &[("A", Viseme::A)]);
        assert_eq!(actual.actors[0].skeleton[HEAD], Vec2::new(24.0, -87.0));
    }

    const ARM_RIG: &str = "characters:
  PUPPET:
    asset: custom
    rig:
      version: 1
      id: puppet
      joints:
        - {id: root, parent: null}
        - {id: arm, parent: root, x: 10}
      parts:
        - {id: base, joint: root, points: [[0, 0], [1, 0], [0, 1]], color: [1, 2, 3, 255]}
        - {id: hand, joint: arm, points: [[0, 0], [1, 0], [0, 1]], color: [9, 8, 7, 255]}
  OTHER: {asset: plain}
scenes:
  - id: shot
    duration: 2
    events: []
    composition:
      joint_tracks:
        PUPPET:
          arm: [{at: 0}, {at: 2, rotation: 1.5707963267948966, x: 4}]
";
    fn hand(script: &Script, time: f64) -> Vec<Vec2> {
        evaluate(script, 0, time).unwrap().actors[1].rigid_parts[1]
            .points
            .clone()
    }
    fn near(actual: &[Vec2], expected: [[f64; 2]; 3]) {
        for (a, e) in actual.iter().zip(expected) {
            assert!(
                (a.x - e[0]).abs() < 1e-9 && (a.y - e[1]).abs() < 1e-9,
                "{actual:?}"
            );
        }
    }

    #[test]
    fn custom_rig_joint_tracks_pose_child_parts_deterministically_and_leave_others_unchanged() {
        let script = motion_script::parse(ARM_RIG).unwrap();
        // Actors evaluate in character-id order: OTHER (no rig), then PUPPET.
        assert!(evaluate(&script, 0, 1.0).unwrap().actors[0]
            .rigid_parts
            .is_empty());
        near(&hand(&script, 0.0), [[10.0, 0.0], [11.0, 0.0], [10.0, 1.0]]);
        // Halfway: rotation pi/4 about the joint (existing affine convention) and +2 in x.
        let (sin, cos) = std::f64::consts::FRAC_PI_4.sin_cos();
        near(
            &hand(&script, 1.0),
            [[12.0, 0.0], [12.0 + cos, sin], [12.0 - sin, cos]],
        );
        // The parent's part never moves.
        let base = evaluate(&script, 0, 1.0).unwrap().actors[1].rigid_parts[0]
            .points
            .clone();
        near(&base, [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]);
        near(&hand(&script, 2.0), [[14.0, 0.0], [14.0, 1.0], [13.0, 0.0]]);
        // Repeated and out-of-order seeks, and the prepared path, agree exactly.
        let validated = ValidatedScript::new(std::sync::Arc::new(script.clone())).unwrap();
        for time in [1.5, 0.25, 1.5, 2.0, 0.0] {
            let direct = evaluate(&script, 0, time).unwrap();
            assert_eq!(direct, validated.evaluate(0, time).unwrap());
            assert_eq!(direct, evaluate(&script, 0, time).unwrap());
        }
        // Removing the track restores the rest pose.
        let mut rest = script.clone();
        rest.scenes[0].composition.joint_tracks.clear();
        near(&hand(&rest, 1.0), [[10.0, 0.0], [11.0, 0.0], [10.0, 1.0]]);
    }

    #[test]
    fn invalid_joint_tracks_fail_validation_with_actionable_errors() {
        for (from, to, message) in [
            ("arm: [", "wing: [", "has no joint wing"),
            (
                "PUPPET:\n          arm",
                "OTHER:\n          arm",
                "has no rig",
            ),
            (
                "{at: 2, rotation",
                "{at: 3, rotation",
                "joint tracks require",
            ),
            (
                "rotation: 1.5707963267948966, x: 4",
                "scale_x: 0",
                "joint tracks require",
            ),
            ("{at: 0}, ", "{at: 0}, {at: 0}, ", "joint tracks require"),
        ] {
            let source = ARM_RIG.replace(from, to);
            assert_ne!(source, ARM_RIG, "{from}");
            // Parsing validates the screenplay, so the actionable error surfaces there.
            let error = motion_script::parse(&source).unwrap_err().to_string();
            assert!(error.contains(message), "{to}: {error}");
        }
    }

    #[test]
    fn retargeted_rig_clip_poses_a_differently_named_rig_like_the_source() {
        let source = "characters:
  A:
    asset: custom
    rig:
      version: 1
      id: a
      joints: [{id: root, parent: null}, {id: arm, parent: root, x: 10}]
      parts: [{id: h, joint: arm, points: [[0, 0], [1, 0], [0, 1]], color: [1, 2, 3, 255]}]
  B:
    asset: custom
    rig:
      version: 1
      id: b
      joints: [{id: hip, parent: null}, {id: wing, parent: hip, x: 10}]
      parts: [{id: h, joint: wing, points: [[0, 0], [1, 0], [0, 1]], color: [1, 2, 3, 255]}]
scenes:
  - {id: shot, duration: 2, events: []}
";
        let keys = [
            motion_script::ActorKey::default(),
            motion_script::ActorKey {
                at: 2.0,
                x: 6.0,
                rotation: 1.0,
                ..Default::default()
            },
        ];
        let authored = motion_script::apply_joint_track(source, 0, "A", "arm", &keys).unwrap();
        let captured = motion_script::capture_rig_clip(&authored, 0, "A", "swing", false).unwrap();
        let placement = motion_script::RigClipPlacement {
            joint_map: [("arm".to_string(), "wing".to_string())].into(),
            ..Default::default()
        };
        let applied =
            motion_script::apply_rig_clip(&captured, 0, "B", "swing", &placement).unwrap();
        let script = motion_script::parse(&applied).unwrap();
        for time in [0.0, 0.5, 1.3, 2.0] {
            let scene = evaluate(&script, 0, time).unwrap();
            let part = |id: &str| {
                scene
                    .actors
                    .iter()
                    .find(|a| a.id == id)
                    .unwrap()
                    .rigid_parts[0]
                    .points
                    .clone()
            };
            assert_eq!(part("A"), part("B"), "t={time}");
        }
    }

    #[test]
    fn rules_expand_identically_for_seek_and_validated_script() {
        let source = "animation_clips: {w: {duration: 2, bone_tracks: {head: [{at: 0, x: 40}, {at: 2, x: 40}]}}}\ncharacters: {A: {asset: a}}\nscenes: [{id: s, duration: 6, events: [{type: emotion, at: 1, actor: A, value: happy}]}]\nrules:\n  r: {on: emotion, value: happy, then: {type: clip, clip: w, duration: 2}}\n";
        let script = motion_script::parse(source).unwrap();
        let validated = ValidatedScript::new(std::sync::Arc::new(script.clone())).unwrap();
        let before = format!("{:?}", evaluate(&script, 0, 0.5).unwrap());
        let during = format!("{:?}", evaluate(&script, 0, 1.5).unwrap());
        assert_ne!(before, during, "rule clip changes the pose");
        for time in [1.5, 0.5, 1.5, 4.0, 1.5] {
            assert_eq!(
                format!("{:?}", validated.evaluate(0, time).unwrap()),
                format!("{:?}", evaluate(&script, 0, time).unwrap())
            );
        }
        assert_eq!(format!("{:?}", validated.evaluate(0, 1.5).unwrap()), during);
    }
}
