//! X-sheet (exposure sheet): one row per video frame with dialogue, phonemes, the
//! 13-viseme mouth, its classic chart shape and the timing provenance per speaker.
//!
//! Every time is a whole sample on the 24 kHz dialogue clock (`motion_audio::clock`).
//! Frame `f` starts at `frame_to_sample(f, fps)` and the mouth shown for the frame is
//! the one at that first sample, which is the rule video export uses. Timecodes are
//! derived from the integer frame index, never from floating-point seconds.
//!
//! Input tracks carry *scene* samples of the visible (trimmed) window. Use
//! [`XSheetTrack::from_timeline_clip`] to convert an `AudioTimeline` clip, which
//! applies the clip offset and trim. See docs/xsheet.md.
use crate::{
    audio_timeline::{AudioTimeline, AudioTimelineClip},
    english::VisemeCue,
    mouth_chart::{viseme_name, MouthChart},
    AlignmentSource, Viseme,
};
use motion_audio::clock::{frame_to_sample, sample_to_frame, DIALOGUE_SAMPLE_RATE};
use std::{collections::BTreeMap, fmt, fmt::Write as _};

pub const MAX_XSHEET_FPS: u32 = 240;
/// One hour at 240 fps; larger sheets are refused instead of allocated.
pub const MAX_XSHEET_FRAMES: u64 = 864_000;
const MAX_TEXT_BYTES: usize = 4096;

/// Half-open scene-sample interval carrying a piece of dialogue text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XSheetWord {
    pub text: String,
    pub start_sample: u64,
    pub end_sample: u64,
}

/// Half-open scene-sample interval of one IPA phoneme.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XSheetPhoneme {
    pub phoneme: String,
    pub start_sample: u64,
    pub end_sample: u64,
}

/// One dialogue clip as it is heard in the scene.
#[derive(Clone, Debug, PartialEq)]
pub struct XSheetTrack {
    pub clip_id: u64,
    pub speaker: String,
    /// First audible scene sample (after offset and trim).
    pub start_sample: u64,
    /// Exclusive end in scene samples.
    pub end_sample: u64,
    /// `None` when the clip has no mouth timing (the mouth rests).
    pub source: Option<AlignmentSource>,
    /// Scene-sample cues inside `[start_sample, end_sample)`; gaps are `Rest`.
    pub visemes: Vec<VisemeCue>,
    pub phonemes: Vec<XSheetPhoneme>,
    /// Word-level timing when known; otherwise see [`XSheetTrack::with_line`].
    pub words: Vec<XSheetWord>,
}

impl XSheetTrack {
    /// Scene view of an `AudioTimeline` clip: source-local cues are shifted by
    /// `start_sample - trim_start_samples` and clipped to the visible window.
    pub fn from_timeline_clip(clip: &AudioTimelineClip) -> Self {
        let lo = clip.trim_start_samples;
        let hi = lo + clip.duration_samples;
        let to_scene = |local: u64| local - lo + clip.start_sample;
        let window = |start: u64, end: u64| {
            let (s, e) = (start.max(lo), end.min(hi));
            (s < e).then(|| (to_scene(s), to_scene(e)))
        };
        let (source, visemes, phonemes) = match &clip.mouth {
            None => (None, Vec::new(), Vec::new()),
            Some(mouth) => (
                Some(mouth.source.clone()),
                mouth
                    .visemes
                    .iter()
                    .filter_map(|c| {
                        window(c.start_sample, c.end_sample).map(|(s, e)| VisemeCue {
                            start_sample: s,
                            end_sample: e,
                            viseme: c.viseme,
                        })
                    })
                    .collect(),
                mouth
                    .phonemes
                    .iter()
                    .filter_map(|p| {
                        window(p.start_sample, p.end_sample).map(|(s, e)| XSheetPhoneme {
                            phoneme: p.phoneme.clone(),
                            start_sample: s,
                            end_sample: e,
                        })
                    })
                    .collect(),
            ),
        };
        Self {
            clip_id: clip.id,
            speaker: clip.actor.clone(),
            start_sample: clip.start_sample,
            end_sample: clip.end_sample(),
            source,
            visemes,
            phonemes,
            words: Vec::new(),
        }
    }

    /// Attaches the whole line as one span over the visible window. Used when no
    /// word-level timing exists: the line is not split into guessed word times.
    pub fn with_line(mut self, text: &str) -> Self {
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        self.words = if text.is_empty() {
            Vec::new()
        } else {
            vec![XSheetWord {
                text,
                start_sample: self.start_sample,
                end_sample: self.end_sample,
            }]
        };
        self
    }
}

/// Where the mouth timing in a cell came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Provenance {
    /// No clip of this speaker is audible at the frame.
    Silent,
    /// A clip is audible but has no mouth timing.
    Untimed,
    Timed(AlignmentSource),
}

impl Provenance {
    pub fn label(&self) -> String {
        match self {
            Self::Silent => String::new(),
            Self::Untimed => "none".into(),
            Self::Timed(AlignmentSource::Manual) => "manual".into(),
            Self::Timed(AlignmentSource::Forced { tool, version }) => {
                format!("forced:{tool}/{version}")
            }
            Self::Timed(AlignmentSource::Estimated { tool, version }) => {
                format!("estimated:{tool}/{version}")
            }
        }
    }
}

/// Dialogue column of one cell: classic X-sheet notation with a hold bar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WordMark {
    None,
    /// One or more words start inside this frame.
    Start(String),
    /// A word that started earlier is still being spoken.
    Hold,
}

impl WordMark {
    pub fn label(&self) -> &str {
        match self {
            Self::None => "",
            Self::Start(text) => text,
            Self::Hold => "|",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XSheetCell {
    pub word: WordMark,
    /// Phonemes starting inside this frame, space separated.
    pub phonemes: String,
    pub viseme: Viseme,
    pub chart: MouthChart,
    pub provenance: Provenance,
    /// Clip audible at the frame's first sample.
    pub clip_id: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XSheetRow {
    pub frame: u64,
    /// First scene sample of the frame.
    pub sample: u64,
    /// One cell per entry of [`XSheet::speakers`].
    pub cells: Vec<XSheetCell>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XSheet {
    pub scene_id: String,
    pub fps: u32,
    pub duration_samples: u64,
    /// Sorted speaker names; the cell order of every row.
    pub speakers: Vec<String>,
    pub rows: Vec<XSheetRow>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum XSheetError {
    InvalidFrameRate(u32),
    TooManyFrames(u64),
    InvalidTrack {
        clip_id: u64,
        reason: String,
    },
    OverlappingTracks {
        speaker: String,
        first: u64,
        second: u64,
    },
    Timeline(String),
}

impl fmt::Display for XSheetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFrameRate(fps) => {
                write!(f, "X-sheet frame rate {fps} must be 1–{MAX_XSHEET_FPS} fps")
            }
            Self::TooManyFrames(n) => write!(
                f,
                "X-sheet would have {n} frames (limit {MAX_XSHEET_FRAMES}); shorten the scene"
            ),
            Self::InvalidTrack { clip_id, reason } => {
                write!(f, "X-sheet clip {clip_id}: {reason}")
            }
            Self::OverlappingTracks {
                speaker,
                first,
                second,
            } => write!(
                f,
                "X-sheet: clips {first} and {second} of {speaker} overlap; one speaker can say one line at a time"
            ),
            Self::Timeline(e) => write!(f, "X-sheet: invalid audio timeline: {e}"),
        }
    }
}
impl std::error::Error for XSheetError {}

/// `HH:MM:SS:FF` from the integer frame index (non-drop-frame).
pub fn timecode(frame: u64, fps: u32) -> String {
    let fps = u64::from(fps.max(1));
    let (seconds, ff) = (frame / fps, frame % fps);
    format!(
        "{:02}:{:02}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60,
        ff
    )
}

/// Number of frames that show at least one sample of a scene of `duration_samples`.
pub fn frame_count(duration_samples: u64, fps: u32) -> Result<u64, XSheetError> {
    if fps == 0 || fps > MAX_XSHEET_FPS {
        return Err(XSheetError::InvalidFrameRate(fps));
    }
    if duration_samples == 0 {
        return Ok(0);
    }
    let last = sample_to_frame(duration_samples - 1, fps)
        .map_err(|e| XSheetError::Timeline(e.to_string()))?;
    let count = last + 1;
    if count > MAX_XSHEET_FRAMES {
        return Err(XSheetError::TooManyFrames(count));
    }
    Ok(count)
}

fn check_spans<'a>(
    clip_id: u64,
    what: &str,
    spans: impl Iterator<Item = (u64, u64)>,
    window: (u64, u64),
    texts: impl Iterator<Item = &'a str>,
) -> Result<(), XSheetError> {
    let mut previous = window.0;
    for (start, end) in spans {
        if start < previous || end <= start || end > window.1 {
            return Err(XSheetError::InvalidTrack {
                clip_id,
                reason: format!(
                    "{what} [{start}, {end}) must be ordered, non-empty, non-overlapping and inside the clip"
                ),
            });
        }
        previous = end;
    }
    for text in texts {
        if text.len() > MAX_TEXT_BYTES || text.chars().any(char::is_control) {
            return Err(XSheetError::InvalidTrack {
                clip_id,
                reason: format!(
                    "{what} text must be at most {MAX_TEXT_BYTES} bytes without control characters"
                ),
            });
        }
    }
    Ok(())
}

fn validate_track(track: &XSheetTrack, duration_samples: u64) -> Result<(), XSheetError> {
    let id = track.clip_id;
    let invalid = |reason: &str| XSheetError::InvalidTrack {
        clip_id: id,
        reason: reason.into(),
    };
    if track.speaker.trim().is_empty() || track.speaker.chars().any(char::is_control) {
        return Err(invalid("speaker name must be non-empty text"));
    }
    if track.end_sample <= track.start_sample || track.end_sample > duration_samples {
        return Err(invalid("clip must be non-empty and end within the scene"));
    }
    let window = (track.start_sample, track.end_sample);
    check_spans(
        id,
        "viseme",
        track.visemes.iter().map(|c| (c.start_sample, c.end_sample)),
        window,
        std::iter::empty(),
    )?;
    check_spans(
        id,
        "phoneme",
        track
            .phonemes
            .iter()
            .map(|p| (p.start_sample, p.end_sample)),
        window,
        track.phonemes.iter().map(|p| p.phoneme.as_str()),
    )?;
    check_spans(
        id,
        "word",
        track.words.iter().map(|w| (w.start_sample, w.end_sample)),
        window,
        track.words.iter().map(|w| w.text.as_str()),
    )
}

/// Builds the sheet. Tracks are validated; an invalid input yields an error, never a
/// partially filled sheet.
pub fn build(
    scene_id: &str,
    duration_samples: u64,
    fps: u32,
    tracks: &[XSheetTrack],
) -> Result<XSheet, XSheetError> {
    let frames = frame_count(duration_samples, fps)?;
    let mut by_speaker: BTreeMap<&str, Vec<&XSheetTrack>> = BTreeMap::new();
    for track in tracks {
        validate_track(track, duration_samples)?;
        by_speaker.entry(&track.speaker).or_default().push(track);
    }
    for (speaker, list) in &mut by_speaker {
        list.sort_by_key(|t| (t.start_sample, t.clip_id));
        for pair in list.windows(2) {
            if pair[1].start_sample < pair[0].end_sample {
                return Err(XSheetError::OverlappingTracks {
                    speaker: (*speaker).to_owned(),
                    first: pair[0].clip_id,
                    second: pair[1].clip_id,
                });
            }
        }
    }
    let speakers: Vec<String> = by_speaker.keys().map(|s| (*s).to_owned()).collect();
    let lanes: Vec<&Vec<&XSheetTrack>> = by_speaker.values().collect();
    let mut rows = Vec::with_capacity(frames as usize);
    for frame in 0..frames {
        let first =
            frame_to_sample(frame, fps).map_err(|e| XSheetError::Timeline(e.to_string()))?;
        let next = frame_to_sample(frame + 1, fps)
            .map_err(|e| XSheetError::Timeline(e.to_string()))?
            .min(duration_samples);
        let cells = lanes.iter().map(|lane| cell(lane, first, next)).collect();
        rows.push(XSheetRow {
            frame,
            sample: first,
            cells,
        });
    }
    Ok(XSheet {
        scene_id: scene_id.to_owned(),
        fps,
        duration_samples,
        speakers,
        rows,
    })
}

/// One speaker's cell for the frame covering scene samples `[first, next)`.
fn cell(lane: &[&XSheetTrack], first: u64, next: u64) -> XSheetCell {
    // Tracks touching the frame, in time order (they do not overlap).
    let touching = lane
        .iter()
        .filter(|t| t.start_sample < next && first < t.end_sample);
    let mut started = Vec::new();
    let mut held = false;
    let mut phonemes = Vec::new();
    for track in touching.clone() {
        for word in &track.words {
            if (first..next).contains(&word.start_sample) {
                started.push(word.text.as_str());
            } else if word.start_sample < first && first < word.end_sample {
                held = true;
            }
        }
        for p in &track.phonemes {
            if (first..next).contains(&p.start_sample) {
                phonemes.push(p.phoneme.as_str());
            }
        }
    }
    let word = if !started.is_empty() {
        WordMark::Start(started.join(" "))
    } else if held {
        WordMark::Hold
    } else {
        WordMark::None
    };
    let audible = lane
        .iter()
        .find(|t| t.start_sample <= first && first < t.end_sample);
    let (viseme, provenance, clip_id) = match audible {
        None => (Viseme::Rest, Provenance::Silent, None),
        Some(track) => {
            let cues = &track.visemes;
            let i = cues.partition_point(|c| c.end_sample <= first);
            let viseme = cues
                .get(i)
                .filter(|c| c.start_sample <= first)
                .map_or(Viseme::Rest, |c| c.viseme);
            let provenance = track
                .source
                .clone()
                .map_or(Provenance::Untimed, Provenance::Timed);
            (viseme, provenance, Some(track.clip_id))
        }
    };
    XSheetCell {
        word,
        phonemes: phonemes.join(" "),
        viseme,
        chart: MouthChart::from_viseme(viseme),
        provenance,
        clip_id,
    }
}

impl XSheet {
    /// Sheet for a validated `AudioTimeline`. `lines` optionally maps clip IDs to
    /// their dialogue text (attached as whole-line spans).
    pub fn from_audio_timeline(
        timeline: &AudioTimeline,
        fps: u32,
        lines: &BTreeMap<u64, String>,
    ) -> Result<Self, XSheetError> {
        timeline
            .validate()
            .map_err(|e| XSheetError::Timeline(e.to_string()))?;
        debug_assert_eq!(timeline.sample_rate, DIALOGUE_SAMPLE_RATE);
        let tracks: Vec<XSheetTrack> = timeline
            .clips
            .iter()
            .map(|clip| {
                let track = XSheetTrack::from_timeline_clip(clip);
                match lines.get(&clip.id) {
                    Some(text) => track.with_line(text),
                    None => track,
                }
            })
            .collect();
        build(&timeline.scene_id, timeline.duration_samples, fps, &tracks)
    }

    /// Row index showing scene sample `sample` (the playhead), if inside the scene.
    pub fn row_at_sample(&self, sample: u64) -> Option<usize> {
        if sample >= self.duration_samples {
            return None;
        }
        let frame = sample_to_frame(sample, self.fps).ok()?;
        usize::try_from(frame).ok().filter(|&i| i < self.rows.len())
    }

    const COLUMNS: [&'static str; 8] = [
        "frame", "timecode", "speaker", "dialogue", "phonemes", "viseme", "chart", "timing",
    ];

    /// Long-format records: one per frame and speaker; frames without speakers keep
    /// one record with empty speaker fields so every frame is listed.
    fn records(&self) -> Vec<[String; 8]> {
        let mut out = Vec::new();
        for row in &self.rows {
            let base = [row.frame.to_string(), timecode(row.frame, self.fps)];
            if self.speakers.is_empty() {
                let [frame, tc] = base;
                out.push([
                    frame,
                    tc,
                    String::new(),
                    String::new(),
                    String::new(),
                    viseme_name(Viseme::Rest).into(),
                    MouthChart::X.to_string(),
                    String::new(),
                ]);
                continue;
            }
            for (speaker, c) in self.speakers.iter().zip(&row.cells) {
                out.push([
                    base[0].clone(),
                    base[1].clone(),
                    speaker.clone(),
                    c.word.label().to_owned(),
                    c.phonemes.clone(),
                    viseme_name(c.viseme).into(),
                    c.chart.to_string(),
                    c.provenance.label(),
                ]);
            }
        }
        out
    }

    /// RFC 4180 CSV with `\n` line endings. Byte-identical for identical input.
    pub fn to_csv(&self) -> String {
        let mut out = String::new();
        let mut line = |fields: &[&str]| {
            let quoted: Vec<String> = fields.iter().map(|f| csv_field(f)).collect();
            out.push_str(&quoted.join(","));
            out.push('\n');
        };
        line(&Self::COLUMNS);
        for record in self.records() {
            line(&record.each_ref().map(String::as_str));
        }
        out
    }

    /// Fixed-width plain text (columns padded by character count) with a header that
    /// records scene, fps and sample clock. Byte-identical for identical input.
    pub fn to_text(&self) -> String {
        let records = self.records();
        let mut widths = Self::COLUMNS.map(|c| c.chars().count());
        for record in &records {
            for (w, field) in widths.iter_mut().zip(record) {
                *w = (*w).max(field.chars().count());
            }
        }
        let mut out = String::new();
        let _ = writeln!(
            out,
            "# X-sheet · scene {} · {} fps · {} samples @ {} Hz",
            sanitize(&self.scene_id),
            self.fps,
            self.duration_samples,
            DIALOGUE_SAMPLE_RATE
        );
        let mut line = |fields: [&str; 8]| {
            let mut text = String::new();
            for (i, (field, width)) in fields.iter().zip(widths).enumerate() {
                if i > 0 {
                    text.push_str("  ");
                }
                let field = sanitize(field);
                let pad = width.saturating_sub(field.chars().count());
                // Numbers right-aligned, text left-aligned.
                if i == 0 {
                    text.extend(std::iter::repeat_n(' ', pad));
                    text.push_str(&field);
                } else {
                    text.push_str(&field);
                    text.extend(std::iter::repeat_n(' ', pad));
                }
            }
            out.push_str(text.trim_end());
            out.push('\n');
        };
        line(Self::COLUMNS);
        for record in &records {
            line(record.each_ref().map(String::as_str));
        }
        out
    }
}

fn sanitize(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

fn csv_field(field: &str) -> String {
    if field.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio_timeline::{AudioSourceInfo, MouthTiming, SamplePhoneme};
    use Viseme::*;

    const SHA: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    fn cue(start: u64, end: u64, viseme: Viseme) -> VisemeCue {
        VisemeCue {
            start_sample: start,
            end_sample: end,
            viseme,
        }
    }

    fn clip(id: u64, actor: &str, start: u64, trim: u64, duration: u64) -> AudioTimelineClip {
        AudioTimelineClip {
            id,
            actor: actor.into(),
            language: "en-us".into(),
            source: AudioSourceInfo::Imported {
                path: "a.wav".into(),
                pcm_sha256: SHA.into(),
            },
            source_samples: 24_000,
            start_sample: start,
            trim_start_samples: trim,
            duration_samples: duration,
            gain: 1.0,
            mouth: Some(MouthTiming {
                source: AlignmentSource::Manual,
                phonemes: vec![
                    SamplePhoneme {
                        phoneme: "m".into(),
                        start_sample: 0,
                        end_sample: 2000,
                    },
                    SamplePhoneme {
                        phoneme: "ɑ".into(),
                        start_sample: 2000,
                        end_sample: 6000,
                    },
                ],
                visemes: vec![cue(0, 2000, Closed), cue(2000, 6000, A), cue(8000, 9000, U)],
            }),
        }
    }

    fn timeline(duration: u64, clips: Vec<AudioTimelineClip>) -> AudioTimeline {
        let mut t = AudioTimeline::new("s1", duration);
        t.clips = clips;
        t
    }

    #[test]
    fn empty_audio_has_no_rows_and_stable_exports() {
        let sheet =
            XSheet::from_audio_timeline(&timeline(0, vec![]), 24, &BTreeMap::new()).unwrap();
        assert!(sheet.rows.is_empty());
        assert_eq!(
            sheet.to_csv(),
            "frame,timecode,speaker,dialogue,phonemes,viseme,chart,timing\n"
        );
        assert!(sheet
            .to_text()
            .starts_with("# X-sheet · scene s1 · 24 fps · 0 samples"));
        // Silent scene: one rest row per frame, no speakers.
        let sheet = build("s1", 24_000, 24, &[]).unwrap();
        assert_eq!(sheet.rows.len(), 24);
        assert!(sheet.rows.iter().all(|r| r.cells.is_empty()));
        assert_eq!(
            sheet.to_csv().lines().nth(1),
            Some("0,00:00:00:00,,,,rest,X,")
        );
    }

    #[test]
    fn rows_follow_sample_clock_including_partial_last_frame() {
        // 24 kHz / 24 fps = 1000 samples per frame; 2500 samples → 3 frames.
        assert_eq!(frame_count(2500, 24), Ok(3));
        assert_eq!(frame_count(2000, 24), Ok(2));
        assert_eq!(frame_count(1, 24), Ok(1));
        // 30 fps does not divide 24 kHz evenly: frame 1 starts at sample 800.
        let sheet = build("s", 2401, 30, &[]).unwrap();
        let samples: Vec<u64> = sheet.rows.iter().map(|r| r.sample).collect();
        assert_eq!(samples, [0, 800, 1600, 2400]);
        assert_eq!(timecode(29, 30), "00:00:00:29");
        assert_eq!(timecode(30 * 3661 + 5, 30), "01:01:01:05");
        assert_eq!(frame_count(10, 0), Err(XSheetError::InvalidFrameRate(0)));
        assert_eq!(
            frame_count(10, 241),
            Err(XSheetError::InvalidFrameRate(241))
        );
        assert!(matches!(
            frame_count(u64::MAX / 2, 240),
            Err(XSheetError::TooManyFrames(_))
        ));
    }

    #[test]
    fn offset_and_trim_move_mouth_with_audio() {
        // Clip starts at scene sample 3000, trims 1000 source samples.
        let t = timeline(12_000, vec![clip(7, "EMMA", 3000, 1000, 8000)]);
        let mut lines = BTreeMap::new();
        lines.insert(7, "Hi  there".to_string());
        let sheet = XSheet::from_audio_timeline(&t, 24, &lines).unwrap();
        let col: Vec<(Viseme, char)> = sheet
            .rows
            .iter()
            .map(|r| (r.cells[0].viseme, r.cells[0].chart.letter()))
            .collect();
        // Scene 3000..4000 = source 1000..2000 (Closed); 4000..8000 = A; then a gap
        // (Rest) and U at source 8000..9000 = scene 10000..11000; clip ends at 11000.
        assert_eq!(col[2], (Rest, 'X'));
        assert_eq!(col[3], (Closed, 'A'));
        assert_eq!(col[4], (A, 'D'));
        assert_eq!(col[7], (A, 'D'));
        assert_eq!(col[8], (Rest, 'X'));
        assert_eq!(col[10], (U, 'F'));
        assert_eq!(col[11], (Rest, 'X'));
        let cells: Vec<&XSheetCell> = sheet.rows.iter().map(|r| &r.cells[0]).collect();
        assert_eq!(cells[2].provenance, Provenance::Silent);
        assert_eq!(cells[3].provenance.label(), "manual");
        assert_eq!(cells[3].clip_id, Some(7));
        assert_eq!(cells[11].clip_id, None);
        // "m" is partly trimmed: it shows from the first audible sample. "ɑ" starts
        // at scene 4000.
        assert_eq!(cells[3].phonemes, "m");
        assert_eq!(cells[4].phonemes, "ɑ");
        // Whole line at the first audible frame, hold bars to the clip end.
        assert_eq!(cells[3].word, WordMark::Start("Hi there".into()));
        assert!(cells[4..=10].iter().all(|c| c.word == WordMark::Hold));
        assert_eq!(cells[11].word, WordMark::None);
        assert_eq!(sheet.row_at_sample(4999), Some(4));
        assert_eq!(sheet.row_at_sample(12_000), None);
    }

    #[test]
    fn scene_boundary_clip_ending_at_scene_end() {
        // Clip fills the last 1500 samples exactly; last frame is partial.
        let t = timeline(4500, vec![clip(1, "ALEX", 3000, 0, 1500)]);
        let sheet = XSheet::from_audio_timeline(&t, 24, &BTreeMap::new()).unwrap();
        assert_eq!(sheet.rows.len(), 5);
        assert_eq!(sheet.rows[3].cells[0].viseme, Closed);
        assert_eq!(sheet.rows[4].cells[0].viseme, Closed);
        assert_eq!(sheet.rows[4].sample, 4000);
        // A track past the scene end is refused, not truncated.
        let mut track = XSheetTrack::from_timeline_clip(&clip(1, "ALEX", 3000, 0, 1500));
        track.end_sample = 4501;
        assert!(matches!(
            build("s", 4500, 24, &[track]),
            Err(XSheetError::InvalidTrack { clip_id: 1, .. })
        ));
    }

    #[test]
    fn untimed_and_mid_frame_clip_start() {
        let mut c = clip(2, "EMMA", 1500, 0, 1000);
        c.mouth = None;
        let sheet =
            XSheet::from_audio_timeline(&timeline(3000, vec![c]), 24, &BTreeMap::new()).unwrap();
        // Frame 1 starts at 1000, before the clip: silent like export at that sample.
        assert_eq!(sheet.rows[1].cells[0].provenance, Provenance::Silent);
        assert_eq!(sheet.rows[2].cells[0].provenance, Provenance::Untimed);
        assert_eq!(sheet.rows[2].cells[0].provenance.label(), "none");
        assert_eq!(sheet.rows[2].cells[0].viseme, Rest);
    }

    #[test]
    fn speakers_sorted_and_overlap_rejected() {
        let a = XSheetTrack::from_timeline_clip(&clip(1, "ZED", 0, 0, 2000));
        let b = XSheetTrack::from_timeline_clip(&clip(2, "AMY", 0, 0, 2000));
        let sheet = build("s", 3000, 24, &[a.clone(), b]).unwrap();
        assert_eq!(sheet.speakers, ["AMY", "ZED"]);
        let mut c = a.clone();
        c.clip_id = 3;
        c.start_sample = 1000;
        c.visemes.clear();
        c.phonemes.clear();
        c.end_sample = 2500;
        assert_eq!(
            build("s", 3000, 24, &[a, c]),
            Err(XSheetError::OverlappingTracks {
                speaker: "ZED".into(),
                first: 1,
                second: 3
            })
        );
    }

    #[test]
    fn rejects_invalid_spans_and_text() {
        let base = XSheetTrack::from_timeline_clip(&clip(1, "A", 0, 0, 6000));
        let mut t = base.clone();
        t.visemes = vec![cue(100, 50, A)];
        assert!(build("s", 6000, 24, &[t]).is_err());
        let mut t = base.clone();
        t.visemes = vec![cue(0, 7000, A)];
        assert!(build("s", 8000, 24, &[t]).is_err());
        let t = base.clone().with_line("bad\u{7}text");
        assert!(build("s", 6000, 24, &[t]).is_err());
        let mut t = base;
        t.speaker = " ".into();
        assert!(build("s", 6000, 24, &[t]).is_err());
    }

    #[test]
    fn exports_are_deterministic_and_escaped() {
        let t = timeline(
            12_000,
            vec![clip(1, "EMMA", 0, 0, 9000), {
                let mut c = clip(2, "ALEX", 2000, 0, 6000);
                c.mouth.as_mut().unwrap().source = AlignmentSource::Estimated {
                    tool: "rhubarb".into(),
                    version: "1.13".into(),
                };
                c
            }],
        );
        let mut lines = BTreeMap::new();
        lines.insert(1, "Well, \"yes\"".to_string());
        let a = XSheet::from_audio_timeline(&t, 24, &lines).unwrap();
        let b = XSheet::from_audio_timeline(&t.clone(), 24, &lines.clone()).unwrap();
        assert_eq!(a.to_csv().as_bytes(), b.to_csv().as_bytes());
        assert_eq!(a.to_text().as_bytes(), b.to_text().as_bytes());
        let csv = a.to_csv();
        assert!(csv.contains("0,00:00:00:00,EMMA,\"Well, \"\"yes\"\"\",m,closed,A,manual\n"));
        assert!(csv.contains("2,00:00:00:02,ALEX,,m,closed,A,estimated:rhubarb/1.13\n"));
        // Two speakers per frame in sorted order.
        assert_eq!(csv.lines().count(), 1 + 12 * 2);
        let text = a.to_text();
        let header = text.lines().nth(1).unwrap();
        assert!(header.starts_with("frame  timecode     speaker  dialogue"));
        let widths: Vec<usize> = text
            .lines()
            .skip(1)
            .map(|l| l.find(':').unwrap_or(0))
            .collect();
        // Timecode column starts at the same character in every record.
        assert!(widths[1..].windows(2).all(|w| w[0] == w[1]));
        // Golden line guards accidental format drift.
        let expected_first = "    0  00:00:00:00  ALEX                            rest    X";
        assert_eq!(text.lines().nth(2), Some(expected_first));
    }
}
