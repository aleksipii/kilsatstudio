//! Desktop side of the high-level story format (`kilsat_story: 1`): the
//! embedded stick-studio catalog (six characters, four painted sets) and the
//! assembly of a compiled story into a project with per-scene set artwork.
//! Everything is embedded; building a project performs no I/O.
use crate::project_history::MAX_SOURCE_BYTES;
use motion_script::story::{
    self, CatalogCharacter, CatalogSet, IssueKind, StoryCatalog, StoryCompilation, StoryIssue,
};
use std::collections::BTreeMap;

const MANIFEST: &str = include_str!("../../../examples/stick-studio/manifest.json");

/// `(id, description for the assistant, project)` for each painted set.
const SETS: [(&str, &str, &str); 4] = [
    (
        "toimisto",
        "office interior with desks, daytime",
        include_str!("../../../examples/stick-studio/backgrounds/toimisto.kilsat"),
    ),
    (
        "kahvila",
        "café interior with counter and tables",
        include_str!("../../../examples/stick-studio/backgrounds/kahvila.kilsat"),
    ),
    (
        "pysakki",
        "bus stop on a street at night, outdoors",
        include_str!("../../../examples/stick-studio/backgrounds/pysakki.kilsat"),
    ),
    (
        "hautausmaa",
        "cemetery under a crescent moon, outdoors",
        include_str!("../../../examples/stick-studio/backgrounds/hautausmaa.kilsat"),
    ),
];

/// `(id, personality for the assistant, English voice, speaking rate)`.
/// Finnish uses the one installed Piper voice, so the rate is what tells the
/// characters apart there.
const CAST: [(&str, &str, &str, f64); 6] = [
    ("NOUTAJA", "calm, laconic grim reaper with dry humour; black hood and a scythe", "bm_george", 0.88),
    ("VEIKKO", "ordinary commuter everyone can relate to; blue cap and shoulder bag", "am_michael", 1.0),
    ("PASMA", "pompous small-time boss who turns small things into big ones; red tie and clipboard", "am_adam", 0.96),
    ("KIPINA", "curious inventor whose fix always needs another fix; orange beanie and goggles", "af_bella", 1.08),
    ("KIIRE", "courier who reaches everywhere except a standstill; lime vest and a parcel", "am_eric", 1.18),
    ("SISU", "resourceful, calm grandmother who does not step aside even for the reaper; glasses, violet scarf and a cane", "bf_emma", 0.92),
];
const FINNISH_VOICE: &str = "piper_harri";

pub(crate) struct Built {
    pub document: motion_project::ProjectDocument,
    pub compilation: StoryCompilation,
}

fn catalog_error(message: String) -> Vec<StoryIssue> {
    vec![StoryIssue {
        kind: IssueKind::Invalid,
        path: "catalog".into(),
        message,
        line: None,
        column: None,
    }]
}

fn set_document(project: &str) -> Result<motion_project::ProjectDocument, String> {
    motion_project::decode(project.as_bytes()).map_err(|e| e.to_string())
}

pub(crate) fn catalog() -> Result<StoryCatalog, String> {
    let starter = crate::stick_studio::load()?;
    let script = motion_script::parse(&starter.source).map_err(|e| e.to_string())?;
    let manifest: serde_json::Value = serde_json::from_str(MANIFEST).map_err(|e| e.to_string())?;
    let mut characters = BTreeMap::new();
    for (id, description, english_voice, speed) in CAST {
        let character = script
            .characters
            .get(id)
            .ok_or_else(|| format!("starter project has no character {id}"))?;
        let entry = manifest["cast"]
            .as_array()
            .and_then(|cast| cast.iter().find(|c| c["id"] == id))
            .ok_or_else(|| format!("manifest has no cast entry {id}"))?;
        let mut character = character.clone();
        character.voice = None;
        characters.insert(
            id.to_string(),
            CatalogCharacter {
                name: entry["name"].as_str().unwrap_or(id).to_string(),
                description: description.into(),
                character,
                accent: entry["accent"].as_str().unwrap_or("#ffffff").to_lowercase(),
                english_voice: english_voice.into(),
                finnish_voice: FINNISH_VOICE.into(),
                speed,
            },
        );
    }
    let mut sets = BTreeMap::new();
    for (id, description, project) in SETS {
        let document = set_document(project)?;
        let set_script = motion_script::parse(&document.source).map_err(|e| e.to_string())?;
        let scene = set_script
            .scenes
            .first()
            .ok_or_else(|| format!("set {id} has no scene"))?;
        sets.insert(
            id.to_string(),
            CatalogSet {
                name: set_script.project.title.clone(),
                description: description.into(),
                background_color: scene.composition.background_color.clone(),
            },
        );
    }
    Ok(StoryCatalog { characters, sets })
}

/// Instructions to paste into Claude or ChatGPT before the story idea.
pub(crate) fn guide() -> Result<String, String> {
    Ok(story::authoring_guide(&catalog()?))
}

/// Compile pasted story text and assemble a project with each scene's set artwork.
pub(crate) fn build(
    text: &str,
    measured: Option<&[story::MeasuredLine]>,
) -> Result<Built, Vec<StoryIssue>> {
    let catalog = catalog().map_err(catalog_error)?;
    let compilation = story::compile_story(text, &catalog, measured)?;
    let source =
        motion_script::to_source(&compilation.script).map_err(|e| catalog_error(e.to_string()))?;
    if source.len() > MAX_SOURCE_BYTES {
        return Err(vec![StoryIssue {
            kind: IssueKind::Bound,
            path: "cast".into(),
            message: format!(
                "the compiled screenplay is {} KiB; the editor limit is {} KiB — use fewer characters or scenes",
                source.len() / 1024,
                MAX_SOURCE_BYTES / 1024
            ),
            line: None,
            column: None,
        }]);
    }
    let mut document =
        motion_project::ProjectDocument::new(source).map_err(|e| catalog_error(e.to_string()))?;
    let mut set_layers = BTreeMap::new();
    for (id, _, project) in SETS {
        let set = set_document(project).map_err(catalog_error)?;
        set_layers.insert(id, set.art.layers);
    }
    for (scene_id, set_id) in &compilation.scene_sets {
        let layers = set_layers
            .get(set_id.as_str())
            .ok_or_else(|| catalog_error(format!("set {set_id} has no artwork")))?;
        for layer in layers {
            let mut layer = layer.clone();
            let suffix = layer
                .id
                .strip_prefix(&format!("{}_", layer.scene_id))
                .unwrap_or(&layer.id)
                .to_string();
            layer.id = format!("{scene_id}_{suffix}");
            layer.scene_id = scene_id.clone();
            document.art.layers.push(layer);
        }
    }
    document
        .validate()
        .map_err(|e| catalog_error(e.to_string()))?;
    Ok(Built {
        document,
        compilation,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const EXAMPLE: &str = include_str!("../../../examples/story/kahvitauko.yaml");

    #[test]
    fn catalog_matches_the_embedded_pack() {
        let catalog = catalog().unwrap();
        assert_eq!(catalog.characters.len(), 6);
        assert_eq!(catalog.sets.len(), 4);
        assert!(catalog.characters.values().all(|c| c
            .character
            .rig
            .as_ref()
            .is_some_and(|r| r.eyes.is_some())));
        let guide = guide().unwrap();
        assert!(guide.contains("SISU") && guide.contains("hautausmaa"));
    }

    #[test]
    fn example_story_becomes_a_project_with_set_artwork_per_scene() {
        let built = build(EXAMPLE, None).unwrap();
        let script = motion_script::parse(&built.document.source).unwrap();
        assert_eq!(script.scenes.len(), 2);
        for scene in &script.scenes {
            let layers = built
                .document
                .art
                .layers
                .iter()
                .filter(|l| l.scene_id == scene.id)
                .count();
            assert!(layers > 10, "scene {} has {layers} set layers", scene.id);
        }
        let mut ids: Vec<_> = built.document.art.layers.iter().map(|l| &l.id).collect();
        let count = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), count);
        // The document round-trips through the project format.
        let bytes = motion_project::encode(&built.document).unwrap();
        motion_project::decode(&bytes).unwrap();
    }

    #[test]
    fn every_set_and_character_compiles() {
        for (set, _, _) in SETS {
            for chunk in CAST.chunks(3) {
                let cast: String = chunk
                    .iter()
                    .map(|(id, _, _, _)| format!("  {id}: {{}}\n"))
                    .collect();
                let positions: Vec<String> = chunk
                    .iter()
                    .zip(["left", "center", "right"])
                    .map(|((id, _, _, _), p)| format!("{id}: {p}"))
                    .collect();
                let beats: String = chunk
                    .iter()
                    .map(|(id, _, _, _)| format!("      - say: {id}\n        line: Hello there.\n        gesture: wave\n"))
                    .collect();
                let text = format!(
                    "kilsat_story: 1\ntitle: T\nlanguage: en\ncast:\n{cast}scenes:\n  - id: s\n    set: {set}\n    positions: {{{}}}\n    beats:\n{beats}",
                    positions.join(", ")
                );
                build(&text, None).unwrap_or_else(|e| panic!("{set}: {e:#?}"));
            }
        }
    }

    #[test]
    fn errors_are_returned_not_panicked() {
        let issues = build("kilsat_story: 1\ntitle: x\n", None).err().unwrap();
        assert!(!issues.is_empty());
    }
}
