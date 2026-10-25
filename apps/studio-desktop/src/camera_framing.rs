//! Camera presets are ordinary, editable scene keys sampled by the shared evaluator.
use crate::project_history::MAX_SOURCE_BYTES;
use motion_scene::{ActorPose, Vec2};
use motion_script::{CameraKey, Script};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shot {
    Wide,
    TwoActors,
    Medium,
    CloseUp,
    PropClose,
}

impl Shot {
    pub(crate) const ALL: [Self; 5] = [
        Self::Wide,
        Self::TwoActors,
        Self::Medium,
        Self::CloseUp,
        Self::PropClose,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Wide => "Yleiskuva",
            Self::TwoActors => "Hahmot samaan kuvaan",
            Self::Medium => "Puolikuva",
            Self::CloseUp => "Lähikuva",
            Self::PropClose => "Kädessä oleva esine",
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Bounds {
    min: Vec2,
    max: Vec2,
}

impl Bounds {
    fn points(points: impl IntoIterator<Item = Vec2>) -> Option<Self> {
        let mut points = points.into_iter();
        let first = points.next()?;
        let mut bounds = Self {
            min: first,
            max: first,
        };
        for point in points {
            bounds.min.x = bounds.min.x.min(point.x);
            bounds.min.y = bounds.min.y.min(point.y);
            bounds.max.x = bounds.max.x.max(point.x);
            bounds.max.y = bounds.max.y.max(point.y);
        }
        (bounds.min.x.is_finite()
            && bounds.min.y.is_finite()
            && bounds.max.x.is_finite()
            && bounds.max.y.is_finite())
        .then_some(bounds)
    }

    fn corners(self) -> [Vec2; 4] {
        [
            self.min,
            Vec2::new(self.max.x, self.min.y),
            self.max,
            Vec2::new(self.min.x, self.max.y),
        ]
    }

    fn expand(self, margin: f64) -> Self {
        Self {
            min: Vec2::new(self.min.x - margin, self.min.y - margin),
            max: Vec2::new(self.max.x + margin, self.max.y + margin),
        }
    }
}

fn world_bounds(actor: &ActorPose, points: impl IntoIterator<Item = Vec2>) -> Option<Bounds> {
    Bounds::points(points.into_iter().map(|point| actor.local_to_world(point)))
}

fn all_points(actor: &ActorPose) -> Vec<Vec2> {
    if actor.rigid_parts.is_empty() {
        let mut points = actor.skeleton.to_vec();
        let head = actor.skeleton[motion_scene::HEAD];
        points.extend(
            Bounds {
                min: Vec2::new(head.x - 30.0, head.y - 38.0),
                max: Vec2::new(head.x + 30.0, head.y + 38.0),
            }
            .corners(),
        );
        points
    } else {
        actor
            .rigid_parts
            .iter()
            .filter(|part| part.color[3] != 0)
            .flat_map(|part| part.points.iter().copied())
            .collect()
    }
}

/// Use actual posed custom head geometry rather than the procedural skeleton,
/// which is a separate representation when a character supplies a custom rig.
fn head_points(script: &Script, actor: &ActorPose) -> Result<Vec<Vec2>, String> {
    let Some(rig) = script.characters[&actor.id].rig.as_ref() else {
        let center = actor.skeleton[motion_scene::HEAD];
        return Ok(Bounds {
            min: Vec2::new(center.x - 30.0, center.y - 38.0),
            max: Vec2::new(center.x + 30.0, center.y + 38.0),
        }
        .corners()
        .to_vec());
    };
    let Some(head) = rig
        .joints
        .iter()
        .find(|joint| joint.id.eq_ignore_ascii_case("head"))
    else {
        return Err("Lähikuva tarvitsee hahmolta head-nivelen.".into());
    };
    let is_head = |joint: &str| {
        let mut current = Some(joint);
        // The parser has already rejected cycles; the bound also makes this
        // ancestry walk explicit even if the rig format changes later.
        for _ in 0..rig.joints.len() {
            let Some(id) = current else { break };
            if id == head.id {
                return true;
            }
            current = rig
                .joints
                .iter()
                .find(|node| node.id == id)
                .and_then(|node| node.parent.as_deref());
        }
        false
    };
    let points: Vec<_> = actor
        .rigid_parts
        .iter()
        .filter(|part| {
            part.color[3] != 0
                && rig
                    .parts
                    .iter()
                    .find(|definition| definition.id == part.id)
                    .is_some_and(|definition| is_head(&definition.joint))
        })
        .flat_map(|part| part.points.iter().copied())
        .collect();
    if points.is_empty() {
        return Err("Head-niveleen ei ole kiinnitetty näkyvää pään kuvitusta.".into());
    }
    Ok(points)
}

fn selected_bounds(script: &Script, actor: &ActorPose, shot: Shot) -> Result<Bounds, String> {
    let points = match shot {
        Shot::PropClose => {
            let points: Vec<_> = actor
                .rigid_parts
                .iter()
                .filter(|part| {
                    part.color[3] != 0
                        && (part.id.starts_with("heldprop_l_")
                            || part.id.starts_with("heldprop_r_"))
                })
                .flat_map(|part| part.points.iter().copied())
                .collect();
            if points.is_empty() {
                return Err("Kiinnitä hahmolle ensin esine työkalulla Esine käteen.".into());
            }
            // Some space around a small object keeps the gripping hand readable.
            Bounds::points(points)
                .ok_or("Kädessä olevan esineen rajaus ei kelpaa.")?
                .expand(18.0)
                .corners()
                .to_vec()
        }
        Shot::CloseUp => {
            let points = head_points(script, actor)?;
            Bounds::points(points)
                .ok_or("Pään rajaus ei kelpaa.")?
                .expand(22.0)
                .corners()
                .to_vec()
        }
        Shot::Medium => {
            let head =
                Bounds::points(head_points(script, actor)?).ok_or("Pään rajaus ei kelpaa.")?;
            let span = (head.max.y - head.min.y).max(30.0);
            let lower = head.max.y + span * 2.4;
            let mut points = head.expand(10.0).corners().to_vec();
            points.extend(
                all_points(actor)
                    .into_iter()
                    .filter(|point| point.y >= head.min.y - 10.0 && point.y <= lower),
            );
            // Ensure a stable amount of torso context even in a raised-arm pose.
            points.push(Vec2::new((head.min.x + head.max.x) * 0.5, lower));
            points
        }
        Shot::Wide | Shot::TwoActors => all_points(actor),
    };
    world_bounds(actor, points).ok_or_else(|| "Hahmon rajaus ei kelpaa.".into())
}

pub(crate) fn apply(
    source: &str,
    scene_index: usize,
    actor: Option<&str>,
    at: f64,
    shot: Shot,
) -> Result<String, String> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err("Käsikirjoitus ylittää muokkaimen 256 KiB:n kokorajan.".into());
    }
    let script = motion_script::parse(source).map_err(|error| error.to_string())?;
    let scene = script
        .scenes
        .get(scene_index)
        .ok_or("Valitse olemassa oleva kohtaus.")?;
    if !at.is_finite() || at < 0.0 || at > scene.duration {
        return Err("Kuvakoon aika pitää sijoittaa kohtauksen sisälle.".into());
    }
    let evaluated =
        motion_scene::evaluate(&script, scene_index, at).map_err(|error| error.to_string())?;
    let center = Vec2::new(
        f64::from(evaluated.width) * 0.5,
        f64::from(evaluated.height) * 0.5,
    );
    let key = if shot == Shot::Wide {
        CameraKey {
            at,
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        }
    } else {
        let bounds = if shot == Shot::TwoActors {
            let points: Vec<_> = evaluated
                .actors
                .iter()
                .flat_map(|actor| {
                    all_points(actor)
                        .into_iter()
                        .map(|point| actor.local_to_world(point))
                })
                .collect();
            Bounds::points(points).ok_or("Kohtauksessa ei ole näkyviä hahmoja.")?
        } else {
            let selected = actor.ok_or("Valitse ensin rajattava hahmo.")?;
            let selected = evaluated
                .actors
                .iter()
                .find(|pose| pose.id == selected)
                .ok_or("Valitun hahmon pitää näkyä tässä kohtauksessa.")?;
            selected_bounds(&script, selected, shot)?
        };
        // Fitting world geometry through the actual root transform avoids both
        // camera double application and incorrect bounds for rotated/scaled rigs.
        let width = (bounds.max.x - bounds.min.x).max(24.0);
        let height = (bounds.max.y - bounds.min.y).max(24.0);
        CameraKey {
            at,
            x: (bounds.min.x + bounds.max.x) * 0.5 - center.x,
            y: (bounds.min.y + bounds.max.y) * 0.5 - center.y,
            zoom: (f64::from(evaluated.width) * 0.84 / width)
                .min(f64::from(evaluated.height) * 0.84 / height)
                .clamp(0.05, 20.0),
        }
    };
    let mut composition = scene.composition.clone();
    match composition
        .camera
        .binary_search_by(|key| key.at.total_cmp(&at))
    {
        Ok(index) => composition.camera[index] = key,
        Err(index) => {
            if composition.camera.len() >= 4096 {
                return Err("Kameralla on jo enimmäismäärä 4096 avainruutua.".into());
            }
            // A first key after zero would otherwise also change the earlier
            // part of a scene that previously had no authored camera track.
            if composition.camera.is_empty() && at > 0.0 {
                composition.camera.push(CameraKey {
                    at: 0.0,
                    x: 0.0,
                    y: 0.0,
                    zoom: 1.0,
                });
                composition.camera.push(key);
            } else {
                composition.camera.insert(index, key);
            }
        }
    }
    let updated = motion_script::apply_composition(source, scene_index, composition)
        .map_err(|error| error.to_string())?;
    if updated.len() > MAX_SOURCE_BYTES {
        return Err("Kuvakoon lisääminen ylittäisi muokkaimen 256 KiB:n kokorajan.".into());
    }
    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> String {
        crate::stick_studio::load().unwrap().source
    }

    fn camera(source: &str, scene: usize, at: f64) -> motion_scene::CameraPose {
        motion_scene::evaluate(&motion_script::parse(source).unwrap(), scene, at)
            .unwrap()
            .camera
    }

    #[test]
    fn actual_custom_head_is_centered_and_root_transform_is_respected() {
        let source = source();
        let mut script = motion_script::parse(&source).unwrap();
        let actor = &mut script.scenes[0]
            .composition
            .actor_tracks
            .get_mut("VEIKKO")
            .unwrap()[0];
        actor.rotation = 0.7;
        actor.scale_x = 2.3;
        actor.scale_y = 0.8;
        let source = motion_script::to_source(&script).unwrap();
        let changed = apply(&source, 0, Some("VEIKKO"), 0.0, Shot::CloseUp).unwrap();
        let evaluated = motion_scene::evaluate(&script, 0, 0.0).unwrap();
        let actor = evaluated
            .actors
            .iter()
            .find(|actor| actor.id == "VEIKKO")
            .unwrap();
        let camera = camera(&changed, 0, 0.0);
        // Inspect the actual face independently of the framing helper. The
        // procedural fallback head is at a different height for these rigs.
        let face = actor
            .rigid_parts
            .iter()
            .find(|part| part.id == "face")
            .unwrap();
        let mut center = Vec2::new(0.0, 0.0);
        for point in &face.points {
            let world = actor.local_to_world(*point);
            center.x += world.x / face.points.len() as f64;
            center.y += world.y / face.points.len() as f64;
            let point = camera.project(world.x, world.y, evaluated.width, evaluated.height);
            assert!(point.0 >= 0.0 && point.0 <= f64::from(evaluated.width));
            assert!(point.1 >= 0.0 && point.1 <= f64::from(evaluated.height));
        }
        let projected = camera.project(center.x, center.y, evaluated.width, evaluated.height);
        assert!(
            (projected.0 - f64::from(evaluated.width) * 0.5).abs()
                < f64::from(evaluated.width) * 0.1
        );
        assert!(
            (projected.1 - f64::from(evaluated.height) * 0.5).abs()
                < f64::from(evaluated.height) * 0.15
        );
    }

    #[test]
    fn presets_are_distinct_and_keep_other_keys_and_composition() {
        let source = source();
        let script = motion_script::parse(&source).unwrap();
        let original = &script.scenes[0].composition;
        let mut zooms = Vec::new();
        for shot in [Shot::TwoActors, Shot::Medium, Shot::CloseUp] {
            let updated = apply(&source, 0, Some("VEIKKO"), 2.0, shot).unwrap();
            let parsed = motion_script::parse(&updated).unwrap();
            let composition = &parsed.scenes[0].composition;
            assert_eq!(composition.actor_tracks, original.actor_tracks);
            assert_eq!(composition.joint_tracks, original.joint_tracks);
            assert_eq!(composition.layers, original.layers);
            assert!(original
                .camera
                .iter()
                .all(|key| composition.camera.contains(key)));
            zooms.push(camera(&updated, 0, 2.0).zoom);
        }
        assert!(zooms[0] < zooms[1] && zooms[1] < zooms[2], "{zooms:?}");
        let wide = apply(&source, 0, None, 0.0, Shot::Wide).unwrap();
        assert_eq!(camera(&wide, 0, 0.0), motion_scene::CameraPose::default());
    }

    #[test]
    fn phone_is_framed_in_its_actual_posed_hand_without_using_old_camera() {
        let source = source();
        let script = motion_script::parse(&source).unwrap();
        let scene = script
            .scenes
            .iter()
            .position(|scene| scene.id == "pysakkipaniikki")
            .unwrap();
        let changed = apply(&source, scene, Some("VEIKKO"), 1.5, Shot::PropClose).unwrap();
        let evaluated = motion_scene::evaluate(&script, scene, 1.5).unwrap();
        let actor = evaluated
            .actors
            .iter()
            .find(|actor| actor.id == "VEIKKO")
            .unwrap();
        let camera = camera(&changed, scene, 1.5);
        let mut count = 0;
        for part in actor
            .rigid_parts
            .iter()
            .filter(|part| part.id.starts_with("heldprop_r_puhelin_"))
        {
            count += 1;
            for point in &part.points {
                let world = actor.local_to_world(*point);
                let screen = camera.project(world.x, world.y, evaluated.width, evaluated.height);
                assert!(screen.0 >= 0.0 && screen.0 <= f64::from(evaluated.width));
                assert!(screen.1 >= 0.0 && screen.1 <= f64::from(evaluated.height));
            }
        }
        assert!(count > 0, "test must exercise the actual held phone");
    }

    #[test]
    fn invalid_selection_time_and_source_are_atomic_errors() {
        let source = source();
        for time in [f64::NAN, f64::INFINITY, -0.1, 50.0] {
            assert!(apply(&source, 0, Some("VEIKKO"), time, Shot::Medium).is_err());
        }
        assert!(apply(&source, usize::MAX, None, 0.0, Shot::Wide).is_err());
        assert!(apply(&source, 0, None, 0.0, Shot::CloseUp).is_err());
        assert!(apply(&source, 0, Some("MISSING"), 0.0, Shot::CloseUp).is_err());
        assert!(apply(&source, 0, Some("SISU"), 0.0, Shot::CloseUp).is_err());
        assert!(apply(&source, 0, Some("NOUTAJA"), 0.0, Shot::PropClose)
            .unwrap_err()
            .contains("Esine käteen"));
        assert!(apply(&"[".repeat(MAX_SOURCE_BYTES + 1), 0, None, 0.0, Shot::Wide).is_err());
    }

    #[test]
    fn first_camera_key_keeps_the_original_view_at_scene_start() {
        let source = "characters: {A: {asset: characters/a}}\nscenes: [{id: scene, duration: 5, events: []}]";
        let changed = apply(source, 0, Some("A"), 2.0, Shot::CloseUp).unwrap();
        assert_eq!(
            camera(&changed, 0, 0.0),
            motion_scene::CameraPose::default()
        );
        assert!(camera(&changed, 0, 2.0).zoom > 1.0);
        assert_eq!(
            motion_script::parse(&changed).unwrap().scenes[0]
                .composition
                .camera
                .len(),
            2
        );
    }

    #[test]
    fn full_camera_track_rejects_a_new_key_with_a_specific_error() {
        let mut script = motion_script::parse(
            "characters: {}\nscenes: [{id: scene, duration: 4096, events: []}]",
        )
        .unwrap();
        script.scenes[0].composition.camera = (0..4096)
            .map(|index| CameraKey {
                at: f64::from(index),
                x: 0.0,
                y: 0.0,
                zoom: 1.0,
            })
            .collect();
        let source = serde_json::to_string(&script).unwrap();
        assert!(source.len() <= MAX_SOURCE_BYTES);
        assert!(apply(&source, 0, None, 0.5, Shot::Wide)
            .unwrap_err()
            .contains("4096"));
        assert_eq!(
            motion_script::parse(&source).unwrap().scenes[0]
                .composition
                .camera
                .len(),
            4096
        );
    }
}
