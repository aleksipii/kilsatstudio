//! One bounded, in-memory history for screenplay, artwork and asset references.
//! Snapshots never contain audio/image buffers or perform filesystem operations.
use std::collections::VecDeque;

const MAX_ENTRIES: usize = 64;
const MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq)]
struct Metadata {
    format_version: u32,
    assets: Vec<motion_project::AssetReference>,
    art: motion_art::Document,
    timing: motion_project::TimingDocument,
}
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Snapshot {
    pub source: String,
    project: Option<Metadata>,
    // Raw text can temporarily be invalid. Remember this at capture time rather
    // than treating an undo of an incomplete YAML edit as a failed command.
    source_was_valid: bool,
}
impl Snapshot {
    pub fn capture(source: &str, document: Option<&motion_project::ProjectDocument>) -> Self {
        let project = document.map(|d| Metadata {
            format_version: d.format_version,
            assets: d.assets.clone(),
            art: d.art.clone(),
            timing: d.timing.clone(),
        });
        let mut result = Self {
            source: source.into(),
            project,
            source_was_valid: false,
        };
        result.source_was_valid = match result.document() {
            Some(document) => document.validate().is_ok(),
            None => motion_script::parse(source).is_ok(),
        };
        result
    }
    pub fn document(&self) -> Option<motion_project::ProjectDocument> {
        self.project
            .as_ref()
            .map(|p| motion_project::ProjectDocument {
                format_version: p.format_version,
                source: self.source.clone(),
                assets: p.assets.clone(),
                art: p.art.clone(),
                timing: p.timing.clone(),
            })
    }
    fn validate(&self) -> Result<(), String> {
        if self.source.len() > 100_000 {
            return Err("History source exceeds editor limit".into());
        }
        if let Some(p) = &self.project {
            if p.format_version != motion_project::FORMAT_VERSION
                || p.assets.len() > motion_project::MAX_ASSETS
            {
                return Err("Invalid history project metadata".into());
            }
            p.art.validate().map_err(|e| e.to_string())?;
            p.timing.validate().map_err(|e| e.to_string())?;
            let mut ids = std::collections::BTreeSet::new();
            for asset in &p.assets {
                if asset.id.trim().is_empty()
                    || asset.id.len() > 256
                    || !ids.insert(&asset.id)
                    || asset.path.is_empty()
                    || asset.path.len() > 4096
                    || asset
                        .path
                        .chars()
                        .any(|c| c.is_control() || c == '\\' || c == ':')
                    || asset
                        .path
                        .split('/')
                        .any(|c| c.is_empty() || c == "." || c == "..")
                {
                    return Err("Invalid history asset reference".into());
                }
            }
            for layer in &p.art.layers {
                for asset_id in layer.image_assets() {
                    if !p
                        .assets
                        .iter()
                        .any(|a| a.id == *asset_id && a.kind == motion_project::AssetKind::Image)
                    {
                        return Err("History artwork references a missing image asset".into());
                    }
                }
            }
        }
        if self.source_was_valid {
            if let Some(document) = self.document() {
                document.validate().map_err(|e| e.to_string())?;
            } else {
                motion_script::parse(&self.source).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
}
struct Entry {
    snapshot: Snapshot,
    bytes: usize,
}
impl Entry {
    fn new(snapshot: Snapshot) -> Self {
        let bytes = snapshot.source.len().saturating_add(
            serde_json::to_vec(&snapshot.document()).map_or(usize::MAX, |v| v.len()),
        );
        Self { snapshot, bytes }
    }
}
pub(crate) struct History {
    undo: VecDeque<Entry>,
    redo: VecDeque<Entry>,
    bytes: usize,
}
impl History {
    pub fn new(_initial: &str) -> Self {
        Self {
            undo: VecDeque::new(),
            redo: VecDeque::new(),
            bytes: 0,
        }
    }
    pub fn reset(&mut self, _source: &str) {
        self.undo.clear();
        self.redo.clear();
        self.bytes = 0;
    }
    pub fn record(&mut self, before: Snapshot, after: Snapshot) {
        if before == after {
            return;
        }
        self.bytes -= self.redo.iter().map(|e| e.bytes).sum::<usize>();
        self.redo.clear();
        let entry = Entry::new(before);
        if entry.bytes > MAX_BYTES {
            self.undo.clear();
            self.bytes = 0;
            return;
        }
        self.bytes += entry.bytes;
        self.undo.push_back(entry);
        self.trim();
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn navigate(&mut self, current: Snapshot, redo: bool) -> Result<Option<Snapshot>, String> {
        let stack = if redo { &self.redo } else { &self.undo };
        let Some(target) = stack.back() else {
            return Ok(None);
        };
        target.snapshot.validate()?; // Validate before moving either stack.
        current.validate()?;
        let target = if redo {
            self.redo.pop_back()
        } else {
            self.undo.pop_back()
        }
        .unwrap();
        self.bytes -= target.bytes;
        let current = Entry::new(current);
        self.bytes = self.bytes.saturating_add(current.bytes);
        if redo {
            self.undo.push_back(current);
        } else {
            self.redo.push_back(current);
        }
        self.trim();
        Ok(Some(target.snapshot))
    }
    fn trim(&mut self) {
        while self.undo.len() + self.redo.len() > MAX_ENTRIES || self.bytes > MAX_BYTES {
            let removed = self.undo.pop_front().or_else(|| self.redo.pop_front());
            let Some(removed) = removed else {
                break;
            };
            self.bytes -= removed.bytes;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: &str = include_str!("../../../examples/hello-emma.yaml");
    fn document() -> motion_project::ProjectDocument {
        motion_project::ProjectDocument::new(SOURCE).unwrap()
    }
    fn layer() -> motion_art::Layer {
        motion_art::Layer {
            id: "drawn".into(),
            name: "Drawn".into(),
            scene_id: motion_script::parse(SOURCE).unwrap().scenes[0].id.clone(),
            visible: true,
            plane: motion_art::Plane::Foreground,
            active: None,
            opacity: 1.0,
            attachment: None,
            clip: None,
            clip_shape: None,
            blend: motion_art::BlendMode::Normal,
            content: motion_art::Content::Rectangle {
                rect: motion_art::Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 20.0,
                    height: 10.0,
                },
                color: motion_art::Color {
                    r: 1,
                    g: 2,
                    b: 3,
                    a: 255,
                },
            },
        }
    }
    #[test]
    fn timing_apply_is_one_history_step_and_roundtrips_without_audio_buffers() {
        let before = document();
        let mut after = before.clone();
        after.timing.line = "{\"alignment\":null}".into();
        after.timing.scene = "{\"7\":{\"alignment\":null}}".into();
        let mut history = History::new(SOURCE);
        history.record(Snapshot::capture(SOURCE, Some(&before)), Snapshot::capture(SOURCE, Some(&after)));
        let undo = history.navigate(Snapshot::capture(SOURCE, Some(&after)), false).unwrap().unwrap();
        assert!(undo.document().unwrap().timing.is_empty());
        let redo = history.navigate(undo, true).unwrap().unwrap();
        assert_eq!(redo.document().unwrap().timing, after.timing);
        let mut invalid = after;
        invalid.timing.scene = "not JSON".into();
        let current = Snapshot::capture(SOURCE, Some(&invalid));
        assert!(history.navigate(current, false).is_err());
        assert!(history.can_undo());
    }
    #[test]
    fn grouped_artwork_roundtrips_through_chronological_history_and_rejects_cycles() {
        let mut history = History::new(SOURCE);
        let mut doc = document();
        doc.art.layers.push(layer());
        let original = Snapshot::capture(SOURCE, Some(&doc));
        doc.art
            .apply(motion_art::Command::CreateGroup(motion_art::Group {
                id: "group".into(),
                scene_id: doc.art.layers[0].scene_id.clone(),
                plane: motion_art::Plane::Foreground,
                name: "Group".into(),
                visible: true,
                opacity: 0.5,
                blend: motion_art::BlendMode::Multiply,
                children: Vec::new(),
            }))
            .unwrap();
        doc.art
            .apply(motion_art::Command::Move {
                unit: motion_art::ArtUnit::Layer("drawn".into()),
                parent: Some("group".into()),
                index: 0,
            })
            .unwrap();
        let grouped = Snapshot::capture(SOURCE, Some(&doc));
        history.record(original.clone(), grouped.clone());
        let source = format!("# after group\n{SOURCE}");
        let source_edit = Snapshot::capture(&source, Some(&doc));
        history.record(grouped.clone(), source_edit.clone());
        assert_eq!(
            history.navigate(source_edit, false).unwrap(),
            Some(grouped.clone())
        );
        assert_eq!(
            history.navigate(grouped.clone(), false).unwrap(),
            Some(original.clone())
        );
        assert_eq!(
            history.navigate(original, true).unwrap(),
            Some(grouped.clone())
        );
        let mut bad = grouped.clone();
        bad.project.as_mut().unwrap().art.groups[0]
            .children
            .push(motion_art::ArtUnit::Group("group".into()));
        assert!(bad.validate().is_err());
        assert_eq!(grouped.document().unwrap().art, doc.art);
    }
    #[test]
    fn interleaved_source_art_and_inventory_share_one_timeline_and_branch() {
        let mut history = History::new(SOURCE);
        let mut doc = document();
        let initial = Snapshot::capture(SOURCE, Some(&doc));
        let edited_source = format!("# screenplay edit\n{SOURCE}");
        let edited = Snapshot::capture(&edited_source, Some(&doc));
        history.record(initial.clone(), edited.clone());
        doc.art.layers.push(layer());
        let art = Snapshot::capture(&edited_source, Some(&doc));
        history.record(edited.clone(), art.clone());
        doc.assets.push(motion_project::AssetReference {
            id: "local".into(),
            path: "assets/local.wav".into(),
            kind: motion_project::AssetKind::Audio,
        });
        let inventory = Snapshot::capture(&edited_source, Some(&doc));
        history.record(art.clone(), inventory.clone());
        assert_eq!(
            history.navigate(inventory.clone(), false).unwrap(),
            Some(art.clone())
        );
        assert_eq!(
            history.navigate(art.clone(), false).unwrap(),
            Some(edited.clone())
        );
        assert_eq!(
            history.navigate(edited.clone(), false).unwrap(),
            Some(initial.clone())
        );
        assert_eq!(
            history.navigate(initial.clone(), true).unwrap(),
            Some(edited.clone())
        );
        assert_eq!(
            history.navigate(edited.clone(), true).unwrap(),
            Some(art.clone())
        );
        assert_eq!(
            history.navigate(art.clone(), true).unwrap(),
            Some(inventory.clone())
        );
        history.navigate(inventory, false).unwrap();
        let branch = Snapshot::capture(&format!("# new branch\n{SOURCE}"), Some(&doc));
        history.record(art.clone(), branch.clone());
        assert!(!history.can_redo());
        assert_eq!(history.navigate(branch, false).unwrap(), Some(art));
    }
    #[test]
    fn mask_and_blend_restore_in_order_with_source_and_inventory_and_reject_invalid_masks() {
        let mut document = document();
        document.art.layers.push(layer());
        let plain = Snapshot::capture(SOURCE, Some(&document));
        let mut history = History::new(SOURCE);
        document.art.layers[0].clip_shape = Some(motion_art::ClipShape::Ellipse {
            rect: motion_art::Rect {
                x: 0.0,
                y: 0.0,
                width: 20.0,
                height: 10.0,
            },
        });
        document.art.layers[0].blend = motion_art::BlendMode::Multiply;
        let masked = Snapshot::capture(SOURCE, Some(&document));
        history.record(plain.clone(), masked.clone());
        let edited_source = format!("# after mask edit\n{SOURCE}");
        let text = Snapshot::capture(&edited_source, Some(&document));
        history.record(masked.clone(), text.clone());
        document.assets.push(motion_project::AssetReference {
            id: "local".into(),
            path: "assets/local.wav".into(),
            kind: motion_project::AssetKind::Audio,
        });
        let inventory = Snapshot::capture(&edited_source, Some(&document));
        history.record(text.clone(), inventory.clone());
        assert_eq!(
            history.navigate(inventory, false).unwrap(),
            Some(text.clone())
        );
        assert_eq!(history.navigate(text, false).unwrap(), Some(masked.clone()));
        assert_eq!(
            history.navigate(masked.clone(), false).unwrap(),
            Some(plain.clone())
        );
        assert_eq!(history.navigate(plain, true).unwrap(), Some(masked.clone()));
        let restored = masked.document().unwrap();
        assert_eq!(
            restored.art.layers[0].blend,
            motion_art::BlendMode::Multiply
        );
        assert!(matches!(
            restored.art.layers[0].clip_shape,
            Some(motion_art::ClipShape::Ellipse { .. })
        ));
        // Conflicting legacy + advanced masks must not move either history stack.
        document.art.layers[0].clip = Some(motion_art::Rect {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        });
        let invalid = Snapshot::capture(SOURCE, Some(&document));
        history.reset(SOURCE);
        history.record(invalid, masked.clone());
        let counts = (history.undo.len(), history.redo.len(), history.bytes);
        assert!(history.navigate(masked, false).is_err());
        assert_eq!(
            counts,
            (history.undo.len(), history.redo.len(), history.bytes)
        );
    }

    #[test]
    fn completed_painted_mask_gestures_undo_redo_without_asset_inventory_changes() {
        let mut doc = document();
        doc.art.layers.push(layer());
        let initial = Snapshot::capture(SOURCE, Some(&doc));
        let mut history = History::new(SOURCE);
        let mut previous = initial.clone();
        let mut snapshots = Vec::new();
        for erase in [false, true, false] {
            if doc.art.layers[0].clip_shape.is_none() {
                doc.art.layers[0].clip_shape = Some(motion_art::ClipShape::Painted {
                    base_visible: false,
                    strokes: Vec::new(),
                });
            }
            let Some(motion_art::ClipShape::Painted { strokes, .. }) =
                &mut doc.art.layers[0].clip_shape
            else {
                unreachable!()
            };
            strokes.push(motion_art::MaskStroke {
                points: vec![motion_art::Point { x: 5.0, y: 5.0 }],
                radius: if erase { 2.0 } else { 4.0 },
                erase,
                pressures: vec![if erase { 0.25 } else { 0.75 }],
                hardness: 0.5,
            });
            let next = Snapshot::capture(SOURCE, Some(&doc));
            history.record(previous, next.clone());
            previous = next.clone();
            snapshots.push(next);
        }
        assert_eq!(history.undo.len(), 3);
        let mut current = snapshots[2].clone();
        for expected in [&snapshots[1], &snapshots[0], &initial] {
            current = history.navigate(current, false).unwrap().unwrap();
            assert_eq!(&current, expected);
            assert!(current.document().unwrap().assets.is_empty());
        }
        for expected in &snapshots {
            current = history.navigate(current, true).unwrap().unwrap();
            assert_eq!(&current, expected);
        }
        assert!(!history.can_redo());
    }

    #[test]
    fn unfinished_text_is_restored_while_invalid_metadata_is_rejected_atomically() {
        let doc = document();
        let valid = Snapshot::capture(SOURCE, Some(&doc));
        let unfinished = Snapshot::capture("project: [", Some(&doc));
        let mut history = History::new(SOURCE);
        history.record(valid.clone(), unfinished.clone());
        assert_eq!(
            history.navigate(unfinished.clone(), false).unwrap(),
            Some(valid.clone())
        );
        assert_eq!(
            history.navigate(valid.clone(), true).unwrap(),
            Some(unfinished)
        );
        let mut corrupt = valid.clone();
        corrupt
            .project
            .as_mut()
            .unwrap()
            .assets
            .push(motion_project::AssetReference {
                id: "unsafe".into(),
                path: "../escape".into(),
                kind: motion_project::AssetKind::Other,
            });
        history.reset(SOURCE);
        history.record(corrupt, valid.clone());
        let before = (history.undo.len(), history.redo.len(), history.bytes);
        assert!(history.navigate(valid, false).is_err());
        assert_eq!(
            before,
            (history.undo.len(), history.redo.len(), history.bytes)
        );
    }
    #[test]
    fn limits_cover_both_stacks_and_reset_discards_other_project_context() {
        let mut history = History::new("0");
        for i in 0..100 {
            history.record(
                Snapshot::capture(&i.to_string(), None),
                Snapshot::capture(&(i + 1).to_string(), None),
            );
        }
        assert_eq!(history.undo.len(), MAX_ENTRIES);
        let mut current = Snapshot::capture("100", None);
        for _ in 0..32 {
            current = history.navigate(current, false).unwrap().unwrap();
        }
        assert_eq!(history.undo.len() + history.redo.len(), MAX_ENTRIES);
        assert!(history.bytes <= MAX_BYTES);
        history.reset("new project root");
        assert!(!history.can_undo() && !history.can_redo());
        assert_eq!(history.bytes, 0);
        let large = Snapshot::capture(&"x".repeat(90_000), None);
        for i in 0..150 {
            let mut before = large.clone();
            before.source.push_str(&i.to_string());
            let mut after = large.clone();
            after.source.push_str(&(i + 1).to_string());
            history.record(before, after);
        }
        assert!(history.bytes <= MAX_BYTES);
        assert!(history.undo.len() <= MAX_ENTRIES);
    }
    #[test]
    fn artwork_and_asset_metadata_count_toward_the_byte_budget() {
        let mut doc = document();
        for i in 0..200 {
            doc.assets.push(motion_project::AssetReference {
                id: format!("asset-{i}"),
                path: format!("assets/{}.dat", "a".repeat(3900)),
                kind: motion_project::AssetKind::Other,
            });
        }
        let mut history = History::new(SOURCE);
        for i in 0..20 {
            let before = Snapshot::capture(&format!("# {i}\n{SOURCE}"), Some(&doc));
            let after = Snapshot::capture(&format!("# {}\n{SOURCE}", i + 1), Some(&doc));
            history.record(before, after);
        }
        assert!(history.bytes <= MAX_BYTES);
        assert!(history.undo.len() < 20);
        assert!(history.can_undo());
    }

    #[test]
    fn noop_preserves_redo_and_snapshot_never_touches_physical_assets() {
        let mut history = History::new(SOURCE);
        let before = Snapshot::capture(SOURCE, None);
        let mut doc = document();
        doc.assets.push(motion_project::AssetReference {
            id: "missing-on-this-machine".into(),
            path: "assets/retained.wav".into(),
            kind: motion_project::AssetKind::Audio,
        });
        let after = Snapshot::capture(SOURCE, Some(&doc));
        history.record(before.clone(), after.clone());
        history.navigate(after, false).unwrap();
        history.record(before.clone(), before.clone());
        assert!(history.can_redo());
        let restored = history
            .navigate(before, true)
            .unwrap()
            .unwrap()
            .document()
            .unwrap();
        assert_eq!(restored.assets, doc.assets); // No file open, delete, or download needed.
    }
}
