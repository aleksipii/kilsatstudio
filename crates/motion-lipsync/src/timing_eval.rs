//! Offline consistency checks between mouth tracks and an audio energy envelope.
//!
//! Everything here is a deterministic function of PCM samples and cue lists. The
//! energy envelope only says where the signal is louder than a threshold; it is not
//! a voice-activity model and does not know which sounds are speech. Rhubarb cues are
//! themselves an estimate, so agreement between a text estimate and Rhubarb is a
//! consistency check between two estimates, never a quality validation. See
//! docs/lipsync-timing-evaluation.md for the threshold rationale and real results.
use crate::rhubarb::MouthCue;
use serde::Serialize;

/// Envelope parameters. All lengths are samples at the clip's sample rate.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct EnvelopeConfig {
    /// RMS analysis window centred on each hop cell.
    pub window: usize,
    /// Cell length; every sample in a cell gets the cell's voiced/silent decision.
    pub hop: usize,
    /// Cells quieter than this (dBFS, full scale = 1.0) are always silent.
    pub absolute_floor_dbfs: f64,
    /// Cells more than this many dB below the loudest cell are silent.
    pub relative_db: f64,
    /// Silent gaps strictly shorter than this between two voiced runs are bridged
    /// (stop closures and short word joins are not pauses).
    pub bridge_gap: u64,
    /// Voiced runs strictly shorter than this after bridging are dropped (clicks).
    pub min_voiced: u64,
}

impl EnvelopeConfig {
    /// Documented default at 24 kHz: 20 ms window, 10 ms hop, threshold
    /// max(-50 dBFS, loudest cell - 35 dB), bridge gaps under 100 ms, drop runs under 30 ms.
    pub const DEFAULT_24K: EnvelopeConfig = EnvelopeConfig {
        window: 480,
        hop: 240,
        absolute_floor_dbfs: -50.0,
        relative_db: 35.0,
        bridge_gap: 2400,
        min_voiced: 720,
    };
}

/// Half-open sample interval `[start, end)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Interval {
    pub start: u64,
    pub end: u64,
}

impl Interval {
    pub fn len(&self) -> u64 {
        self.end - self.start
    }
    pub fn is_empty(&self) -> bool {
        self.end <= self.start
    }
    fn overlap(&self, other: &Interval) -> u64 {
        self.end
            .min(other.end)
            .saturating_sub(self.start.max(other.start))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Envelope {
    pub frames: u64,
    pub config: EnvelopeConfig,
    /// Loudest cell RMS in dBFS (`-inf` is reported as `None`).
    pub peak_dbfs: Option<f64>,
    /// Threshold actually applied, dBFS.
    pub threshold_dbfs: f64,
    /// Voiced (above-threshold) intervals after bridging and click removal, ordered.
    pub voiced: Vec<Interval>,
}

impl Envelope {
    /// First voiced sample to the end of the last voiced run; `None` when all silent.
    pub fn speech_span(&self) -> Option<Interval> {
        Some(Interval {
            start: self.voiced.first()?.start,
            end: self.voiced.last()?.end,
        })
    }
    /// Samples before the first voiced run (the whole clip when silent).
    pub fn leading_silence(&self) -> u64 {
        self.speech_span().map_or(self.frames, |s| s.start)
    }
    /// Samples after the last voiced run (the whole clip when silent).
    pub fn trailing_silence(&self) -> u64 {
        self.speech_span()
            .map_or(self.frames, |s| self.frames - s.end)
    }
    pub fn is_voiced(&self, sample: u64) -> bool {
        let i = self.voiced.partition_point(|v| v.end <= sample);
        self.voiced.get(i).is_some_and(|v| v.start <= sample)
    }
}

fn dbfs(rms: f64) -> f64 {
    if rms > 0.0 {
        20.0 * rms.log10()
    } else {
        f64::NEG_INFINITY
    }
}

/// Merges gaps shorter than `bridge` between consecutive intervals, then drops
/// intervals shorter than `min_len`.
fn bridge_and_filter(intervals: &[Interval], bridge: u64, min_len: u64) -> Vec<Interval> {
    let mut out: Vec<Interval> = Vec::with_capacity(intervals.len());
    for iv in intervals.iter().filter(|iv| !iv.is_empty()) {
        match out.last_mut() {
            Some(last) if iv.start.saturating_sub(last.end) < bridge => last.end = iv.end,
            _ => out.push(*iv),
        }
    }
    out.retain(|iv| iv.len() >= min_len);
    out
}

/// RMS energy envelope of mono samples. Non-finite samples count as silence.
/// Deterministic; O(n) with a running sum over the window.
pub fn energy_envelope(samples: &[f32], config: EnvelopeConfig) -> Envelope {
    let frames = samples.len() as u64;
    let hop = config.hop.max(1);
    let window = config.window.max(hop);
    let cells = samples.len().div_ceil(hop);
    // Prefix sums of squares in f64 keep the result order-independent and exact enough.
    let mut prefix = Vec::with_capacity(samples.len() + 1);
    prefix.push(0.0f64);
    for s in samples {
        let v = if s.is_finite() { f64::from(*s) } else { 0.0 };
        prefix.push(prefix.last().copied().unwrap_or(0.0) + v * v);
    }
    let levels: Vec<f64> = (0..cells)
        .map(|c| {
            let centre = c * hop + hop / 2;
            let start = centre.saturating_sub(window / 2);
            let end = (start + window).min(samples.len());
            let start = end.saturating_sub(window).min(start);
            let n = (end - start).max(1) as f64;
            dbfs(((prefix[end] - prefix[start]) / n).max(0.0).sqrt())
        })
        .collect();
    let peak = levels.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let threshold = config.absolute_floor_dbfs.max(peak - config.relative_db);
    let mut raw = Vec::new();
    for (c, level) in levels.iter().enumerate() {
        if *level >= threshold && level.is_finite() {
            let start = (c * hop) as u64;
            let end = ((c + 1) * hop).min(samples.len()) as u64;
            match raw.last_mut() {
                Some(Interval { end: e, .. }) if *e == start => *e = end,
                _ => raw.push(Interval { start, end }),
            }
        }
    }
    Envelope {
        frames,
        config,
        peak_dbfs: peak.is_finite().then_some(peak),
        threshold_dbfs: threshold,
        voiced: bridge_and_filter(&raw, config.bridge_gap, config.min_voiced),
    }
}

/// Common mouth set used to compare tracks from different estimators: the nine
/// Rhubarb shapes with `X` (rest) and `A` (closed lips) merged into one closed class
/// `X`. Unknown characters are treated as closed.
pub fn common_shape(shape: char) -> char {
    match shape {
        'B' | 'C' | 'D' | 'E' | 'F' | 'G' | 'H' => shape,
        _ => 'X',
    }
}

/// True when the common shape shows an open (non-closed) mouth.
pub fn is_open(shape: char) -> bool {
    common_shape(shape) != 'X'
}

/// Contiguous runs covering `[0, frames)` in the common set; uncovered samples are `X`.
/// Cues past `frames` are clipped.
pub fn common_runs(cues: &[MouthCue], frames: u64) -> Vec<(Interval, char)> {
    let mut out: Vec<(Interval, char)> = Vec::new();
    let mut push = |start: u64, end: u64, shape: char| {
        if end <= start {
            return;
        }
        match out.last_mut() {
            Some((iv, s)) if *s == shape && iv.end == start => iv.end = end,
            _ => out.push((Interval { start, end }, shape)),
        }
    };
    let mut cursor = 0;
    for c in cues {
        let start = c.start_sample.max(cursor).min(frames);
        let end = c.end_sample.min(frames);
        push(cursor, start, 'X');
        push(start, end.max(start), common_shape(c.shape));
        cursor = cursor.max(end);
    }
    push(cursor, frames, 'X');
    out
}

/// Sample-level agreement of two tracks over `[0, frames)` in the common set. Same
/// definitions as `services/kokoro-runtime/viseme_acceptance.py` (`shape_agreement`,
/// `active_shape_agreement`), applied after the X/A merge, plus open/closed agreement.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TrackAgreement {
    pub frames: u64,
    pub matched_samples: u64,
    pub shape_agreement: f64,
    /// Matching open samples over samples where either track is open; `None` when
    /// both are closed throughout.
    pub active_shape_agreement: Option<f64>,
    /// Samples where both are open or both closed, over all samples.
    pub open_closed_agreement: f64,
}

pub fn compare_tracks(a: &[MouthCue], b: &[MouthCue], frames: u64) -> TrackAgreement {
    let (ra, rb) = (common_runs(a, frames), common_runs(b, frames));
    let (mut i, mut j, mut pos) = (0, 0, 0);
    let (mut matched, mut both_closed, mut open_same, mut either_open) = (0, 0, 0, 0);
    while pos < frames && i < ra.len() && j < rb.len() {
        let end = ra[i].0.end.min(rb[j].0.end);
        let n = end - pos;
        let (sa, sb) = (ra[i].1, rb[j].1);
        if sa == sb {
            matched += n;
        }
        match (sa == 'X', sb == 'X') {
            (true, true) => both_closed += n,
            (false, false) => open_same += n,
            _ => {}
        }
        if sa != 'X' || sb != 'X' {
            either_open += n;
        }
        pos = end;
        if ra[i].0.end == end {
            i += 1;
        }
        if rb[j].0.end == end {
            j += 1;
        }
    }
    let frames_f = frames.max(1) as f64;
    TrackAgreement {
        frames,
        matched_samples: matched,
        shape_agreement: matched as f64 / frames_f,
        active_shape_agreement: (either_open > 0)
            .then(|| (matched - both_closed) as f64 / either_open as f64),
        open_closed_agreement: (both_closed + open_same) as f64 / frames_f,
    }
}

/// Open-mouth intervals of a track, with closed gaps shorter than `bridge` filled
/// (a lip closure for /m b p/ inside a word is not a mouth-closed pause).
pub fn open_intervals(cues: &[MouthCue], frames: u64, bridge: u64) -> Vec<Interval> {
    let open: Vec<Interval> = common_runs(cues, frames)
        .into_iter()
        .filter(|(_, s)| *s != 'X')
        .map(|(iv, _)| iv)
        .collect();
    bridge_and_filter(&open, bridge, 1)
}

/// Count and total length of disagreement runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct RunStats {
    /// Maximal runs of any length.
    pub runs: usize,
    /// Runs at least `frame_samples` long (one animation frame).
    pub runs_at_least_one_frame: usize,
    pub samples: u64,
}

/// Mouth track versus envelope. Signed errors are mouth minus audio in milliseconds:
/// negative onset error means the mouth opens before the sound.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EnvelopeConsistency {
    pub open_during_silence: RunStats,
    pub closed_during_voiced: RunStats,
    /// First open sample minus first voiced sample.
    pub utterance_onset_error_ms: Option<f64>,
    /// End of last open interval minus end of last voiced run.
    pub utterance_offset_error_ms: Option<f64>,
    pub voiced_segments: usize,
    /// Voiced segments overlapping at least one open interval.
    pub matched_segments: usize,
    pub segment_onset_mean_abs_ms: Option<f64>,
    pub segment_onset_mean_signed_ms: Option<f64>,
    pub segment_offset_mean_abs_ms: Option<f64>,
    pub segment_offset_mean_signed_ms: Option<f64>,
}

fn ms(samples: f64, rate: u32) -> f64 {
    samples * 1000.0 / f64::from(rate)
}

fn mismatch_runs(
    frames: u64,
    frame_samples: u64,
    a: &[Interval],
    b: &[Interval],
    want_a: bool,
) -> RunStats {
    // Boundaries of both interval sets partition [0, frames) into constant pieces.
    let mut cuts: Vec<u64> = vec![0, frames];
    cuts.extend(a.iter().chain(b).flat_map(|iv| [iv.start, iv.end]));
    cuts.retain(|c| *c <= frames);
    cuts.sort_unstable();
    cuts.dedup();
    let inside = |set: &[Interval], s: u64| {
        let i = set.partition_point(|v| v.end <= s);
        set.get(i).is_some_and(|v| v.start <= s)
    };
    let mut stats = RunStats::default();
    let mut run = 0u64;
    let close = |run: &mut u64, stats: &mut RunStats| {
        if *run > 0 {
            stats.runs += 1;
            if *run >= frame_samples {
                stats.runs_at_least_one_frame += 1;
            }
            stats.samples += *run;
            *run = 0;
        }
    };
    for w in cuts.windows(2) {
        let (s, e) = (w[0], w[1]);
        // Mismatch: in `a` and not in `b` (want_a), i.e. open & silent or voiced & closed.
        if inside(a, s) == want_a && !inside(b, s) {
            run += e - s;
        } else {
            close(&mut run, &mut stats);
        }
    }
    close(&mut run, &mut stats);
    stats
}

/// Compares a mouth track with an envelope at `rate` Hz. `frame_samples` is one
/// animation frame (1000 at 24 kHz / 24 fps). Open intervals use the envelope's
/// `bridge_gap` so both sides are smoothed the same way.
pub fn envelope_consistency(
    cues: &[MouthCue],
    envelope: &Envelope,
    rate: u32,
    frame_samples: u64,
) -> EnvelopeConsistency {
    let frames = envelope.frames;
    let open = open_intervals(cues, frames, envelope.config.bridge_gap);
    let voiced = &envelope.voiced;
    let mean = |v: &[f64]| (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64);
    let (mut on, mut off) = (Vec::new(), Vec::new());
    for seg in voiced {
        let best = open
            .iter()
            .filter(|o| o.overlap(seg) > 0)
            .max_by_key(|o| (o.overlap(seg), std::cmp::Reverse(o.start)));
        if let Some(o) = best {
            on.push(ms(o.start as f64 - seg.start as f64, rate));
            off.push(ms(o.end as f64 - seg.end as f64, rate));
        }
    }
    let abs = |v: &[f64]| v.iter().map(|x| x.abs()).collect::<Vec<_>>();
    EnvelopeConsistency {
        open_during_silence: mismatch_runs(frames, frame_samples, &open, voiced, true),
        closed_during_voiced: mismatch_runs(frames, frame_samples, voiced, &open, true),
        utterance_onset_error_ms: match (open.first(), voiced.first()) {
            (Some(o), Some(v)) => Some(ms(o.start as f64 - v.start as f64, rate)),
            _ => None,
        },
        utterance_offset_error_ms: match (open.last(), voiced.last()) {
            (Some(o), Some(v)) => Some(ms(o.end as f64 - v.end as f64, rate)),
            _ => None,
        },
        voiced_segments: voiced.len(),
        matched_segments: on.len(),
        segment_onset_mean_abs_ms: mean(&abs(&on)),
        segment_onset_mean_signed_ms: mean(&on),
        segment_offset_mean_abs_ms: mean(&abs(&off)),
        segment_offset_mean_signed_ms: mean(&off),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 24_000;
    const CFG: EnvelopeConfig = EnvelopeConfig::DEFAULT_24K;

    fn tone(n: usize, amp: f32) -> Vec<f32> {
        (0..n)
            .map(|i| amp * (i as f32 * 2.0 * std::f32::consts::PI * 200.0 / 24_000.0).sin())
            .collect()
    }
    /// Concatenate (length, amplitude) pieces; amplitude 0 is digital silence.
    fn signal(pieces: &[(usize, f32)]) -> Vec<f32> {
        pieces.iter().flat_map(|(n, a)| tone(*n, *a)).collect()
    }
    fn cue(start: u64, end: u64, shape: char) -> MouthCue {
        MouthCue {
            start_sample: start,
            end_sample: end,
            shape,
        }
    }

    #[test]
    fn envelope_finds_leading_and_trailing_silence_on_hop_grid() {
        // 0.25 s silence, 1 s tone, 0.5 s silence: boundaries are multiples of the hop.
        let s = signal(&[(6000, 0.0), (24000, 0.3), (12000, 0.0)]);
        let e = energy_envelope(&s, CFG);
        assert_eq!(e.frames, 42000);
        assert_eq!(e.voiced.len(), 1);
        let span = e.speech_span().unwrap();
        // The centred 20 ms window reaches 10 ms into silence: at most one hop of spread.
        assert!(span.start.abs_diff(6000) <= 240, "{span:?}");
        assert!(span.end.abs_diff(30000) <= 240, "{span:?}");
        assert_eq!(e.leading_silence(), span.start);
        assert_eq!(e.trailing_silence(), 42000 - span.end);
        assert!((e.peak_dbfs.unwrap() - 20.0 * (0.3f64 / 2f64.sqrt()).log10()).abs() < 0.1);
    }

    #[test]
    fn envelope_bridges_short_gaps_and_keeps_long_pauses() {
        // 50 ms gap is bridged (< 100 ms); 300 ms gap is a pause.
        let s = signal(&[
            (4800, 0.3),
            (1200, 0.0),
            (4800, 0.3),
            (7200, 0.0),
            (4800, 0.3),
        ]);
        let e = energy_envelope(&s, CFG);
        assert_eq!(e.voiced.len(), 2, "{:?}", e.voiced);
        assert!(e.voiced[0].end.abs_diff(10800) <= 240);
        assert!(e.voiced[1].start.abs_diff(18000) <= 240);
    }

    #[test]
    fn envelope_threshold_is_relative_to_peak_and_floored() {
        // A tone 40 dB below the loud part is under peak - 35 dB: silent.
        let s = signal(&[(12000, 0.5), (12000, 0.005)]);
        let e = energy_envelope(&s, CFG);
        assert_eq!(e.voiced.len(), 1);
        assert!(e.voiced[0].end.abs_diff(12000) <= 240);
        // A lone quiet tone is below the -50 dBFS floor even though it is the peak.
        let quiet = energy_envelope(&tone(24000, 0.002), CFG);
        assert!(quiet.voiced.is_empty());
        assert_eq!(quiet.speech_span(), None);
        assert_eq!(quiet.leading_silence(), 24000);
        let empty = energy_envelope(&[], CFG);
        assert_eq!(empty.peak_dbfs, None);
        assert!(empty.voiced.is_empty());
    }

    #[test]
    fn envelope_drops_clicks_and_is_deterministic() {
        let mut s = signal(&[(24000, 0.0)]);
        for x in &mut s[12000..12100] {
            *x = 0.9; // ~4 ms click, shorter than 30 ms
        }
        s.extend(tone(12000, 0.3));
        let a = energy_envelope(&s, CFG);
        assert_eq!(a.voiced.len(), 1);
        assert!(a.voiced[0].start >= 24000 - 240);
        assert_eq!(a, energy_envelope(&s, CFG));
        // Non-finite samples are silence, never a panic.
        let nan = energy_envelope(&[f32::NAN; 4800], CFG);
        assert!(nan.voiced.is_empty());
    }

    #[test]
    fn common_set_merges_rest_and_closed() {
        assert_eq!(common_shape('A'), 'X');
        assert_eq!(common_shape('X'), 'X');
        assert_eq!(common_shape('D'), 'D');
        assert!(!is_open('A') && is_open('G'));
        let runs = common_runs(&[cue(10, 20, 'A'), cue(20, 30, 'X'), cue(30, 40, 'D')], 50);
        assert_eq!(
            runs,
            vec![
                (Interval { start: 0, end: 30 }, 'X'),
                (Interval { start: 30, end: 40 }, 'D'),
                (Interval { start: 40, end: 50 }, 'X'),
            ]
        );
    }

    #[test]
    fn agreement_has_known_values() {
        // a: D on [0,50), b: D on [25,75) over 100 samples.
        let a = [cue(0, 50, 'D')];
        let b = [cue(25, 75, 'D')];
        let r = compare_tracks(&a, &b, 100);
        // matched: D on [25,50) = 25, closed both on [75,100) = 25.
        assert_eq!(r.matched_samples, 50);
        assert!((r.shape_agreement - 0.5).abs() < 1e-12);
        assert!((r.active_shape_agreement.unwrap() - 25.0 / 75.0).abs() < 1e-12);
        assert!((r.open_closed_agreement - 0.5).abs() < 1e-12);
        // Different open shapes: open/closed agrees, shapes do not.
        let r = compare_tracks(&[cue(0, 100, 'C')], &[cue(0, 100, 'D')], 100);
        assert_eq!(r.shape_agreement, 0.0);
        assert_eq!(r.open_closed_agreement, 1.0);
        assert_eq!(r.active_shape_agreement, Some(0.0));
        // A vs X is agreement in the common set; both closed => active undefined.
        let r = compare_tracks(&[cue(0, 100, 'A')], &[], 100);
        assert_eq!(r.shape_agreement, 1.0);
        assert_eq!(r.active_shape_agreement, None);
    }

    #[test]
    fn open_intervals_bridge_short_lip_closures() {
        let cues = [
            cue(0, 1000, 'D'),
            cue(1000, 1500, 'A'),
            cue(1500, 3000, 'C'),
        ];
        assert_eq!(
            open_intervals(&cues, 10_000, 2400),
            vec![Interval {
                start: 0,
                end: 3000
            }]
        );
        assert_eq!(open_intervals(&cues, 10_000, 100).len(), 2);
    }

    fn envelope(frames: u64, voiced: Vec<Interval>) -> Envelope {
        Envelope {
            frames,
            config: CFG,
            peak_dbfs: Some(-10.0),
            threshold_dbfs: -45.0,
            voiced,
        }
    }

    #[test]
    fn consistency_reports_offsets_and_silence_mismatch() {
        // Voiced [12000, 36000) in a 48000-sample clip; mouth open [6000, 42000):
        // opens 250 ms early and closes 250 ms late.
        let e = envelope(
            48000,
            vec![Interval {
                start: 12000,
                end: 36000,
            }],
        );
        let c = envelope_consistency(&[cue(6000, 42000, 'D')], &e, RATE, 1000);
        assert_eq!(c.utterance_onset_error_ms, Some(-250.0));
        assert_eq!(c.utterance_offset_error_ms, Some(250.0));
        assert_eq!(c.open_during_silence.runs, 2);
        assert_eq!(c.open_during_silence.runs_at_least_one_frame, 2);
        assert_eq!(c.open_during_silence.samples, 12000);
        assert_eq!(c.closed_during_voiced, RunStats::default());
        assert_eq!((c.voiced_segments, c.matched_segments), (1, 1));
        assert_eq!(c.segment_onset_mean_signed_ms, Some(-250.0));
        assert_eq!(c.segment_offset_mean_abs_ms, Some(250.0));
    }

    #[test]
    fn consistency_counts_closed_during_voiced_and_short_runs() {
        // Two voiced segments; mouth covers only the first and part of the second.
        let e = envelope(
            48000,
            vec![
                Interval {
                    start: 0,
                    end: 10000,
                },
                Interval {
                    start: 20000,
                    end: 30000,
                },
            ],
        );
        let c = envelope_consistency(
            &[cue(0, 10000, 'C'), cue(20500, 30000, 'E')],
            &e,
            RATE,
            1000,
        );
        // [20000, 20500) closed while voiced: one run, shorter than one frame.
        assert_eq!(
            c.closed_during_voiced,
            RunStats {
                runs: 1,
                runs_at_least_one_frame: 0,
                samples: 500
            }
        );
        assert_eq!(c.open_during_silence.samples, 0);
        assert_eq!(c.matched_segments, 2);
        assert!((c.segment_onset_mean_signed_ms.unwrap() - 500.0 / 24.0 / 2.0).abs() < 1e-9);
        // Silent clip: no utterance errors, and every open sample is a mismatch.
        let silent = envelope(24000, vec![]);
        let c = envelope_consistency(&[cue(0, 24000, 'D')], &silent, RATE, 1000);
        assert_eq!(c.utterance_onset_error_ms, None);
        assert_eq!(c.open_during_silence.samples, 24000);
        assert_eq!(c.segment_onset_mean_abs_ms, None);
    }
}
