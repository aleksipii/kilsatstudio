//! Detached event drafts. Applying a draft must still validate the whole document.
//! The drafts come from the shared `motion_scene::event_edit`, which the iPad
//! timeline uses too.
use motion_script::{Event, Script};

pub use motion_scene::event_edit::Kind;

/// A draft at the nearest scene frame, strictly before the scene endpoint.
pub fn create(
    script: &Script,
    scene_index: usize,
    actor: &str,
    at: f64,
    kind: Kind,
) -> Result<Event, String> {
    motion_scene::event_edit::create(script, scene_index, actor, at, kind)
        .map_err(|error| error.to_string())
}

/// Clone every field except the frame-snapped start time.
pub fn duplicate_at(
    script: &Script,
    scene_index: usize,
    event_index: usize,
    at: f64,
) -> Result<Event, String> {
    motion_scene::event_edit::duplicate_at(script, scene_index, event_index, at)
        .map_err(|error| error.to_string())
}
