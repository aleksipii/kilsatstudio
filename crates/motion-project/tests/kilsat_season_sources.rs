//! All authored season projects must be loadable by the real application schema.
use std::path::Path;
#[test]
fn all_31_finnish_episode_projects_validate_with_assets_and_distinct_stories() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/kilsat-season");
    let mut titles = std::collections::BTreeSet::new();
    let mut texts = std::collections::BTreeSet::new();
    for number in 1..=31 {
        let path = root.join(format!("ep{number:02}.kilsat"));
        let doc = motion_project::decode(&std::fs::read(&path).unwrap())
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let script = motion_script::parse(&doc.source).unwrap();
        assert_eq!(script.characters.len(), 2, "episode{number}");
        assert!(titles.insert(script.project.title.clone()));
        let seconds: f64 = script.scenes.iter().map(|s| s.duration).sum();
        assert!((30.0..=60.0).contains(&seconds));
        let mut turns = 0;
        for character in script.characters.values() {
            let voice = character.voice.as_ref().unwrap();
            assert_eq!(voice.provider, "piper");
            assert_eq!(voice.language, "fi-fi");
        }
        for scene in &script.scenes {
            for event in &scene.events {
                if let motion_script::Event::Speak { text, .. } = event {
                    assert!(!text.chars().any(|c| c.is_ascii_digit()));
                    texts.insert(text.clone());
                    turns += 1;
                }
            }
        }
        assert_eq!(turns, 8, "episode{number}");
        for asset in &doc.assets {
            assert!(
                root.join(&asset.path).is_file(),
                "missingasset {}",
                asset.path
            );
        }
    }
    assert!(
        texts.len() > 200,
        "stories must contain individually authored dialogue"
    );
}
