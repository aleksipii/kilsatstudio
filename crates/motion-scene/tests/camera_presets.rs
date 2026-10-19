use motion_script::{apply_camera_preset, parse, CameraPreset};

#[test]
fn camera_templates_share_the_deterministic_scene_clock() {
    let source = "characters: {}\nscenes: [{id: shot, duration: 4, events: []}]";
    for (preset, x, zoom) in [
        (CameraPreset::PushIn, 0.0, 1.125),
        (CameraPreset::PullOut, 0.0, 1.125),
        (CameraPreset::PanLeft, -50.0, 1.0),
        (CameraPreset::PanRight, 50.0, 1.0),
    ] {
        let edited = apply_camera_preset(source, 0, preset, false).unwrap();
        let script = parse(&edited).unwrap();
        let middle = motion_scene::evaluate(&script, 0, 2.0).unwrap().camera;
        assert_eq!(middle.x, x);
        assert_eq!(middle.y, 0.0);
        assert_eq!(middle.zoom, zoom);
        let _ = motion_scene::evaluate(&script, 0, 4.0).unwrap();
        let _ = motion_scene::evaluate(&script, 0, 0.0).unwrap();
        assert_eq!(motion_scene::evaluate(&script, 0, 2.0).unwrap().camera, middle);
        let endpoint = motion_scene::evaluate(&script, 0, 4.0).unwrap().camera;
        let keys = &script.scenes[0].composition.camera;
        assert_eq!(endpoint.x, keys[1].x);
        assert_eq!(endpoint.zoom, keys[1].zoom);
    }
}
