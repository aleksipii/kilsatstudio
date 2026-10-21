//! Builder for `examples/night-room-reference/`, shared by the render test and the
//! `night_room_reference` example. Everything here is original and deterministic:
//! characters come from `base.yaml`, shots from `shots.yaml`, the bedroom and
//! hallway artwork is plain rectangles/ellipses defined below, the lights and the
//! animated phone screen (generated PNG frames) are defined below, and the dialogue
//! audio is a generated placeholder tone (not speech) unless real WAVs are supplied.
#![allow(dead_code)]
use motion_art::{
    BlendMode, Color, Content, Document, Flicker, FlickerMode, ImageFrame, ImageResource, Layer,
    Light, LightSource, Plane, Playback, Point, Rect, ScreenGlow,
};
use motion_audio::AudioBuffer;
use motion_export::ArtContext;
use motion_project::{AssetKind, AssetReference, ProjectDocument};
use motion_script::{Event, ShotExpansion, ShotList};
use motion_voice::export::{render_scene, RenderedScene};
use motion_voice::timeline::DialogueClip;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const SAMPLE_RATE: u32 = 24_000;

pub fn example_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/night-room-reference")
}

/// Parse `base.yaml` and `shots.yaml` and expand the shot list.
pub fn expansion() -> ShotExpansion {
    expansion_with_durations(&BTreeMap::new())
}

/// As [`expansion`], with line durations replaced by `durations[audio path]` seconds
/// (used to rebuild the sequence around real speech whose length differs from the
/// placeholder tones).
pub fn expansion_with_durations(durations: &BTreeMap<String, f64>) -> ShotExpansion {
    let dir = example_dir();
    let base = std::fs::read_to_string(dir.join("base.yaml")).unwrap();
    let shots = std::fs::read_to_string(dir.join("shots.yaml")).unwrap();
    let base = motion_script::parse(&base).unwrap();
    let mut list = ShotList::from_yaml(&shots).unwrap();
    for shot in &mut list.shots {
        if let Some(line) = &mut shot.line {
            if let Some(duration) = line.audio.as_ref().and_then(|a| durations.get(a)) {
                line.duration = *duration;
            }
        }
    }
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
        // The warm glow is a flickering light in `scene_lights`.
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
    // The green hallway light is a light in `scene_lights`.
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

/// Ids of the generated phone-app frames (project image assets).
pub const PHONE_FRAME_IDS: [&str; 3] = ["phone-app-0", "phone-app-1", "phone-app-2"];
/// Seconds each phone frame is shown; the animation loops.
pub const PHONE_FRAME_SECONDS: f64 = 0.6;
/// Axis-aligned screen rectangle over the phone body, in scene pixels. It is the
/// hand joint position of STAN placed at (320, 300) (see `base.yaml`), so the layer
/// matches the rig only for that placement (both phone shots use it).
pub const PHONE_SCREEN: Rect = Rect {
    x: 323.03,
    y: 266.06,
    width: 20.0,
    height: 34.0,
};

/// One 40x68 RGBA frame of an original, made-up "face editor" app screen: a header,
/// a face with eyes and a sliders panel. Pure function of the frame index.
pub fn phone_frame(index: usize) -> ImageResource {
    const W: i32 = 40;
    const H: i32 = 68;
    let (top, bottom, header) = match index {
        0 => ((40, 70, 160), (90, 60, 170), (230, 235, 255)),
        1 => ((30, 120, 150), (50, 80, 170), (255, 235, 200)),
        _ => ((215, 225, 250), (170, 190, 240), (60, 80, 160)),
    };
    let eye_radius = [2.0f64, 3.2, 2.6][index.min(2)];
    let knobs = [[10, 24, 18], [28, 12, 30], [20, 30, 8]][index.min(2)];
    let mut rgba = Vec::with_capacity((W * H * 4) as usize);
    for y in 0..H {
        for x in 0..W {
            let t = f64::from(y) / f64::from(H - 1);
            let mut c = [
                top.0 as f64 + (bottom.0 - top.0) as f64 * t,
                top.1 as f64 + (bottom.1 - top.1) as f64 * t,
                top.2 as f64 + (bottom.2 - top.2) as f64 * t,
            ];
            let (fx, fy) = (f64::from(x) - 20.0, f64::from(y) - 26.0);
            if y < 6 {
                c = [header.0 as f64, header.1 as f64, header.2 as f64];
            }
            if (fx / 12.0).powi(2) + (fy / 15.0).powi(2) <= 1.0 {
                c = [240.0, 196.0, 160.0];
            }
            for ex in [-5.0, 5.0] {
                if (fx - ex).powi(2) + (fy + 3.0).powi(2) <= eye_radius.powi(2) {
                    c = [30.0, 24.0, 40.0];
                }
            }
            if (fx.abs() <= 4.0) && (fy - 8.0).abs() <= 1.0 {
                c = [150.0, 70.0, 70.0];
            }
            // Three slider tracks with knobs.
            for (row, knob) in knobs.iter().enumerate() {
                let ty = 48 + 6 * row as i32;
                if (6..=34).contains(&x) && y == ty {
                    c = [190.0, 195.0, 215.0];
                }
                if (x - knob).abs() <= 1 && (y - ty).abs() <= 2 {
                    c = [255.0, 255.0, 255.0];
                }
            }
            rgba.extend_from_slice(&[c[0] as u8, c[1] as u8, c[2] as u8, 255]);
        }
    }
    ImageResource::from_rgba(W as u32, H as u32, rgba).unwrap()
}

/// Decoded images by project asset id, as the app would load them.
pub fn phone_images() -> BTreeMap<String, ImageResource> {
    PHONE_FRAME_IDS
        .iter()
        .enumerate()
        .map(|(i, id)| ((*id).to_string(), phone_frame(i)))
        .collect()
}

fn phone_asset_path(id: &str) -> String {
    format!("assets/{id}.png")
}

fn phone_screen(scene: &str) -> Layer {
    layer(
        scene,
        "phone_screen",
        Plane::Foreground,
        Content::AnimatedImage {
            rect: PHONE_SCREEN,
            frames: PHONE_FRAME_IDS
                .iter()
                .map(|id| ImageFrame {
                    asset_id: (*id).into(),
                    duration: PHONE_FRAME_SECONDS,
                })
                .collect(),
            playback: Playback::Loop,
        },
    )
}

fn light(scene: &str, id: &str, flicker: Option<Flicker>, source: LightSource) -> Light {
    Light {
        id: format!("{scene}_{id}"),
        scene_id: scene.into(),
        visible: true,
        active: None,
        flicker,
        source,
    }
}

#[allow(clippy::too_many_arguments)]
fn radial(
    position: (f64, f64),
    radius: f64,
    aspect: f64,
    color: Color,
    intensity: f64,
    screen_glow: Option<ScreenGlow>,
) -> LightSource {
    LightSource::Radial {
        position: Point {
            x: position.0,
            y: position.1,
        },
        radius,
        aspect,
        softness: 1.0,
        color,
        intensity,
        blend: BlendMode::Screen,
        attachment: None,
        track: Vec::new(),
        screen_glow,
    }
}

/// Lights per shot: a dark violet multiply tint and vignette everywhere; a cool
/// glow tied to the phone prop in the boy shots; a warm flickering lamp in the
/// medium shot; green hallway light in the doorway and father shots.
fn scene_lights(scene: &str) -> Vec<Light> {
    let mut lights = vec![
        light(
            scene,
            "room_tint",
            None,
            LightSource::Ambient {
                color: rgb(96, 70, 170),
                strength: 0.78,
            },
        ),
        light(
            scene,
            "vignette",
            None,
            LightSource::Vignette {
                strength: 0.55,
                inner: 0.4,
            },
        ),
    ];
    if matches!(scene, "bed_closeup" | "boy_phone_medium") {
        lights.push(light(
            scene,
            "phone_glow",
            None,
            radial(
                (0.0, -20.0),
                105.0,
                1.1,
                rgb(130, 180, 255),
                0.5,
                Some(ScreenGlow {
                    layer_id: format!("{scene}_phone_screen"),
                    gain: 1.0,
                    tint: 0.6,
                }),
            ),
        ));
    }
    match scene {
        "boy_phone_medium" => lights.push(light(
            scene,
            "lamp_glow",
            Some(Flicker {
                mode: FlickerMode::Noise,
                amount: 0.25,
                rate_hz: 6.0,
                seed: 7,
            }),
            radial((510.0, 215.0), 150.0, 1.0, rgb(255, 175, 90), 0.85, None),
        )),
        "doorway_wide" => lights.push(light(
            scene,
            "hallway_light",
            None,
            radial((320.0, 180.0), 220.0, 1.3, rgb(80, 230, 140), 0.3, None),
        )),
        "dad_closeup" => lights.push(light(
            scene,
            "hallway_light",
            None,
            radial((320.0, 140.0), 200.0, 1.0, rgb(80, 230, 140), 0.2, None),
        )),
        _ => {}
    }
    lights
}

/// Artwork per scene id (layers, phone screen, lights); unknown scene ids get none.
pub fn art_document(script: &motion_script::Script) -> Document {
    let mut layers = Vec::new();
    let mut lights = Vec::new();
    for scene in &script.scenes {
        match scene.id.as_str() {
            "bed_closeup" => {
                layers.extend(bedroom(&scene.id, false));
                layers.push(phone_screen(&scene.id));
            }
            "boy_phone_medium" => {
                layers.extend(bedroom(&scene.id, true));
                layers.push(phone_screen(&scene.id));
            }
            "doorway_wide" => layers.extend(doorway(&scene.id)),
            "dad_closeup" => layers.extend(hallway(&scene.id)),
            _ => continue,
        }
        lights.extend(scene_lights(&scene.id));
    }
    Document {
        layers,
        lights,
        ..Document::default()
    }
}

/// The artwork without lights (flat colours), for exact-colour probes.
pub fn unlit_art_document(script: &motion_script::Script) -> Document {
    Document {
        lights: Vec::new(),
        ..art_document(script)
    }
}

/// The project exactly as committed in `night-room.kilsat`.
pub fn project_document(expansion: &ShotExpansion) -> ProjectDocument {
    let source = motion_script::to_source(&expansion.script).unwrap();
    let mut document = ProjectDocument::new(source).unwrap();
    document.art = art_document(&expansion.script);
    for id in PHONE_FRAME_IDS {
        document.assets.push(AssetReference {
            id: id.into(),
            path: phone_asset_path(id),
            kind: AssetKind::Image,
        });
    }
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
        images: phone_images(),
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
/// Each clip gets the text-estimated viseme track from the speak event's text (the
/// derivation the dialogue pipeline uses); no alignment is attached, so the mouth
/// source is "estimated from text", never forced alignment.
pub fn rendered_scenes(script: &motion_script::Script, dir: &Path) -> Vec<RenderedScene> {
    rendered_scenes_from(script, &audio_dir(dir))
}

/// As [`rendered_scenes`] but reads `dir/<audio path>` with no fallback.
pub fn rendered_scenes_from(script: &motion_script::Script, dir: &Path) -> Vec<RenderedScene> {
    let overrides = motion_lipsync::english::Pronunciations::default();
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
                        text,
                        audio: Some(audio),
                        ..
                    } => {
                        let audio = motion_audio::read_wav(dir.join(audio)).unwrap_or_else(|e| {
                            panic!("{audio}: {e}; run `cargo run -p motion-export --example night_room_reference -- write`")
                        });
                        let mut clip = DialogueClip {
                            actor: actor.clone(),
                            start_frame: (at * f64::from(SAMPLE_RATE)).round() as u64,
                            audio,
                            alignment: None,
                            estimated_alignment: None,
                            gain: 1.0,
                            text_estimated: None,
                            trim_start_frame: 0,
                        };
                        clip.derive_text_estimate(text, &overrides)
                            .unwrap_or_else(|e| panic!("text estimate for {text:?}: {e}"));
                        Some(clip)
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
    for (id, image) in phone_images() {
        let path = dir.join(phone_asset_path(&id));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, image.png()).unwrap();
    }
    for (_, _, actor, audio, duration) in lines(&expansion) {
        let path = dir.join(&audio);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        motion_audio::write_wav(path, &placeholder_audio(&actor, duration)).unwrap();
    }
}

/// Number of mouth-variant changes over one clip's manifest intervals, counting a
/// gap or a `Rest` viseme as the rig's base mouth (the acting rigs map every viseme
/// except `rest` to its own variant). `(switches, intervals)`.
pub fn mouth_switches(clip: &motion_voice::export::DialogueManifest) -> (usize, usize) {
    use motion_lipsync::Viseme;
    let mut previous = Viseme::Rest;
    let mut cursor = clip.start_frame;
    let mut switches = 0;
    let mut note = |viseme: Viseme| {
        if viseme != previous {
            switches += 1;
            previous = viseme;
        }
    };
    for interval in &clip.visemes {
        if interval.start_frame > cursor {
            note(Viseme::Rest);
        }
        note(interval.viseme);
        cursor = interval.end_frame;
    }
    note(Viseme::Rest);
    (switches, clip.visemes.len())
}

/// Per-line report: caption window on the global sequence clock, estimate source and
/// mouth-variant switches. The caption window is the speak event's `at` to the end of
/// the clip's audio (what `captions::cues_for_scene` uses).
pub fn dialogue_report(expansion: &ShotExpansion, rendered: &[RenderedScene]) -> String {
    let mut out = String::from(
        "shot                 actor  caption_start_s  caption_end_s  audio_s  mouth_intervals  mouth_switches  text\n",
    );
    for planned in &expansion.plan {
        let scene = &expansion.script.scenes[planned.scene_index];
        let manifest = &rendered[planned.scene_index].manifest;
        for event in &scene.events {
            let Event::Speak {
                at, actor, text, ..
            } = event
            else {
                continue;
            };
            let start = (*at * f64::from(SAMPLE_RATE)).round() as u64;
            let Some(clip) = manifest
                .dialogue
                .iter()
                .find(|c| c.actor == *actor && c.start_frame == start)
            else {
                continue;
            };
            let rate = f64::from(SAMPLE_RATE);
            let (switches, intervals) = mouth_switches(clip);
            out.push_str(&format!(
                "{:<20} {:<6} {:>15.3} {:>14.3} {:>8.3} {:>16} {:>15}  {}\n",
                planned.scene_id,
                actor,
                planned.start + at,
                planned.start + (clip.start_frame + clip.duration_frames) as f64 / rate,
                clip.duration_frames as f64 / rate,
                intervals,
                switches,
                text
            ));
        }
    }
    out
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

/// Line durations (seconds) read from `dir/<audio path>` for every speak event.
pub fn wav_durations(dir: &Path) -> BTreeMap<String, f64> {
    lines(&expansion())
        .into_iter()
        .map(|(_, _, _, audio, _)| {
            let buffer = motion_audio::read_wav(dir.join(&audio))
                .unwrap_or_else(|e| panic!("{}: {e}", dir.join(&audio).display()));
            assert_eq!(buffer.sample_rate, SAMPLE_RATE, "{audio}");
            let seconds = buffer.frames() as f64 / f64::from(SAMPLE_RATE);
            (audio, seconds)
        })
        .collect()
}

/// The expansion and rendered scenes: placeholder tones when `real_audio` is `None`,
/// otherwise the WAVs in that directory with the shot list re-timed to their lengths.
pub fn build(real_audio: Option<&Path>) -> (ShotExpansion, Vec<RenderedScene>) {
    match real_audio {
        None => {
            let expansion = expansion();
            let rendered = rendered_scenes(&expansion.script, &example_dir());
            (expansion, rendered)
        }
        Some(dir) => {
            let expansion = expansion_with_durations(&wav_durations(dir));
            let rendered = rendered_scenes_from(&expansion.script, dir);
            (expansion, rendered)
        }
    }
}

/// One representative global sample per shot: mid-line for shots with dialogue (the
/// caption is up and the mouth moving), mid-shot otherwise.
pub fn representative_samples(expansion: &ShotExpansion) -> Vec<u64> {
    let timeline = motion_scene::SequenceTimeline::compile(&expansion.script).unwrap();
    expansion
        .plan
        .iter()
        .zip(timeline.spans())
        .map(
            |(planned, span)| match (planned.dialogue_start, planned.dialogue_end) {
                (Some(a), Some(b)) => {
                    ((a + b) / 2.0 * f64::from(motion_scene::SEQUENCE_SAMPLE_RATE)).round() as u64
                }
                _ => span.start_sample + span.duration_samples / 2,
            },
        )
        .collect()
}
