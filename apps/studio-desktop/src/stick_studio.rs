//! Embedded, self-contained original starter project; loading performs no I/O.
use crate::project_history::MAX_SOURCE_BYTES;

const PROJECT: &str = include_str!("../../../examples/stick-studio/stick-studio.kilsat");

pub(crate) fn load() -> Result<motion_project::ProjectDocument, String> {
    let document = motion_project::decode(PROJECT.as_bytes()).map_err(|e| e.to_string())?;
    if document.source.len() > MAX_SOURCE_BYTES {
        return Err("Esimerkin käsikirjoitus ylittää muokkaimen 256 KiB:n kokorajan.".into());
    }
    if !document.assets.is_empty() || !document.timing.is_empty() {
        return Err(
            "Tikku-ukkostudion esimerkki ei saa tarvita ulkoisia resursseja tai ajoitusta.".into(),
        );
    }
    let script = motion_script::parse(&document.source).map_err(|e| e.to_string())?;
    if script.scenes.iter().any(|scene| {
        !scene.composition.audio_cues.is_empty()
            || scene
                .events
                .iter()
                .any(|event| matches!(event, motion_script::Event::Speak { .. }))
    }) {
        return Err("Tikku-ukkostudion esimerkin on oltava äänetön.".into());
    }
    Ok(document)
}
