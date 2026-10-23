//! Validated, source-preserving editor transactions; pointer previews are ephemeral.
use eframe::egui;
use motion_script::{ActorKey, Script};

pub fn canvas_to_world(
    rect: egui::Rect,
    scene: &motion_scene::EvaluatedScene,
    pointer: egui::Pos2,
) -> Option<[f64; 2]> {
    let scale =
        (rect.width() / scene.width.max(1) as f32).min(rect.height() / scene.height.max(1) as f32);
    if !scale.is_finite()
        || scale <= 0.0
        || !scene.camera.zoom.is_finite()
        || scene.camera.zoom <= 0.0
        || !pointer.x.is_finite()
        || !pointer.y.is_finite()
    {
        return None;
    }
    let canvas = egui::Rect::from_center_size(
        rect.center(),
        egui::vec2(scene.width as f32, scene.height as f32) * scale,
    );
    if !canvas.contains(pointer) {
        return None;
    }
    let p = (pointer - canvas.min) / scale;
    let cx = f64::from(scene.width) / 2.0;
    let cy = f64::from(scene.height) / 2.0;
    Some([
        (f64::from(p.x) - cx) / scene.camera.zoom + cx + scene.camera.x,
        (f64::from(p.y) - cy) / scene.camera.zoom + cy + scene.camera.y,
    ])
}
fn screen_point(
    rect: egui::Rect,
    scene: &motion_scene::EvaluatedScene,
    x: f64,
    y: f64,
) -> egui::Pos2 {
    let scale =
        (rect.width() / scene.width.max(1) as f32).min(rect.height() / scene.height.max(1) as f32);
    let origin = rect.center() - egui::vec2(scene.width as f32, scene.height as f32) * scale / 2.0;
    let p = scene.camera.project(x, y, scene.width, scene.height);
    origin + egui::vec2(p.0 as f32, p.1 as f32) * scale
}
pub fn actor_bounds(
    rect: egui::Rect,
    scene: &motion_scene::EvaluatedScene,
    actor: &motion_scene::ActorPose,
) -> egui::Rect {
    let mut points: Vec<_> = actor.skeleton.to_vec();
    points.extend(
        actor
            .rigid_parts
            .iter()
            .flat_map(|part| part.points.iter().copied()),
    );
    let head = actor.skeleton[motion_scene::HEAD];
    points.extend([
        motion_scene::Vec2::new(head.x - 30.0, head.y - 38.0),
        motion_scene::Vec2::new(head.x + 30.0, head.y - 38.0),
        motion_scene::Vec2::new(head.x - 30.0, head.y + 38.0),
        motion_scene::Vec2::new(head.x + 30.0, head.y + 38.0),
    ]);
    points
        .into_iter()
        .fold(egui::Rect::NOTHING, |bounds, point| {
            let world = actor.local_to_world(point);
            let p = screen_point(rect, scene, world.x, world.y);
            bounds.union(egui::Rect::from_min_max(p, p))
        })
        .expand(7.0)
}
pub fn pick(
    rect: egui::Rect,
    scene: &motion_scene::EvaluatedScene,
    pointer: egui::Pos2,
) -> Option<String> {
    canvas_to_world(rect, scene, pointer)?;
    scene
        .actors
        .iter()
        .rev()
        .find(|actor| actor_bounds(rect, scene, actor).contains(pointer))
        .map(|actor| actor.id.clone())
}
pub fn snapped_time(script: &Script, scene_index: usize, time: f64) -> Result<f64, String> {
    let scene = script
        .scenes
        .get(scene_index)
        .ok_or("Select an existing scene")?;
    if !time.is_finite() || time < 0.0 || time > scene.duration || script.project.fps == 0 {
        return Err("Invalid playhead/frame rate".into());
    }
    Ok(
        ((time * f64::from(script.project.fps)).round() / f64::from(script.project.fps))
            .min(scene.duration),
    )
}
fn sampled_key(keys: &[ActorKey], at: f64) -> Result<ActorKey, String> {
    if keys.is_empty() {
        return Ok(ActorKey {
            at,
            ..Default::default()
        });
    }
    let sample = motion_animation::Track::new(keys.iter().map(ActorKey::animation_key).collect())
        .map_err(|e| e.to_string())?
        .sample(at)
        .map_err(|e| e.to_string())?;
    Ok(ActorKey {
        at,
        x: sample.translation.x,
        y: sample.translation.y,
        rotation: sample.rotation,
        scale_x: sample.scale.x,
        scale_y: sample.scale.y,
        easing: keys
            .iter()
            .rev()
            .find(|key| key.at <= at)
            .unwrap_or(&keys[0])
            .easing,
    })
}
fn upsert(keys: &mut Vec<ActorKey>, key: ActorKey) {
    if keys.is_empty() && key.at > 0.0 {
        keys.push(ActorKey::default());
    }
    match keys.binary_search_by(|existing| existing.at.total_cmp(&key.at)) {
        Ok(index) => keys[index] = key,
        Err(index) => keys.insert(index, key),
    }
}
pub struct Drag {
    pub source: String,
    pub scene_index: usize,
    pub actor: String,
    pub time: f64,
    pub rect: egui::Rect,
    pub camera: motion_scene::CameraPose,
    start: [f64; 2],
    key: ActorKey,
    keys: Vec<ActorKey>,
}
impl Drag {
    pub fn new(
        source: &str,
        scene_index: usize,
        actor: &str,
        time: f64,
        rect: egui::Rect,
        scene: &motion_scene::EvaluatedScene,
        pointer: egui::Pos2,
    ) -> Result<Self, String> {
        let script = motion_script::parse(source).map_err(|e| e.to_string())?;
        let at = snapped_time(&script, scene_index, time)?;
        if !scene.actors.iter().any(|pose| pose.id == actor) {
            return Err("Select a visible character".into());
        }
        let start = canvas_to_world(rect, scene, pointer).ok_or("Start inside the stage")?;
        let keys = script.scenes[scene_index]
            .composition
            .actor_tracks
            .get(actor)
            .cloned()
            .unwrap_or_default();
        Ok(Self {
            source: source.into(),
            scene_index,
            actor: actor.into(),
            time,
            rect,
            camera: scene.camera,
            start,
            key: sampled_key(&keys, at)?,
            keys,
        })
    }
    pub fn delta(
        &self,
        scene: &motion_scene::EvaluatedScene,
        pointer: egui::Pos2,
    ) -> Option<[f64; 2]> {
        let current = canvas_to_world(self.rect, scene, pointer)?;
        Some([current[0] - self.start[0], current[1] - self.start[1]])
    }
    pub fn apply(
        &self,
        scene: &motion_scene::EvaluatedScene,
        pointer: egui::Pos2,
    ) -> Result<String, String> {
        if scene.camera != self.camera {
            return Err("Camera changed; move cancelled".into());
        }
        let delta = self
            .delta(scene, pointer)
            .ok_or("Drop inside the stage; move cancelled")?;
        if delta[0].abs() < 0.001 && delta[1].abs() < 0.001 {
            return Ok(self.source.clone());
        }
        let mut key = self.key.clone();
        key.x += delta[0];
        key.y += delta[1];
        let mut keys = self.keys.clone();
        upsert(&mut keys, key);
        motion_script::apply_actor_track(&self.source, self.scene_index, &self.actor, &keys)
            .map_err(|e| e.to_string())
    }
}
pub struct HeadDraft {
    pub scene_index: usize,
    pub actor: String,
    pub at: f64,
    pub x: f64,
    pub y: f64,
    pub tilt_degrees: f64,
    pub supports_tilt: bool,
    target: String,
    key: ActorKey,
    keys: Vec<ActorKey>,
}
impl HeadDraft {
    pub fn new(
        script: &Script,
        scene_index: usize,
        actor: &str,
        time: f64,
    ) -> Result<Self, String> {
        let at = snapped_time(script, scene_index, time)?;
        let character = script.characters.get(actor).ok_or("Select a character")?;
        let scene = &script.scenes[scene_index];
        let (target, supports_tilt, keys) = if let Some(rig) = &character.rig {
            let head = rig
                .joints
                .iter()
                .find(|joint| joint.id.eq_ignore_ascii_case("head"))
                .ok_or("This custom rig needs a joint named head for these controls")?;
            (
                head.id.clone(),
                true,
                scene
                    .composition
                    .joint_tracks
                    .get(actor)
                    .and_then(|tracks| tracks.get(&head.id))
                    .cloned()
                    .unwrap_or_default(),
            )
        } else {
            (
                "head".into(),
                false,
                scene
                    .composition
                    .bone_tracks
                    .get(actor)
                    .and_then(|tracks| tracks.get("head"))
                    .cloned()
                    .unwrap_or_default(),
            )
        };
        let key = sampled_key(&keys, at)?;
        Ok(Self {
            scene_index,
            actor: actor.into(),
            at,
            x: key.x,
            y: key.y,
            tilt_degrees: key.rotation.to_degrees(),
            supports_tilt,
            target,
            key,
            keys,
        })
    }
    pub fn apply(&self, source: &str) -> Result<String, String> {
        if !self.x.is_finite() || !self.y.is_finite() || !self.tilt_degrees.is_finite() {
            return Err("Head controls must be finite".into());
        }
        let mut key = self.key.clone();
        key.x = self.x;
        key.y = self.y;
        if self.supports_tilt {
            key.rotation = self.tilt_degrees.to_radians();
        }
        let mut keys = self.keys.clone();
        upsert(&mut keys, key);
        if self.supports_tilt {
            motion_script::apply_joint_track(
                source,
                self.scene_index,
                &self.actor,
                &self.target,
                &keys,
            )
        } else {
            motion_script::apply_bone_track(
                source,
                self.scene_index,
                &self.actor,
                &self.target,
                &keys,
            )
        }
        .map_err(|e| e.to_string())
    }
}
pub fn blank_scene(source: &str) -> Result<(String, usize), String> {
    let script = motion_script::parse(source).map_err(|e| e.to_string())?;
    let index = script.scenes.len();
    let mut number = index + 1;
    while script
        .scenes
        .iter()
        .any(|scene| scene.id == format!("scene_{number}"))
    {
        number += 1;
    }
    let mut scene = motion_script::Scene {
        id: format!("scene_{number}"),
        duration: 10.0,
        background: Some("studio".into()),
        composition: Default::default(),
        events: vec![],
    };
    let count = script.characters.len();
    for (i, actor) in script.characters.keys().enumerate() {
        scene.composition.actor_tracks.insert(
            actor.clone(),
            vec![ActorKey {
                x: f64::from(script.project.width) * ((i + 1) as f64 / (count + 1) as f64 - 0.5),
                ..Default::default()
            }],
        );
    }
    motion_script::add_scene(source, scene)
        .map(|source| (source, index))
        .map_err(|e| e.to_string())
}
pub fn new_episode(title: &str) -> Result<String, String> {
    if title.trim().is_empty() || title.len() > 200 {
        return Err("Episode title must be 1–200 bytes".into());
    }
    let mut script = motion_script::parse(include_str!("../../../examples/hello-emma.yaml"))
        .map_err(|e| e.to_string())?;
    script.project.title = title.trim().into();
    script.scenes[0].id = "starter".into();
    script.scenes[0].duration = 10.0;
    script.scenes[0].events.clear();
    let source = motion_script::to_source(&script).map_err(|e| e.to_string())?;
    let (source, _) = blank_scene(&source)?;
    let mut script = motion_script::parse(&source).map_err(|e| e.to_string())?;
    script.scenes.remove(0);
    motion_script::to_source(&script).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: &str = include_str!("../../../examples/hello-emma.yaml");
    fn fixture() -> (Script, motion_scene::EvaluatedScene, egui::Rect) {
        let script = motion_script::parse(SOURCE).unwrap();
        let scene = motion_scene::evaluate(&script, 0, 0.0).unwrap();
        (
            script,
            scene,
            egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(960.0, 640.0)),
        )
    }
    #[test]
    fn camera_letterboxing_and_topmost_pick() {
        let (_, mut scene, rect) = fixture();
        scene.camera = motion_scene::CameraPose {
            x: 80.0,
            y: -30.0,
            zoom: 1.5,
        };
        let p = screen_point(rect, &scene, 1000.0, 500.0);
        let world = canvas_to_world(rect, &scene, p).unwrap();
        assert!((world[0] - 1000.0).abs() < 0.001 && (world[1] - 500.0).abs() < 0.001);
        assert!(canvas_to_world(rect, &scene, rect.min).is_none());
        let mut top = scene.actors[0].clone();
        top.id = "top".into();
        scene.actors = vec![scene.actors[0].clone(), top];
        let bounds = actor_bounds(rect, &scene, &scene.actors[1]);
        assert_eq!(pick(rect, &scene, bounds.center()).as_deref(), Some("top"));
    }
    #[test]
    fn drag_samples_transform_and_keeps_other_keys() {
        let (mut script, _, rect) = fixture();
        let keys = vec![
            ActorKey {
                rotation: 0.5,
                scale_x: 1.2,
                scale_y: 0.8,
                ..Default::default()
            },
            ActorKey {
                at: 2.0,
                x: 100.0,
                rotation: 0.5,
                scale_x: 1.2,
                scale_y: 0.8,
                ..Default::default()
            },
        ];
        script.scenes[0]
            .composition
            .actor_tracks
            .insert("EMMA".into(), keys);
        let source = motion_script::to_source(&script).unwrap();
        let mut scene = motion_scene::evaluate(&script, 0, 1.0).unwrap();
        scene.camera.zoom = 2.0;
        let start = rect.center();
        let drag = Drag::new(&source, 0, "EMMA", 1.0, rect, &scene, start).unwrap();
        assert_eq!(drag.apply(&scene, start).unwrap(), source);
        assert!(drag.apply(&scene, rect.min - egui::vec2(1.0, 1.0)).is_err());
        let result =
            motion_script::parse(&drag.apply(&scene, start + egui::vec2(20.0, 10.0)).unwrap())
                .unwrap();
        let keys = &result.scenes[0].composition.actor_tracks["EMMA"];
        assert_eq!(keys.len(), 3);
        assert!((keys[1].x - 70.0).abs() < 0.001);
        assert!((keys[1].y - 10.0).abs() < 0.001);
        assert_eq!(keys[1].rotation, 0.5);
        assert_eq!(keys[1].scale_x, 1.2);
        assert_eq!(keys[1].scale_y, 0.8);
        assert_eq!(keys[2].x, 100.0);
    }
    #[test]
    fn custom_head_tilt_uses_named_joint_and_preserves_scale() {
        let source = "characters:\n  P:\n    asset: custom\n    rig:\n      version: 1\n      id: p\n      joints: [{id: root, parent: null}, {id: HeAd, parent: root, x: 5}]\n      parts: [{id: face, joint: HeAd, points: [[0, 0], [2, 0], [0, 2]], color: [1, 2, 3, 255]}]\nscenes:\n  - {id: s, duration: 2, events: [], composition: {joint_tracks: {P: {HeAd: [{at: 0, scale_x: 1.5}]}}}}\n";
        let script = motion_script::parse(source).unwrap();
        let mut draft = HeadDraft::new(&script, 0, "P", 1.0).unwrap();
        assert!(draft.supports_tilt);
        draft.tilt_degrees = 30.0;
        let edited = motion_script::parse(&draft.apply(source).unwrap()).unwrap();
        let key = &edited.scenes[0].composition.joint_tracks["P"]["HeAd"][1];
        assert!((key.rotation - std::f64::consts::PI / 6.0).abs() < 0.0001);
        assert_eq!(key.scale_x, 1.5);
        let missing = motion_script::parse(&source.replace("HeAd", "arm")).unwrap();
        assert!(HeadDraft::new(&missing, 0, "P", 1.0).is_err());
    }
    #[test]
    fn head_and_creation_validate() {
        let (script, _, _) = fixture();
        let mut draft = HeadDraft::new(&script, 0, "EMMA", 1.0).unwrap();
        assert!(!draft.supports_tilt);
        draft.x = 12.0;
        draft.y = -6.0;
        let edited = motion_script::parse(&draft.apply(SOURCE).unwrap()).unwrap();
        assert_eq!(
            edited.scenes[0].composition.bone_tracks["EMMA"]["head"][1].x,
            12.0
        );
        draft.x = f64::NAN;
        assert!(draft.apply(SOURCE).is_err());
        let (source, index) = blank_scene(SOURCE).unwrap();
        let script = motion_script::parse(&source).unwrap();
        assert_eq!(index, 1);
        assert_eq!(script.scenes[index].duration, 10.0);
        assert!(script.scenes[index].events.is_empty());
        let script = motion_script::parse(&new_episode("Episode two").unwrap()).unwrap();
        assert_eq!(script.project.title, "Episode two");
        assert_eq!(script.scenes.len(), 1);
        assert!(script.scenes[0].events.is_empty());
        assert!(new_episode(" ").is_err());
        assert!(snapped_time(&script, 0, f64::NAN).is_err());
    }
}
