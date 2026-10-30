//! Camera templates expand to ordinary, editable scene-local keys.
use crate::{edit_source, missing_event, CameraKey, Error};

/// Bounded full-scene movements, using the existing linear camera evaluator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraPreset {
    PushIn,
    PullOut,
    PanLeft,
    PanRight,
}

/// Generate neutral-centered camera keys; no hidden runtime preset state is saved.
/// Pans move the camera center by 100 scene pixels; zoom ranges from 1 to 1.25.
pub fn camera_preset_keys(preset: CameraPreset, duration: f64) -> Result<Vec<CameraKey>, Error> {
    if !duration.is_finite() || duration <= 0.0 {
        return Err(Error::Validation(
            "camera preset requires a finite positive scene duration".into(),
        ));
    }
    let mut start = CameraKey {
        at: 0.0,
        x: 0.0,
        y: 0.0,
        zoom: 1.0,
        angle: None,
    };
    let mut end = CameraKey {
        at: duration,
        ..start.clone()
    };
    match preset {
        CameraPreset::PushIn => end.zoom = 1.25,
        CameraPreset::PullOut => start.zoom = 1.25,
        CameraPreset::PanLeft => end.x = -100.0,
        CameraPreset::PanRight => end.x = 100.0,
    }
    Ok(vec![start, end])
}

/// Atomically replace the camera track only. Existing keys require explicit consent.
/// Like other source edits, YAML formatting is normalized and comments removed.
pub fn apply_camera_preset(
    source: &str,
    scene_index: usize,
    preset: CameraPreset,
    replace_existing: bool,
) -> Result<String, Error> {
    edit_source(source, |candidate| {
        let scene = candidate
            .scenes
            .get_mut(scene_index)
            .ok_or_else(missing_event)?;
        if !scene.composition.camera.is_empty() && !replace_existing {
            return Err(Error::Validation(
                "camera preset would replace existing keys; explicitly enable replacement".into(),
            ));
        }
        scene.composition.camera = camera_preset_keys(preset, scene.duration)?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: &str = "characters: {A: {asset: a}}\nscenes: [{id: shot, duration: 4, events: [{type: emotion, actor: A, at: 1, value: happy}]}]";

    #[test]
    fn presets_expand_to_roundtrippable_keys_and_preserve_content() {
        for preset in [
            CameraPreset::PushIn,
            CameraPreset::PullOut,
            CameraPreset::PanLeft,
            CameraPreset::PanRight,
        ] {
            let edited = apply_camera_preset(SOURCE, 0, preset, false).unwrap();
            let script = crate::parse(&edited).unwrap();
            assert_eq!(script.scenes[0].events.len(), 1);
            assert_eq!(script.scenes[0].id, "shot");
            assert_eq!(
                script.scenes[0].composition.camera,
                camera_preset_keys(preset, 4.0).unwrap()
            );
            assert!(apply_camera_preset(&edited, 0, preset, false).is_err());
            assert_eq!(
                apply_camera_preset(&edited, 0, preset, true).unwrap(),
                edited
            );
        }
        assert!(apply_camera_preset(SOURCE, 1, CameraPreset::PushIn, true).is_err());
    }

    #[test]
    fn replacement_preserves_other_composition_fields() {
        let mut script = crate::parse(SOURCE).unwrap();
        script.scenes[0].composition.background_color = "#123456".into();
        script.scenes[0].composition.fade_in = 0.5;
        let source = crate::to_source(&script).unwrap();
        let first = apply_camera_preset(&source, 0, CameraPreset::PushIn, false).unwrap();
        let replaced = apply_camera_preset(&first, 0, CameraPreset::PanLeft, true).unwrap();
        let result = crate::parse(&replaced).unwrap();
        assert_eq!(result.scenes[0].composition.background_color, "#123456");
        assert_eq!(result.scenes[0].composition.fade_in, 0.5);
        assert_eq!(result.scenes[0].events.len(), 1);
        assert_eq!(result.scenes[0].composition.camera[1].x, -100.0);
        assert!(apply_camera_preset("broken: [", 0, CameraPreset::PushIn, true).is_err());
    }

    #[test]
    fn invalid_times_never_create_a_track() {
        for duration in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(camera_preset_keys(CameraPreset::PushIn, duration).is_err());
        }
        let push = camera_preset_keys(CameraPreset::PushIn, 4.0).unwrap();
        assert_eq!((push[0].zoom, push[1].zoom), (1.0, 1.25));
        let pull = camera_preset_keys(CameraPreset::PullOut, 4.0).unwrap();
        assert_eq!((pull[0].zoom, pull[1].zoom), (1.25, 1.0));
        assert_eq!(
            camera_preset_keys(CameraPreset::PanLeft, 4.0).unwrap()[1].x,
            -100.0
        );
        assert_eq!(
            camera_preset_keys(CameraPreset::PanRight, 4.0).unwrap()[1].x,
            100.0
        );
    }
}
