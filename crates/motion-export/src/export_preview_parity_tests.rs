//! A10: export and desktop preview evaluate the same frame at the same time with
//! the same evaluator and renderer. Uses the G2 minimal scene (A08).
use super::*;
use std::sync::Arc;

fn g2() -> (Script, ArtContext, RenderedScene) {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/g2-minimal-scene/scene.kilsat");
    let project = motion_project::load(&path).unwrap();
    let script = motion_script::parse(&project.source).unwrap();
    let samples = (script.scenes[0].duration * f64::from(SAMPLE_RATE)).round() as u64;
    let rendered = RenderedScene {
        audio: motion_audio::AudioBuffer {
            sample_rate: SAMPLE_RATE,
            channels: 1,
            samples: vec![0.0; samples as usize],
        },
        manifest: SceneManifest {
            schema_version: 1,
            scene_id: script.scenes[0].id.clone(),
            sample_rate: SAMPLE_RATE,
            channels: 1,
            total_frames: samples,
            dialogue: vec![],
            cues: vec![],
        },
    };
    let art = ArtContext {
        document: project.art,
        images: BTreeMap::new(),
    };
    (script, art, rendered)
}

#[test]
fn export_frames_use_frame_over_fps_and_match_the_preview_pixels() {
    let (script, art, rendered) = g2();
    let fps = script.project.fps;
    let preview = ScenePreview::new(
        Arc::new(script.clone()),
        0,
        Arc::new(art.clone()),
        Some(Arc::new(rendered.manifest.clone())),
    )
    .unwrap();
    let cancel = CancellationToken::new();
    for frame in [0u64, 1, 23, 47, 48, 72, 95] {
        let (sample, time, svg) =
            encode_frame(&script, 0, &rendered, frame, false, &cancel, Some(&art)).unwrap();
        // t = frame / fps on the canonical sample clock.
        assert_eq!(
            sample,
            motion_audio::clock::frame_to_sample(frame, fps).unwrap()
        );
        assert_eq!(time, frame as f64 / f64::from(fps));
        // Same evaluator and renderer as a direct evaluation at that time.
        let pose = motion_scene::evaluate(&script, 0, time).unwrap();
        let direct = motion_render::render_svg_with_art(
            &pose,
            pose.width,
            pose.height,
            &[],
            &script.scenes[0].id,
            &art.document,
            &art.images,
        )
        .unwrap();
        assert_eq!(
            String::from_utf8(svg.clone()).unwrap(),
            direct,
            "frame {frame}"
        );
        // The desktop preview of the same sample is the exported SVG, scaled.
        let shown = preview.render(sample, 1024, &cancel).unwrap();
        let scaled =
            media::rasterize_scaled_pixmap(std::str::from_utf8(&svg).unwrap(), Some(1024)).unwrap();
        assert_eq!(
            (shown.width, shown.height),
            (scaled.width(), scaled.height())
        );
        assert!(
            shown.rgba == scaled.data(),
            "preview pixels differ at frame {frame}"
        );
    }
}
