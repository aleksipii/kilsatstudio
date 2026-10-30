//! OpenTimelineIO (`.otio`) timeline for an exported sequence.
//!
//! Written with plain serde: no Python or OTIO library is needed. Every time is
//! derived from the integer 24 kHz sample clock with the same frame rule as the
//! PNG/MP4 export (`frame f` starts at `floor(f * 24000 / fps)`), so cut points
//! are exact frame numbers and never accumulated floating-point sums.
//!
//! Structure: `Timeline → Stack → [Video, Mix, Dialogue…] Track → Clip/Gap`.
//! Video clips reference the flattened render (PNG sequence or movie); scene
//! crossfades are already baked into that media and are marked, not re-applied.
//! Media references are relative to the `.otio` file's directory.
use super::*;
use serde::{de::Error as _, Deserialize, Deserializer, Serializer};
use serde_json::{json, Map, Value};

macro_rules! schema_tag {
    ($name:ident, $schema:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
        pub struct $name;
        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str($schema)
            }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = String::deserialize(deserializer)?;
                if value == $schema {
                    Ok($name)
                } else {
                    Err(D::Error::custom(format!(
                        "expected OTIO_SCHEMA {}, found {value}",
                        $schema
                    )))
                }
            }
        }
    };
}
schema_tag!(RationalTimeSchema, "RationalTime.1");
schema_tag!(TimeRangeSchema, "TimeRange.1");
schema_tag!(TimelineSchema, "Timeline.1");
schema_tag!(StackSchema, "Stack.1");
schema_tag!(TrackSchema, "Track.1");
schema_tag!(MarkerSchema, "Marker.2");

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RationalTime {
    #[serde(rename = "OTIO_SCHEMA")]
    pub schema: RationalTimeSchema,
    pub rate: f64,
    pub value: f64,
}
impl RationalTime {
    /// Exact for integers below 2^53; all Kilsat counts are far smaller.
    pub fn new(value: u64, rate: u32) -> Self {
        Self {
            schema: RationalTimeSchema,
            rate: f64::from(rate),
            value: value as f64,
        }
    }
    /// The integer value, or `None` if it is fractional, negative or out of range.
    pub fn whole(&self) -> Option<u64> {
        (self.value.is_finite()
            && self.value >= 0.0
            && self.value.fract() == 0.0
            && self.value < 9_007_199_254_740_992.0)
            .then_some(self.value as u64)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeRange {
    #[serde(rename = "OTIO_SCHEMA")]
    pub schema: TimeRangeSchema,
    pub duration: RationalTime,
    pub start_time: RationalTime,
}
impl TimeRange {
    pub fn new(start: u64, duration: u64, rate: u32) -> Self {
        Self {
            schema: TimeRangeSchema,
            duration: RationalTime::new(duration, rate),
            start_time: RationalTime::new(start, rate),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Timeline {
    #[serde(rename = "OTIO_SCHEMA")]
    pub schema: TimelineSchema,
    pub metadata: Map<String, Value>,
    pub name: String,
    pub global_start_time: Option<RationalTime>,
    pub tracks: Stack,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stack {
    #[serde(rename = "OTIO_SCHEMA")]
    pub schema: StackSchema,
    pub metadata: Map<String, Value>,
    pub name: String,
    pub source_range: Option<TimeRange>,
    pub effects: Vec<Value>,
    pub markers: Vec<Marker>,
    pub enabled: bool,
    pub children: Vec<Track>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    #[serde(rename = "OTIO_SCHEMA")]
    pub schema: TrackSchema,
    pub metadata: Map<String, Value>,
    pub name: String,
    pub source_range: Option<TimeRange>,
    pub effects: Vec<Value>,
    pub markers: Vec<Marker>,
    pub enabled: bool,
    pub children: Vec<Item>,
    /// `"Video"` or `"Audio"`.
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "OTIO_SCHEMA")]
pub enum Item {
    #[serde(rename = "Clip.2")]
    Clip(Clip),
    #[serde(rename = "Gap.1")]
    Gap(Gap),
}
impl Item {
    pub fn source_range(&self) -> Option<&TimeRange> {
        match self {
            Item::Clip(clip) => clip.source_range.as_ref(),
            Item::Gap(gap) => gap.source_range.as_ref(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    pub metadata: Map<String, Value>,
    pub name: String,
    pub source_range: Option<TimeRange>,
    pub effects: Vec<Value>,
    pub markers: Vec<Marker>,
    pub enabled: bool,
    pub media_references: BTreeMap<String, MediaReference>,
    pub active_media_reference_key: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Gap {
    pub metadata: Map<String, Value>,
    pub name: String,
    pub source_range: Option<TimeRange>,
    pub effects: Vec<Value>,
    pub markers: Vec<Marker>,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "OTIO_SCHEMA")]
pub enum MediaReference {
    #[serde(rename = "ExternalReference.1")]
    External {
        metadata: Map<String, Value>,
        name: String,
        available_range: Option<TimeRange>,
        available_image_bounds: Option<Value>,
        target_url: String,
    },
    #[serde(rename = "ImageSequenceReference.1")]
    ImageSequence {
        metadata: Map<String, Value>,
        name: String,
        available_range: Option<TimeRange>,
        available_image_bounds: Option<Value>,
        target_url_base: String,
        name_prefix: String,
        name_suffix: String,
        start_frame: u64,
        frame_step: u64,
        rate: f64,
        frame_zero_padding: u32,
        missing_frame_policy: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Marker {
    #[serde(rename = "OTIO_SCHEMA")]
    pub schema: MarkerSchema,
    pub metadata: Map<String, Value>,
    pub name: String,
    pub color: String,
    pub marked_range: TimeRange,
    pub comment: String,
}

/// Which exported video file the timeline's video clips point at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OtioVideo {
    /// The `frame-%06d.png` sequence written by the PNG/MP4/ProRes sequence exports.
    PngSequence,
    /// A movie in the same directory, e.g. `sequence.mp4` or `sequence.mov`.
    Movie(String),
}

const DEFAULT_MEDIA: &str = "DEFAULT_MEDIA";

fn kilsat(value: Value) -> Map<String, Value> {
    let mut map = Map::new();
    map.insert("kilsat".into(), value);
    map
}

fn clip(
    name: String,
    range: TimeRange,
    media: MediaReference,
    metadata: Map<String, Value>,
    markers: Vec<Marker>,
) -> Item {
    Item::Clip(Clip {
        metadata,
        name,
        source_range: Some(range),
        effects: Vec::new(),
        markers,
        enabled: true,
        media_references: BTreeMap::from([(DEFAULT_MEDIA.into(), media)]),
        active_media_reference_key: DEFAULT_MEDIA.into(),
    })
}

fn gap(duration: u64, rate: u32) -> Item {
    Item::Gap(Gap {
        metadata: Map::new(),
        name: String::new(),
        source_range: Some(TimeRange::new(0, duration, rate)),
        effects: Vec::new(),
        markers: Vec::new(),
        enabled: true,
    })
}

fn track(name: String, kind: &str, enabled: bool, children: Vec<Item>) -> Track {
    Track {
        schema: TrackSchema,
        metadata: Map::new(),
        name,
        source_range: None,
        effects: Vec::new(),
        markers: Vec::new(),
        enabled,
        children,
        kind: kind.into(),
    }
}

/// First output frame whose start sample is at or after `sample`.
fn first_frame_at(sample: u64, fps: u32) -> Result<u64, Error> {
    sample
        .checked_mul(u64::from(fps))
        .map(|n| n.div_ceil(u64::from(SAMPLE_RATE)))
        .ok_or_else(|| invalid("OTIO frame clock overflow"))
}

/// Build the OTIO timeline for a sequence export of `script`. Runs the same
/// preflight as the export, so it fails for scripts the export would reject.
pub fn sequence_otio(
    script: &Script,
    rendered: &[RenderedScene],
    video: &OtioVideo,
) -> Result<Timeline, Error> {
    if let OtioVideo::Movie(name) = video {
        if name.is_empty() || name.contains(['/', '\\', '\0']) || name.starts_with('.') {
            return Err(invalid(
                "OTIO movie must be a plain file name in the export directory",
            ));
        }
    }
    let (timeline, total_frames, _) =
        sequence::preflight(script, rendered, None, true, CaptionMode::Follow)?;
    let fps = script.project.fps;
    let total_samples = timeline.total_samples();
    let video_media = || match video {
        OtioVideo::PngSequence => MediaReference::ImageSequence {
            metadata: Map::new(),
            name: "frames".into(),
            available_range: Some(TimeRange::new(0, total_frames, fps)),
            available_image_bounds: None,
            target_url_base: "./".into(),
            name_prefix: "frame-".into(),
            name_suffix: ".png".into(),
            start_frame: 0,
            frame_step: 1,
            rate: f64::from(fps),
            frame_zero_padding: 6,
            missing_frame_policy: "error".into(),
        },
        OtioVideo::Movie(name) => MediaReference::External {
            metadata: Map::new(),
            name: name.clone(),
            available_range: Some(TimeRange::new(0, total_frames, fps)),
            available_image_bounds: None,
            target_url: name.clone(),
        },
    };
    let audio_media = || MediaReference::External {
        metadata: Map::new(),
        name: "audio.wav".into(),
        available_range: Some(TimeRange::new(0, total_samples, SAMPLE_RATE)),
        available_image_bounds: None,
        target_url: "audio.wav".into(),
    };

    // Video: one clip per scene, cut at the first frame of each incoming scene.
    let spans = timeline.spans();
    let mut cuts = Vec::with_capacity(spans.len() + 1);
    for span in spans {
        cuts.push(first_frame_at(span.start_sample, fps)?);
    }
    cuts.push(total_frames);
    let mut video_items = Vec::new();
    for (index, span) in spans.iter().enumerate() {
        let (start, end) = (cuts[index], cuts[index + 1]);
        if end <= start {
            continue; // Scene shorter than one output frame; it never starts a frame.
        }
        let mut markers = Vec::new();
        if index > 0 && spans[index - 1].outgoing_overlap > 0 {
            let overlap_end =
                first_frame_at(span.start_sample + spans[index - 1].outgoing_overlap, fps)?
                    .min(end);
            markers.push(Marker {
                schema: MarkerSchema,
                metadata: Map::new(),
                name: "Crossfade (baked)".into(),
                color: "PURPLE".into(),
                marked_range: TimeRange::new(start, overlap_end - start, fps),
                comment: "Dissolve from the previous scene is already rendered into this media."
                    .into(),
            });
        }
        video_items.push(clip(
            span.scene_id.clone(),
            TimeRange::new(start, end - start, fps),
            video_media(),
            kilsat(json!({
                "scene_index": span.scene_index,
                "scene_id": span.scene_id,
                "start_sample": span.start_sample,
                "duration_samples": span.duration_samples,
                "outgoing_overlap_samples": span.outgoing_overlap,
            })),
            markers,
        ));
    }

    // Dialogue: global sample placement on the same clock, one or more lanes per
    // actor so crossfade overlaps never collide on a track.
    let mut lines = Vec::new();
    for span in spans {
        for clip in &rendered[span.scene_index].manifest.dialogue {
            if clip.duration_frames > 0 {
                lines.push((span.start_sample + clip.start_frame, span, clip));
            }
        }
    }
    lines.sort_by(|a, b| (a.0, &a.2.actor).cmp(&(b.0, &b.2.actor)));
    let mut lanes: BTreeMap<&str, Vec<(u64, Vec<Item>)>> = BTreeMap::new();
    for (start, span, line) in lines {
        let actor_lanes = lanes.entry(line.actor.as_str()).or_default();
        let lane = match actor_lanes.iter().position(|(end, _)| *end <= start) {
            Some(lane) => lane,
            None => {
                actor_lanes.push((0, Vec::new()));
                actor_lanes.len() - 1
            }
        };
        let (end, items) = &mut actor_lanes[lane];
        if start > *end {
            items.push(gap(start - *end, SAMPLE_RATE));
        }
        let provenance = match &line.alignment_source {
            None => Value::Null,
            Some(source) => serde_json::to_value(source)?,
        };
        items.push(clip(
            format!("{} · {}", line.actor, span.scene_id),
            TimeRange::new(start, line.duration_frames, SAMPLE_RATE),
            audio_media(),
            kilsat(json!({
                "actor": line.actor,
                "scene_id": span.scene_id,
                "scene_start_sample": line.start_frame,
                "source_trim_samples": line.trim_start_frame,
                "gain": line.gain,
                "alignment_source": provenance,
                "audio": "slice of the mixed audio.wav, not an isolated stem",
            })),
            Vec::new(),
        ));
        *end = start + line.duration_frames;
    }

    let mut tracks = vec![
        track("Video".into(), "Video", true, video_items),
        track(
            "Mix".into(),
            "Audio",
            true,
            vec![clip(
                "audio.wav".into(),
                TimeRange::new(0, total_samples, SAMPLE_RATE),
                audio_media(),
                Map::new(),
                Vec::new(),
            )],
        ),
    ];
    for (actor, actor_lanes) in lanes {
        for (lane, (_, items)) in actor_lanes.into_iter().enumerate() {
            let name = if lane == 0 {
                format!("Dialogue: {actor}")
            } else {
                format!("Dialogue: {actor} ({})", lane + 1)
            };
            // Disabled: these slices are already in Mix; they mark line placement.
            tracks.push(track(name, "Audio", false, items));
        }
    }
    Ok(Timeline {
        schema: TimelineSchema,
        metadata: kilsat(json!({
            "otio_writer": "kilsat motion-export",
            "writer_schema": 1,
            "fps": fps,
            "sample_rate": SAMPLE_RATE,
            "frame_count": total_frames,
            "total_samples": total_samples,
            "frame_rule": "frame f starts at sample floor(f * sample_rate / fps)",
            "color": "sRGB-encoded, Rec.709/sRGB primaries; no ACES/OCIO pipeline",
        })),
        name: script.project.title.clone(),
        global_start_time: Some(RationalTime::new(0, fps)),
        tracks: Stack {
            schema: StackSchema,
            metadata: Map::new(),
            name: "tracks".into(),
            source_range: None,
            effects: Vec::new(),
            markers: Vec::new(),
            enabled: true,
            children: tracks,
        },
    })
}

/// Serialize a timeline as indented `.otio` JSON.
pub fn otio_json(timeline: &Timeline) -> Result<Vec<u8>, Error> {
    let mut bytes = serde_json::to_vec_pretty(timeline)?;
    bytes.push(b'\n');
    if bytes.len() > 8 * 1024 * 1024 {
        return Err(invalid("OTIO timeline exceeds 8 MiB"));
    }
    Ok(bytes)
}

/// Parse `.otio` JSON written by Kilsat (or any file using the same schemas).
pub fn parse_otio(bytes: &[u8]) -> Result<Timeline, Error> {
    if bytes.len() > 8 * 1024 * 1024 {
        return Err(invalid("OTIO timeline exceeds 8 MiB"));
    }
    Ok(serde_json::from_slice(bytes)?)
}

/// Write `timeline.otio` into an existing export directory (never overwrites).
pub fn write_sequence_otio(
    script: &Script,
    rendered: &[RenderedScene],
    directory: &Path,
    video: &OtioVideo,
) -> Result<PathBuf, Error> {
    let bytes = otio_json(&sequence_otio(script, rendered, video)?)?;
    let path = directory.join("timeline.otio");
    write_new(&path, &bytes)?;
    Ok(path)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use motion_voice::export::{DialogueManifest, SceneManifest};

    /// Two 1 s scenes (red, blue) with an optional 0.5 s crossfade and dialogue
    /// that overlaps across the crossfade for EMMA.
    pub(crate) fn two_scene_fixture(fade: bool) -> (Script, Vec<RenderedScene>) {
        let (mut script, first) = crate::tests::fixture();
        script.project.width = 8;
        script.project.height = 8;
        script.project.title = "P07 golden".into();
        script.scenes[0].composition.background_color = "#ff0000".into();
        let mut second = script.scenes[0].clone();
        second.id = "second".into();
        second.composition.background_color = "#0000ff".into();
        if fade {
            script.scenes[0].composition.transition =
                motion_script::Transition::Crossfade { duration: 0.5 };
        }
        script.scenes.push(second);
        let line = |actor: &str, start_frame, duration_frames| DialogueManifest {
            actor: actor.into(),
            start_frame,
            duration_frames,
            trim_start_frame: 0,
            gain: 1.0,
            alignment_source: None,
            visemes: Vec::new(),
        };
        let second = RenderedScene {
            audio: first.audio.clone(),
            manifest: SceneManifest {
                scene_id: "second".into(),
                dialogue: vec![line("EMMA", 0, 6000), line("ALEX", 9000, 3000)],
                ..first.manifest.clone()
            },
        };
        (script, vec![first, second])
    }

    fn frames(range: &TimeRange) -> (u64, u64) {
        (
            range.start_time.whole().unwrap(),
            range.duration.whole().unwrap(),
        )
    }

    #[test]
    fn golden_file_matches_and_round_trips() {
        let (script, rendered) = two_scene_fixture(true);
        let timeline = sequence_otio(&script, &rendered, &OtioVideo::PngSequence).unwrap();
        let bytes = otio_json(&timeline).unwrap();
        let golden_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/two_scene_crossfade.otio");
        if std::env::var_os("KILSAT_UPDATE_GOLDEN").is_some() {
            fs::write(&golden_path, &bytes).unwrap();
        }
        let golden = fs::read(&golden_path).unwrap();
        assert_eq!(
            String::from_utf8(bytes.clone()).unwrap(),
            String::from_utf8(golden.clone()).unwrap(),
            "regenerate with KILSAT_UPDATE_GOLDEN=1 after reviewing the change"
        );
        let parsed = parse_otio(&golden).unwrap();
        assert_eq!(parsed, timeline);
        assert_eq!(otio_json(&parsed).unwrap(), golden);
        let names: Vec<_> = parsed
            .tracks
            .children
            .iter()
            .map(|t| (t.name.as_str(), t.kind.as_str(), t.enabled))
            .collect();
        assert_eq!(
            names,
            [
                ("Video", "Video", true),
                ("Mix", "Audio", true),
                ("Dialogue: ALEX", "Audio", false),
                ("Dialogue: EMMA", "Audio", false),
                ("Dialogue: EMMA (2)", "Audio", false),
            ]
        );
    }

    #[test]
    fn parse_rejects_wrong_schema_versions() {
        let (script, rendered) = two_scene_fixture(false);
        let timeline = sequence_otio(&script, &rendered, &OtioVideo::PngSequence).unwrap();
        let text = String::from_utf8(otio_json(&timeline).unwrap()).unwrap();
        for (from, to) in [
            ("\"Timeline.1\"", "\"Timeline.9\""),
            ("\"Clip.2\"", "\"Clip.7\""),
            ("\"RationalTime.1\"", "\"RationalTime.0\""),
        ] {
            let error = parse_otio(text.replacen(from, to, 1).as_bytes()).unwrap_err();
            assert!(matches!(error, Error::Json(_)), "{from}: {error}");
        }
    }

    /// Cut points and durations equal the exported frames: every frame of the PNG
    /// export (and so of the MP4 encoded from it) lies in exactly one video clip,
    /// and that clip's scene is the incoming contribution of that frame.
    #[test]
    fn cuts_and_durations_match_exported_frames_exactly() {
        for fps in [4, 7, 13, 24, 30] {
            for fade in [false, true] {
                let (mut script, rendered) = two_scene_fixture(fade);
                script.project.fps = fps;
                let dest = crate::tests::destination();
                let report = export_png_sequence(
                    &script,
                    &rendered,
                    None,
                    &dest,
                    &CancellationToken::new(),
                    |_| {},
                )
                .unwrap();
                let path = write_sequence_otio(
                    &script,
                    &rendered,
                    &dest,
                    &OtioVideo::Movie("sequence.mp4".into()),
                )
                .unwrap();
                let otio = parse_otio(&fs::read(&path).unwrap()).unwrap();
                let manifest: Value =
                    serde_json::from_slice(&fs::read(dest.join("sequence.json")).unwrap()).unwrap();
                let video = &otio.tracks.children[0];
                let mut cursor = 0u64;
                for item in &video.children {
                    let Item::Clip(clip) = item else {
                        panic!("video track has only clips")
                    };
                    let (start, duration) = frames(clip.source_range.as_ref().unwrap());
                    assert_eq!(
                        clip.source_range.as_ref().unwrap().start_time.rate,
                        f64::from(fps)
                    );
                    assert_eq!(start, cursor, "fps {fps} fade {fade}: contiguous cuts");
                    assert!(duration > 0);
                    let scene = clip.metadata["kilsat"]["scene_index"].as_u64().unwrap();
                    for frame in start..start + duration {
                        let contributions = manifest["frames"][frame as usize]["contributions"]
                            .as_array()
                            .unwrap();
                        let incoming = contributions.last().unwrap()["scene_index"]
                            .as_u64()
                            .unwrap();
                        assert_eq!(incoming, scene, "fps {fps} fade {fade} frame {frame}");
                        if frame == start {
                            // The frame before a cut belongs to the previous scene.
                            assert!(
                                frame == 0
                                    || manifest["frames"][frame as usize - 1]["contributions"]
                                        .as_array()
                                        .unwrap()
                                        .iter()
                                        .all(|c| c["scene_index"].as_u64() != Some(scene))
                            );
                        }
                    }
                    cursor += duration;
                }
                assert_eq!(cursor, report.frame_count);
                assert_eq!(
                    manifest["frames"].as_array().unwrap().len() as u64,
                    report.frame_count
                );
                // Audio placement is sample exact on the same clock.
                let mix = &otio.tracks.children[1].children[0];
                let (start, samples) = frames(mix.source_range().unwrap());
                assert_eq!(
                    (start, samples),
                    (0, manifest["total_samples"].as_u64().unwrap())
                );
                let emma: Vec<(u64, u64)> = otio
                    .tracks
                    .children
                    .iter()
                    .filter(|t| t.name.starts_with("Dialogue: EMMA"))
                    .flat_map(|t| t.children.iter())
                    .filter_map(|i| match i {
                        Item::Clip(c) => Some(frames(c.source_range.as_ref().unwrap())),
                        Item::Gap(_) => None,
                    })
                    .collect();
                let second_start = if fade { 12_000 } else { 24_000 };
                assert!(emma.contains(&(6000, 12_000)));
                assert!(emma.contains(&(second_start, 6000)));
                fs::remove_dir_all(dest).unwrap();
            }
        }
    }

    #[test]
    fn dialogue_lanes_never_overlap_and_gaps_fill_exactly() {
        let (script, rendered) = two_scene_fixture(true);
        let timeline = sequence_otio(&script, &rendered, &OtioVideo::PngSequence).unwrap();
        for track in &timeline.tracks.children[2..] {
            let mut cursor = 0;
            for item in &track.children {
                let (start, duration) = frames(item.source_range().unwrap());
                match item {
                    Item::Gap(_) => cursor += duration,
                    Item::Clip(_) => {
                        assert_eq!(
                            start, cursor,
                            "{}: clip starts where the track is",
                            track.name
                        );
                        cursor += duration;
                    }
                }
            }
        }
    }

    #[test]
    fn invalid_inputs_are_rejected_before_writing() {
        let (script, mut rendered) = two_scene_fixture(true);
        assert!(sequence_otio(&script, &rendered, &OtioVideo::Movie("../a.mov".into())).is_err());
        rendered.pop();
        assert!(sequence_otio(&script, &rendered, &OtioVideo::PngSequence).is_err());
    }
}
