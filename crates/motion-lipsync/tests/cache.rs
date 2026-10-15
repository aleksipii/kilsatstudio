use motion_audio::AudioBuffer;
use motion_lipsync::{
    cache::{AlignmentCache, CacheKey},
    Alignment, AlignmentSource, TimedPhoneme,
};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static ID: AtomicU64 = AtomicU64::new(0);
fn root() -> PathBuf {
    std::env::temp_dir().join(format!(
        "alignment-cache-{}-{}",
        std::process::id(),
        ID.fetch_add(1, Ordering::Relaxed)
    ))
}
fn audio() -> AudioBuffer {
    AudioBuffer {
        sample_rate: 100,
        channels: 1,
        samples: vec![0.; 100],
    }
}
fn key(a: &AudioBuffer, text: &str, version: &str, config: &str, edit: &str) -> CacheKey {
    CacheKey::new(a, text, "manual-editor", version, config, edit).unwrap()
}
fn alignment() -> Alignment {
    Alignment {
        source: AlignmentSource::Manual,
        duration_seconds: 1.,
        phonemes: vec![TimedPhoneme {
            phoneme: "m".into(),
            start_seconds: 0.1,
            end_seconds: 0.5,
        }],
    }
}
#[test]
fn exact_audio_and_every_timing_input_invalidate() {
    let root = root();
    let cache = AlignmentCache::new(&root);
    let a = audio();
    let k = key(&a, "hello", "1", "{}", "edit1");
    assert!(cache.load(&k).unwrap().is_none());
    cache.store(&k, &alignment()).unwrap();
    assert_eq!(
        cache.load(&k).unwrap().unwrap().phonemes[0].end_seconds,
        0.5
    );
    for candidate in [
        key(&a, "changed", "1", "{}", "edit1"),
        key(&a, "hello", "2", "{}", "edit1"),
        key(&a, "hello", "1", "new", "edit1"),
        key(&a, "hello", "1", "{}", "edit2"),
    ] {
        assert!(cache.load(&candidate).unwrap().is_none());
    }
    let mut changed = a.clone();
    changed.samples[3] = f32::from_bits(1);
    assert!(cache
        .load(&key(&changed, "hello", "1", "{}", "edit1"))
        .unwrap()
        .is_none());
    let other = CacheKey::new(&a, "hello", "other", "1", "{}", "edit1").unwrap();
    assert!(cache.load(&other).unwrap().is_none());
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn corruption_is_an_error_and_wrong_duration_is_not_stored() {
    let root = root();
    let cache = AlignmentCache::new(&root);
    let k = key(&audio(), "hello", "1", "{}", "edit1");
    cache.store(&k, &alignment()).unwrap();
    let file = std::fs::read_dir(&root)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    value["alignment_json"] = "{}".into();
    std::fs::write(&file, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(cache
        .load(&k)
        .unwrap_err()
        .to_string()
        .contains("integrity"));
    std::fs::write(&file, b"broken").unwrap();
    assert!(cache.load(&k).is_err());
    let mut invalid = alignment();
    invalid.duration_seconds = 2.;
    assert!(cache.store(&k, &invalid).is_err());
    let blank = key(&audio(), "hello", "1", "{}", "");
    assert!(cache.store(&blank, &alignment()).is_err());
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn concurrent_writes_are_complete_and_readable() {
    let root = root();
    let k = key(&audio(), "hello", "1", "{}", "edit1");
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let root = &root;
            let k = &k;
            scope.spawn(move || {
                let cache = AlignmentCache::new(root);
                for _ in 0..8 {
                    cache.store(k, &alignment()).unwrap();
                    cache.load(k).unwrap().unwrap();
                }
            });
        }
    });
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn rejects_oversized_identity_and_mismatched_forced_provenance() {
    let root = root();
    let cache = AlignmentCache::new(&root);
    let a = audio();
    assert!(CacheKey::new(&a, &"x".repeat(65537), "tool", "1", "", "").is_err());
    let k = CacheKey::new(&a, "hello", "tool", "1", "", "").unwrap();
    let mut forced = alignment();
    forced.source = AlignmentSource::Forced {
        tool: "different".into(),
        version: "1".into(),
    };
    assert!(cache.store(&k, &forced).is_err());
    assert!(!root.exists());
    let mut changed = a.clone();
    changed.channels = 2;
    assert_ne!(
        key(&a, "hello", "1", "", "e").audio_sha256(),
        key(&changed, "hello", "1", "", "e").audio_sha256()
    );
}
