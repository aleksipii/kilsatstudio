//! Caption cues from `speak` events, drawn through the shared SVG path.
//!
//! Timing: a cue starts at the speak event's `at` and, when the prepared scene
//! manifest holds the matching dialogue clip (same actor and placement sample),
//! ends with that clip's audio, so captions follow the same 24 kHz sample clock as
//! dialogue and mouths. Without prepared audio the end is estimated from the text
//! length and the cue is marked `estimated`.
use super::*;
use motion_render::CaptionCue;
use motion_script::{CaptionMode, CaptionStyle};
use std::borrow::Cow;

/// The one place the user-level caption choice meets the screenplay style.
/// Preview, PNG, SVG and video export all build their caption plan through here.
pub(crate) fn style_for(
    script: &Script,
    mode: CaptionMode,
) -> Result<Option<Cow<'_, CaptionStyle>>, Error> {
    mode.resolve(script.project.captions.as_ref())
        .map_err(|e| invalid(format!("captions: {e}")))
}

/// Estimated speaking time used when no prepared audio exists: 0.4 s plus 0.06 s
/// per character, at least one second. This is a display estimate, not alignment.
pub(crate) fn estimated_seconds(text: &str) -> f64 {
    (0.4 + 0.06 * text.chars().count() as f64).max(1.0)
}

/// Cues for one scene in event order, independent of whether they are drawn (the
/// caller resolves `CaptionMode` first via [`style_for`]).
pub(crate) fn cues_for_scene(
    script: &Script,
    scene_index: usize,
    manifest: Option<&SceneManifest>,
) -> Vec<CaptionCue> {
    let Some(scene) = script.scenes.get(scene_index) else {
        return Vec::new();
    };
    let mut used = vec![false; manifest.map_or(0, |m| m.dialogue.len())];
    let mut cues = Vec::new();
    for event in &scene.events {
        let motion_script::Event::Speak {
            at, actor, text, ..
        } = event
        else {
            continue;
        };
        if text.trim().is_empty() {
            continue;
        }
        let start_frame = (*at * f64::from(SAMPLE_RATE)).round() as u64;
        let matched = manifest.and_then(|m| {
            m.dialogue.iter().enumerate().find(|(i, clip)| {
                !used[*i]
                    && clip.actor == *actor
                    && clip.start_frame == start_frame
                    && clip.duration_frames > 0
            })
        });
        let (end, estimated) = match matched {
            Some((index, clip)) => {
                used[index] = true;
                (
                    (clip.start_frame + clip.duration_frames) as f64 / f64::from(SAMPLE_RATE),
                    false,
                )
            }
            None => (*at + estimated_seconds(text), true),
        };
        cues.push(CaptionCue {
            speaker: actor.clone(),
            text: text.clone(),
            start: *at,
            end,
            estimated,
        });
    }
    cues
}

/// Draw the captions active in `pose` into an SVG made for that pose.
pub(crate) fn apply(
    svg: &mut String,
    script: &Script,
    scene_index: usize,
    manifest: Option<&SceneManifest>,
    pose: &motion_scene::EvaluatedScene,
    mode: CaptionMode,
) -> Result<(), Error> {
    let Some(style) = style_for(script, mode)? else {
        return Ok(());
    };
    let cues = cues_for_scene(script, scene_index, manifest);
    motion_render::append_captions(svg, pose, pose.width, pose.height, &style, &cues)
        .map_err(|e| invalid(e.to_string()))
}

/// Reject, before any frame is produced, captions that cannot be laid out
/// (glyphs missing from the bundled font, words wider than a line, overflow).
pub(crate) fn preflight(
    script: &Script,
    scene_index: usize,
    mode: CaptionMode,
) -> Result<(), Error> {
    let Some(style) = style_for(script, mode)? else {
        return Ok(());
    };
    for cue in cues_for_scene(script, scene_index, None) {
        motion_render::validate_caption(
            &style,
            &cue.text,
            script.project.width,
            script.project.height,
        )
        .map_err(|e| invalid(format!("scene {scene_index}: {e}")))?;
    }
    Ok(())
}
