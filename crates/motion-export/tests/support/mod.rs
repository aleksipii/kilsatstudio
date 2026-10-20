//! Builder for `examples/night-room-reference/`, shared by the render test and the
//! `night_room_reference` example. Everything here is original and deterministic:
//! characters come from `base.yaml`, shots from `shots.yaml`, the bedroom and
//! hallway artwork is plain rectangles/ellipses defined below, and the dialogue
//! audio is a generated placeholder tone (not speech).
#![allow(dead_code)]
use motion_art::{BlendMode, Color, Content, Document, Layer, Plane, Rect};
use motion_audio::AudioBuffer;
use motion_export::ArtContext;
use motion_project::{AssetKind, AssetReference, ProjectDocument};
use motion_script::{Event, ShotExpansion, ShotList};
use motion_voice::export::{render_scene, RenderedScene};
use motion_voice::timeline::DialogueClip;
use std::path::{Path, PathBuf};

pub const SAMPLE_RATE: u32 = 24_000;

pub fn example_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/night-room-reference")
}

/// Parse `base.yaml` and `shots.yaml` and expand the shot list.
pub fn expansion() -> ShotExpansion {
    let dir = example_dir();
    let base = std::fs::read_to_string(dir.join("base.yaml")).unwrap();
    let shots = std::fs::read_to_string(dir.join("shots.yaml")).unwrap();
    let base = motion_script::parse(&base).unwrap();
    let list = ShotList::from_yaml(&shots).unwrap();
    motion_script::expand_shots(&base, &list).unwrap()
}

/// Every speak event: `(scene_index, at, actor, audio path, duration seconds)`.
/// Durations come from the shot list, via the planned dialogue interval.
pub fn lines(expansion: &ShotExpansion) -> Vec<(usize, f64, String, String, f64)> {
    let mut out = Vec::new();
    for planned in &expansion.plan {
        let scene = &expansion.script.scenes[planned.scene_index];
        for event in &scene.events {
            if let Event::Speak {
                at,
                actor,
                audio: Some(audio),
                ..
            } = event
            {
                let duration = planned.dialogue_end.unwrap() - planned.dialogue_start.unwrap();
                out.push((
                    planned.scene_index,
                    *at,
                    actor.clone(),
                    audio.clone(),
                    duration,
                ));
            }
        }
    }
    out
}

/// Placeholder dialogue: a quiet gated square tone, 24 kHz mono. It is NOT
/// speech; it only gives the sample clock real, non-silent audio to place.
pub fn placeholder_audio(actor: &str, duration: f64) -> AudioBuffer {
    let half_period = if actor == "DAD" { 60 } else { 46 };
    let n = (duration * f64::from(SAMPLE_RATE)).round() as usize;
    let samples = (0..n)
        .map(|i| {
            // 0.18 s on, 0.07 s off: a crude syllable rhythm.
            if i % 6000 >= 4320 {
                0.0
            } else if (i / half_period) % 2 == 0 {
                0.08
            } else {
                -0.08
            }
        })
        .collect();
    AudioBuffer {
        sample_rate: SAMPLE_RATE,
        channels: 1,
        samples,
    }
}

fn layer(scene: &str, id: &str, plane: Plane, content: Content) -> Layer {
    Layer {
        id: format!("{scene}_{id}"),
        scene_id: scene.into(),
        name: id.replace('_', " "),
        visible: true,
        plane,
        active: None,
        opacity: 1.0,
        attachment: None,
        clip: None,
        clip_shape: None,
        blend: BlendMode::Normal,
        content,
    }
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
}
fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

/// Dark violet bedroom, bed side. `lamp` adds the warm lamp of shot 3.
fn bedroom(scene: &str, lamp: bool) -> Vec<Layer> {
    let bg = Plane::Background;
    let r = |id, x, y, w, h, c: Color| {
        layer(
            scene,
            id,
            bg,
            Content::Rectangle {
                rect: rect(x, y, w, h),
                color: c,
            },
        )
    };
    let mut layers = vec![
        r("wall", 0.0, 0.0, 640.0, 360.0, rgb(38, 26, 66)),
        r("floor", 0.0, 300.0, 640.0, 60.0, rgb(28, 20, 48)),
        r("window", 500.0, 60.0, 90.0, 120.0, rgb(20, 16, 44)),
        layer(
            scene,
            "moon",
            bg,
            Content::Ellipse {
                rect: rect(530.0, 80.0, 24.0, 24.0),
                color: rgb(200, 200, 230),
            },
        ),
        r("headboard", 170.0, 60.0, 300.0, 260.0, rgb(64, 44, 52)),
        layer(
            scene,
            "pillow",
            bg,
            Content::Ellipse {
                rect: rect(262.0, 190.0, 116.0, 56.0),
                color: rgb(85, 75, 115),
            },
        ),
    ];
    if lamp {
        // TODO(lighting): add the lamp glow as a light source (workstream 4).
        layers.push(r(
            "bedside_table",
            470.0,
            250.0,
            80.0,
            110.0,
            rgb(70, 50, 40),
        ));
        layers.push(r(
            "lamp_shade",
            490.0,
            196.0,
            40.0,
            54.0,
            rgb(240, 190, 110),
        ));
    }
    layers.push(layer(
        scene,
        "blanket",
        Plane::Foreground,
        Content::Rectangle {
            rect: rect(120.0, 308.0, 400.0, 60.0),
            color: rgb(84, 60, 128),
        },
    ));
    layers
}

fn doorway(scene: &str) -> Vec<Layer> {
    let bg = Plane::Background;
    let r = |id, x, y, w, h, c: Color| {
        layer(
            scene,
            id,
            bg,
            Content::Rectangle {
                rect: rect(x, y, w, h),
                color: c,
            },
        )
    };
    // TODO(lighting): hallway light spill on the floor (workstream 4).
    vec![
        r("wall", 0.0, 0.0, 640.0, 360.0, rgb(46, 34, 66)),
        r("door_frame", 212.0, 28.0, 216.0, 306.0, rgb(58, 40, 44)),
        r(
            "hallway_opening",
            222.0,
            38.0,
            196.0,
            296.0,
            rgb(72, 176, 124),
        ),
        r("open_door_panel", 150.0, 40.0, 60.0, 290.0, rgb(50, 34, 36)),
        r("floor", 0.0, 322.0, 640.0, 38.0, rgb(30, 22, 46)),
    ]
}

fn hallway(scene: &str) -> Vec<Layer> {
    let bg = Plane::Background;
    let r = |id, x, y, w, h, c: Color| {
        layer(
            scene,
            id,
            bg,
            Content::Rectangle {
                rect: rect(x, y, w, h),
                color: c,
            },
        )
    };
    vec![
        r("hall_wall", 0.0, 0.0, 640.0, 360.0, rgb(28, 70, 64)),
        r("picture_frame", 470.0, 90.0, 70.0, 90.0, rgb(50, 40, 60)),
    ]
}

/// Artwork per scene id; unknown scene ids get none.
pub fn art_document(script: &motion_script::Script) -> Document {
    let mut layers = Vec::new();
    for scene in &script.scenes {
        match scene.id.as_str() {
            "bed_closeup" => layers.extend(bedroom(&scene.id, false)),
            "boy_phone_medium" => layers.extend(bedroom(&scene.id, true)),
            "doorway_wide" => layers.extend(doorway(&scene.id)),
            "dad_closeup" => layers.extend(hallway(&scene.id)),
            _ => {}
        }
    }
    Document {
        layers,
        ..Document::default()
    }
}

/// The project exactly as committed in `night-room.kilsat`.
pub fn project_document(expansion: &ShotExpansion) -> ProjectDocument {
    let source = motion_script::to_source(&expansion.script).unwrap();
    let mut document = ProjectDocument::new(source).unwrap();
    document.art = art_document(&expansion.script);
    for (_, _, _, audio, _) in lines(expansion) {
        document.assets.push(AssetReference {
            id: audio.clone(),
            path: audio,
            kind: AssetKind::Audio,
        });
    }
    document.validate().unwrap();
    document
}

pub fn art_context(document: &ProjectDocument) -> ArtContext {
    ArtContext {
        document: document.art.clone(),
        images: Default::default(),
    }
}

/// `*.wav` is gitignored, so the committed example has no audio; the tones are a pure
/// function of the shot list. Use `dir` when it already holds every fixture, otherwise
/// generate them once per test process into a temporary directory.
fn audio_dir(dir: &Path) -> PathBuf {
    static GENERATED: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    let expansion = expansion();
    let wanted: Vec<_> = lines(&expansion);
    if wanted
        .iter()
        .all(|(_, _, _, audio, _)| dir.join(audio).is_file())
    {
        return dir.to_path_buf();
    }
    GENERATED
        .get_or_init(|| {
            let scratch = std::env::temp_dir()
                .join(format!("kilsat-night-room-audio-{}", std::process::id()));
            for (_, _, actor, audio, duration) in &wanted {
                let path = scratch.join(audio);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                motion_audio::write_wav(path, &placeholder_audio(actor, *duration)).unwrap();
            }
            scratch
        })
        .clone()
}

/// Rendered per-scene audio: the WAV fixtures in `dir` when present, else regenerated.
pub fn rendered_scenes(script: &motion_script::Script, dir: &Path) -> Vec<RenderedScene> {
    let dir = &audio_dir(dir);
    script
        .scenes
        .iter()
        .map(|scene| {
            let clips: Vec<DialogueClip> = scene
                .events
                .iter()
                .filter_map(|event| match event {
                    Event::Speak {
                        at,
                        actor,
                        audio: Some(audio),
                        ..
                    } => {
                        let audio = motion_audio::read_wav(dir.join(audio)).unwrap_or_else(|e| {
                            panic!("{audio}: {e}; run `cargo run -p motion-export --example night_room_reference -- write`")
                        });
                        Some(DialogueClip {
                            actor: actor.clone(),
                            start_frame: (at * f64::from(SAMPLE_RATE)).round() as u64,
                            audio,
                            alignment: None,
                            estimated_alignment: None,
                            gain: 1.0,
                            text_estimated: None,
                            trim_start_frame: 0,
                        })
                    }
                    _ => None,
                })
                .collect();
            render_scene(&scene.id, scene.duration, &clips).unwrap()
        })
        .collect()
}

/// Regenerate the committed example files (project, audio fixtures) in `dir`.
pub fn write_all(dir: &Path) {
    let expansion = expansion();
    let document = project_document(&expansion);
    motion_project::save_atomic(&dir.join("night-room.kilsat"), &document).unwrap();
    for (_, _, actor, audio, duration) in lines(&expansion) {
        let path = dir.join(&audio);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        motion_audio::write_wav(path, &placeholder_audio(&actor, duration)).unwrap();
    }
}

/// Human-readable cut table from the sample-clock timeline (24 kHz samples).
pub fn timing_report(expansion: &ShotExpansion) -> String {
    let timeline = motion_scene::SequenceTimeline::compile(&expansion.script).unwrap();
    let rate = f64::from(motion_scene::SEQUENCE_SAMPLE_RATE);
    let mut out = String::from(
        "shot                 kind      start_s    end_s  dialogue_start_s  start_sample  end_sample\n",
    );
    for (planned, span) in expansion.plan.iter().zip(timeline.spans()) {
        let dialogue = planned
            .dialogue_start
            .map_or("-".to_string(), |s| format!("{s:.3}"));
        out.push_str(&format!(
            "{:<20} {:<9} {:>7.3} {:>8.3} {:>17} {:>13} {:>11}\n",
            planned.scene_id,
            format!("{:?}", planned.kind),
            span.start_sample as f64 / rate,
            (span.start_sample + span.duration_samples) as f64 / rate,
            dialogue,
            span.start_sample,
            span.start_sample + span.duration_samples,
        ));
    }
    out.push_str(&format!(
        "cut points (s): {}\ntotal: {:.3} s, {} samples\n",
        expansion
            .plan
            .iter()
            .skip(1)
            .map(|p| format!("{:.3}", p.start))
            .collect::<Vec<_>>()
            .join(", "),
        timeline.total_samples() as f64 / rate,
        timeline.total_samples()
    ));
    out
}
