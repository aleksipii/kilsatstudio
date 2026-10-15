use motion_audio::AudioBuffer;
use motion_lipsync::{Alignment, AlignmentSource, TimedPhoneme, Viseme};
use motion_voice::{export::render_scene, timeline::DialogueClip};

fn clip() -> DialogueClip {
    DialogueClip {
        actor: "EMMA".into(),
        start_frame: 3,
        audio: AudioBuffer {
            sample_rate: 24000,
            channels: 1,
            samples: vec![0.25; 24],
        },
        alignment: Some(Alignment {
            source: AlignmentSource::Manual,
            duration_seconds: 0.001,
            phonemes: vec![TimedPhoneme {
                phoneme: "m".into(),
                start_seconds: 0.0,
                end_seconds: 0.0005,
            }],
        }),
        trim_start_frame: 6,
        gain: 0.5,
    }
}

#[test]
fn exported_audio_and_visemes_match_preview_after_trim() {
    let rendered = render_scene("intro", 24.0 / 24000.0, &[clip()]).unwrap();
    assert_eq!(rendered.audio.samples.len(), 24);
    assert_eq!(rendered.audio.samples[2], 0.0);
    assert_eq!(rendered.audio.samples[3], 0.125);
    assert_eq!(rendered.audio.samples[20], 0.125);
    assert_eq!(rendered.audio.samples[21], 0.0);
    let track = &rendered.manifest.dialogue[0];
    assert_eq!(track.duration_frames, 18);
    assert!(matches!(
        track.alignment_source,
        Some(AlignmentSource::Manual)
    ));
    for frame in 0..24 {
        let exported = track
            .visemes
            .iter()
            .find(|v| v.start_frame <= frame && frame < v.end_frame)
            .map(|v| v.viseme)
            .unwrap_or(Viseme::Rest);
        assert_eq!(exported, clip().mouth_at(frame).unwrap());
    }
    let json = serde_json::to_string(&rendered.manifest).unwrap();
    let decoded: motion_voice::export::SceneManifest = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.total_frames, 24);
}

#[test]
fn refuses_overrun_bad_trim_and_invalid_durations() {
    assert!(render_scene("intro", 20.0 / 24000.0, &[clip()]).is_err());
    let mut bad = clip();
    bad.trim_start_frame = 25;
    assert!(render_scene("intro", 1.0, &[bad]).is_err());
    for duration in [f64::NAN, f64::INFINITY, -1.0, 3600.1] {
        assert!(render_scene("intro", duration, &[]).is_err());
    }
    let mut bad = clip();
    bad.start_frame = u64::MAX;
    assert!(render_scene("intro", 1.0, &[bad]).is_err());
}

#[test]
fn empty_scene_and_absent_alignment_are_explicit() {
    let rendered = render_scene("silent", 2.0 / 24000.0, &[]).unwrap();
    assert_eq!(rendered.audio.samples, [0.0, 0.0]);
    assert!(rendered.manifest.dialogue.is_empty());
    let mut clip = clip();
    clip.alignment = None;
    let rendered = render_scene("intro", 1.0, &[clip]).unwrap();
    assert!(rendered.manifest.dialogue[0].alignment_source.is_none());
    assert!(rendered.manifest.dialogue[0].visemes.is_empty());
}

#[test]
fn exact_sample_boundaries_match_preview_despite_float_multiplication() {
    // 7 / 24000 * 24000 rounds slightly above 7 on IEEE doubles. Uncorrected
    // ceil delayed both phoneme onset and release by one sample.
    for trim in [0, 3, 7] {
        let mut clip = clip();
        clip.trim_start_frame = trim;
        clip.start_frame = 2;
        clip.alignment.as_mut().unwrap().phonemes = vec![TimedPhoneme {
            phoneme: "a".into(),
            start_seconds: 7.0 / 24000.0,
            end_seconds: 14.0 / 24000.0,
        }];
        let rendered = render_scene("intro", 32.0 / 24000.0, std::slice::from_ref(&clip)).unwrap();
        for frame in 0..32 {
            let exported = rendered.manifest.dialogue[0]
                .visemes
                .iter()
                .find(|v| v.start_frame <= frame && frame < v.end_frame)
                .map(|v| v.viseme)
                .unwrap_or(Viseme::Rest);
            assert_eq!(
                exported,
                clip.mouth_at(frame).unwrap(),
                "trim {trim}, frame {frame}"
            );
        }
    }
}
