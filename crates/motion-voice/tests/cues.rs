use motion_audio::{read_wav, write_wav, AudioBuffer};
use motion_tts::{SpeechError, SpeechProvider, SpeechRequest, SpeechResult};
use motion_voice::scene_audio::{prepare_scene, PreparedScene};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

struct NoSynthesis;
impl SpeechProvider for NoSynthesis {
    fn synthesize(&self, _: SpeechRequest) -> Result<SpeechResult, SpeechError> {
        Err(SpeechError("dialogue uses imported audio".into()))
    }
}
struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "kilsat-cues-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(root.join("audio")).unwrap();
        let project = Self(root);
        project.wav("voice.wav", 0.5, 24000); // one second of dialogue
        project.wav("audio/hit.wav", 0.25, 100);
        project
    }
    fn wav(&self, name: &str, value: f32, frames: usize) {
        write_wav(
            self.0.join(name),
            &AudioBuffer {
                sample_rate: 24000,
                channels: 1,
                samples: vec![value; frames],
            },
        )
        .unwrap();
    }
    fn script(&self, cues: &str) -> motion_script::Script {
        motion_script::parse(&format!(
            "characters: {{EMMA: {{asset: characters/emma}}}}\nscenes:\n  - id: a\n    duration: 1.0\n    composition:\n      audio_cues: {cues}\n    events:\n      - {{type: speak, at: 0.0, actor: EMMA, text: Hi, audio: voice.wav}}\n"
        ))
        .unwrap()
    }
    fn prepare(&self, cues: &str) -> Result<PreparedScene, SpeechError> {
        prepare_scene(
            &self.script(cues),
            0,
            &self.0,
            &NoSynthesis,
            &BTreeMap::new(),
            |_| {},
        )
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn cue_is_placed_on_exact_sample_and_overlaps_dialogue() {
    let p = Project::new();
    let scene = p
        .prepare("[{id: hit, asset: audio/hit.wav, at: 0.5, gain: 2.0}]")
        .unwrap();
    let a = &scene.rendered.audio.samples;
    assert_eq!(a.len(), 24000);
    assert_eq!(a[11999], 0.5, "dialogue only before the cue");
    assert_eq!(a[12000], 0.5 + 0.25 * 2.0, "overlap sums on the same clock");
    assert_eq!(a[12099], 1.0);
    assert_eq!(a[12100], 0.5, "cue ends after its 100 source frames");
    let cue = &scene.rendered.manifest.cues[0];
    assert_eq!(
        (cue.start_frame, cue.duration_frames, cue.gain),
        (12000, 100, 2.0)
    );
}

#[test]
fn summed_cues_keep_headroom_and_clip_only_in_wav_output() {
    let p = Project::new();
    let scene = p
        .prepare("[{id: a, asset: audio/hit.wav, at: 0.0, gain: 4.0}, {id: b, asset: audio/hit.wav, at: 0.0, gain: 4.0}]")
        .unwrap();
    assert_eq!(scene.rendered.audio.samples[0], 0.5 + 1.0 + 1.0);
    let out = p.0.join("out.wav");
    write_wav(&out, &scene.rendered.audio).unwrap();
    assert!(read_wav(out)
        .unwrap()
        .samples
        .iter()
        .all(|s| s.abs() <= 1.0));
}

#[test]
fn trim_duration_and_scene_end_truncation_are_sample_exact() {
    let p = Project::new();
    p.wav("ramp.wav", 0.125, 24000);
    let scene = p
        .prepare("[{id: t, asset: audio/hit.wav, at: 0.25, trim_start: 0.001, duration: 0.001}, {id: end, asset: ramp.wav, at: 0.99995}]")
        .unwrap();
    let (t, end) = (
        &scene.rendered.manifest.cues[0],
        &scene.rendered.manifest.cues[1],
    );
    assert_eq!(
        (t.start_frame, t.trim_start_frame, t.duration_frames),
        (6000, 24, 24)
    );
    let a = &scene.rendered.audio.samples;
    assert_eq!(a[6000], 0.5 + 0.25);
    assert_eq!(a[6023], 0.5 + 0.25);
    assert_eq!(a[6024], 0.5);
    // 0.99995 s is frame 23999: one sample remains; nothing is written past the scene.
    assert_eq!((end.start_frame, end.duration_frames), (23999, 1));
    assert_eq!(a[23999], 0.5 + 0.125);
}

#[test]
fn rendering_is_deterministic_and_matches_independent_mix() {
    let p = Project::new();
    let cues = "[{id: hit, asset: audio/hit.wav, at: 0.1}, {id: hit2, asset: audio/hit.wav, at: 0.1005, gain: 0.5}]";
    let (first, second) = (p.prepare(cues).unwrap(), p.prepare(cues).unwrap());
    assert_eq!(first.rendered.audio.samples, second.rendered.audio.samples);
    // Preview and export consume this buffer; recompute it through the shared mixer.
    let mut clips: Vec<_> = first.clips.iter().map(|c| c.mix_clip()).collect();
    clips.extend(first.cues.iter().map(|c| c.mix_clip()));
    let direct = motion_audio::mix_timeline(&clips, 24000, 1, 24000).unwrap();
    assert_eq!(direct.samples, first.rendered.audio.samples);
    // Seeking is reading an absolute sample index: there is no stateful clock to drift.
    assert_eq!(first.rendered.audio.samples[2400], 0.75);
    assert_eq!(first.rendered.audio.samples[2412], 0.5 + 0.25 + 0.125);
}

fn parse(cues: &str) -> Result<motion_script::Script, motion_script::Error> {
    motion_script::parse(&format!(
        "characters: {{A: {{asset: a}}}}\nscenes:\n  - {{id: s, duration: 2, composition: {{audio_cues: [{cues}]}}, events: []}}\n"
    ))
}

#[test]
fn script_validation_distinguishes_syntax_reference_and_unsupported() {
    let ok = "{id: x, asset: audio/hit.wav, at: 1}";
    assert!(parse(ok).is_ok());
    assert!(matches!(
        parse("{id: x, asset: a.wav, at: 1, volume: 1}"),
        Err(motion_script::Error::Syntax(_))
    ));
    assert!(matches!(
        parse("{id: x, at: 1}"),
        Err(motion_script::Error::Syntax(_))
    ));
    for bad in [
        "asset: ../hit.wav, at: 1",
        "asset: /abs/hit.wav, at: 1",
        "asset: 'a\\b.wav', at: 1",
        "asset: '', at: 1",
        "asset: audio/hit.wav, at: 2",
        "asset: audio/hit.wav, at: .nan",
        "asset: audio/hit.wav, at: -1",
        "asset: audio/hit.wav, at: 1, gain: 4.5",
        "asset: audio/hit.wav, at: 1, gain: -1",
        "asset: audio/hit.wav, at: 1, gain: .inf",
        "asset: audio/hit.wav, at: 1, trim_start: -1",
        "asset: audio/hit.wav, at: 1, duration: 1.5",
    ] {
        assert!(
            matches!(
                parse(&format!("{{id: x, {bad}}}")),
                Err(motion_script::Error::Validation(_))
            ),
            "{bad}"
        );
    }
    assert!(matches!(
        parse("{id: 'bad id', asset: a.wav, at: 1}"),
        Err(motion_script::Error::Validation(_))
    ));
    assert!(matches!(
        parse("{id: x, asset: audio/hit.mp3, at: 1}"),
        Err(motion_script::Error::Unsupported(_))
    ));
    assert!(matches!(
        parse("{id: x, asset: a.wav, at: 1}, {id: x, asset: a.wav, at: 1}"),
        Err(motion_script::Error::Validation(_))
    ));
    let many = (0..65)
        .map(|i| format!("{{id: c{i}, asset: a.wav, at: 0}}"))
        .collect::<Vec<_>>()
        .join(",");
    assert!(matches!(
        parse(&many),
        Err(motion_script::Error::Validation(_))
    ));
    let sound = "characters: {A: {asset: a}}\nscenes: [{id: s, duration: 2, events: [{type: sound, at: 0, asset: x.wav}]}]";
    assert!(matches!(
        motion_script::parse(sound),
        Err(motion_script::Error::Unsupported(_))
    ));
}

#[test]
fn asset_errors_are_reported_before_any_scene_is_returned() {
    let p = Project::new();
    let err = |cues: &str| p.prepare(cues).err().expect("must fail").0;
    assert!(err("[{id: m, asset: missing.wav, at: 0}]").contains("invalid asset reference"));
    write_wav(
        p.0.join("wide.wav"),
        &AudioBuffer {
            sample_rate: 48000,
            channels: 1,
            samples: vec![0.1; 10],
        },
    )
    .unwrap();
    assert!(err("[{id: w, asset: wide.wav, at: 0}]").contains("24kHz mono PCM16"));
    assert!(err("[{id: t, asset: audio/hit.wav, at: 0, trim_start: 1}]").contains("trim_start"));
    assert!(err("[{id: e, asset: audio/hit.wav, at: 0.99999999}]").contains("scene end"));
    // Decoded-audio budget is 300 s per scene: three 120 s cues exceed it, two do not.
    p.wav("long.wav", 0.0, 24000 * 120);
    assert!(err("[{id: a, asset: long.wav, at: 0}, {id: b, asset: long.wav, at: 0}, {id: c, asset: long.wav, at: 0}]").contains("budget"));
    assert!(p
        .prepare("[{id: a, asset: long.wav, at: 0}, {id: b, asset: long.wav, at: 0}]")
        .is_ok());
}

#[cfg(unix)]
#[test]
fn symlink_escaping_project_root_is_rejected() {
    let p = Project::new();
    let outside = Project::new();
    std::os::unix::fs::symlink(outside.0.join("audio/hit.wav"), p.0.join("link.wav")).unwrap();
    let e = p
        .prepare("[{id: l, asset: link.wav, at: 0}]")
        .err()
        .unwrap()
        .0;
    assert!(e.contains("invalid asset reference"), "{e}");
}

#[test]
fn decoded_cue_pcm_is_reserved_released_on_drop_and_not_leaked_by_errors() {
    use motion_jobs::MemoryBudget;
    use motion_tts::CancellationToken;
    let p = Project::new();
    let script = p
        .script("[{id: a, asset: audio/hit.wav, at: 0.1}, {id: b, asset: audio/hit.wav, at: 0.2}]");
    let scene = &script.scenes[0];
    let cancel = CancellationToken::default();
    // Each cue decodes 100 frames = 400 bytes of f32 PCM.
    let roomy = MemoryBudget::new(1000);
    let clips = motion_voice::cues::prepare_cues_in(scene, &p.0, &cancel, &roomy).unwrap();
    assert_eq!(roomy.used(), 800);
    let kept = clips[0].clone();
    drop(clips);
    assert_eq!(
        roomy.used(),
        400,
        "the first clip's clone still holds its PCM"
    );
    drop(kept);
    assert_eq!(roomy.used(), 0);

    let tight = MemoryBudget::new(600);
    let error = motion_voice::cues::prepare_cues_in(scene, &p.0, &cancel, &tight)
        .err()
        .unwrap()
        .0;
    assert!(
        error.contains("audio cue b decoded PCM needs 400 bytes but only 200 of 600"),
        "{error}"
    );
    assert!(error.contains("close or clear"), "{error}");
    assert_eq!(tight.used(), 0, "the first cue's reservation was released");
    // The refusal is deterministic.
    let again = motion_voice::cues::prepare_cues_in(scene, &p.0, &cancel, &tight)
        .err()
        .unwrap()
        .0;
    assert_eq!(error, again);
}
