//! Sample-frame based audio primitives shared by preview and export.
use serde::{Deserialize, Serialize};
use std::{fmt, path::Path};

#[derive(Debug)]
pub struct AudioError(pub String);
impl fmt::Display for AudioError { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.0) } }
impl std::error::Error for AudioError {}
impl From<hound::Error> for AudioError { fn from(e: hound::Error) -> Self { Self(e.to_string()) } }

/// Interleaved, normalized PCM. Rate conversion is deliberately explicit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioBuffer { pub sample_rate: u32, pub channels: u16, pub samples: Vec<f32> }
impl AudioBuffer {
    pub fn validate(&self) -> Result<(), AudioError> {
        if self.sample_rate == 0 || self.channels == 0 || self.samples.len() % self.channels as usize != 0 || self.samples.iter().any(|x| !x.is_finite()) {
            return Err(AudioError("invalid PCM rate, channels, frame count, or non-finite sample".into()));
        }
        Ok(())
    }
    pub fn frames(&self) -> usize { if self.channels == 0 { 0 } else { self.samples.len() / self.channels as usize } }
    pub fn duration_seconds(&self) -> f64 { if self.sample_rate == 0 { 0.0 } else { self.frames() as f64 / self.sample_rate as f64 } }
}

pub fn read_wav(path: impl AsRef<Path>) -> Result<AudioBuffer, AudioError> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    let samples = match (spec.sample_format, spec.bits_per_sample) {
        (hound::SampleFormat::Float, 32) => reader.samples::<f32>().collect::<Result<Vec<_>, _>>()?,
        (hound::SampleFormat::Int, bits @ 1..=32) => {
            let scale = 2_f64.powi(bits as i32 - 1);
            reader.samples::<i32>().map(|s| s.map(|v| (v as f64 / scale) as f32)).collect::<Result<Vec<_>, _>>()?
        }
        _ => return Err(AudioError("unsupported WAV encoding".into())),
    };
    let audio = AudioBuffer { sample_rate: spec.sample_rate, channels: spec.channels, samples };
    audio.validate()?;
    Ok(audio)
}

/// Writes PCM16, clipping only at the export boundary.
pub fn write_wav(path: impl AsRef<Path>, audio: &AudioBuffer) -> Result<(), AudioError> {
    audio.validate()?;
    let spec = hound::WavSpec { channels: audio.channels, sample_rate: audio.sample_rate, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for &sample in &audio.samples { writer.write_sample((sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16)?; }
    writer.finalize()?;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WaveformPeak { pub min: f32, pub max: f32 }
/// Downsamples all channels to min/max bins without losing short transients.
pub fn waveform_peaks(audio: &AudioBuffer, frames_per_bin: usize) -> Result<Vec<WaveformPeak>, AudioError> {
    audio.validate()?;
    if frames_per_bin == 0 { return Err(AudioError("waveform bin size must be positive".into())); }
    let bin = frames_per_bin.checked_mul(audio.channels as usize).ok_or_else(|| AudioError("waveform bin overflow".into()))?;
    Ok(audio.samples.chunks(bin).map(|samples| WaveformPeak {
        min: samples.iter().copied().fold(f32::INFINITY, f32::min),
        max: samples.iter().copied().fold(f32::NEG_INFINITY, f32::max),
    }).collect())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MixClip {
    pub audio: AudioBuffer,
    /// Timeline placement, expressed in output sample frames.
    pub start_frame: u64,
    pub trim_start_frame: u64,
    pub duration_frames: Option<u64>,
    pub gain: f32,
}
/// Same clock and placement rules for preview and exported audio. No implicit resampling.
pub fn mix_timeline(clips: &[MixClip], sample_rate: u32, channels: u16, total_frames: usize) -> Result<AudioBuffer, AudioError> {
    if sample_rate == 0 || channels == 0 { return Err(AudioError("invalid output format".into())); }
    let count = total_frames.checked_mul(channels as usize).ok_or_else(|| AudioError("mix length overflow".into()))?;
    let mut samples = Vec::new();
    samples.try_reserve_exact(count).map_err(|_| AudioError("mix allocation failed".into()))?;
    samples.resize(count, 0.0_f32);
    for clip in clips {
        clip.audio.validate()?;
        if clip.audio.sample_rate != sample_rate || clip.audio.channels != channels || !clip.gain.is_finite() {
            return Err(AudioError("clip format mismatch or invalid gain".into()));
        }
        let trim = usize::try_from(clip.trim_start_frame).map_err(|_| AudioError("trim exceeds platform range".into()))?;
        if trim > clip.audio.frames() { return Err(AudioError("trim exceeds source duration".into())); }
        let available = clip.audio.frames() - trim;
        let requested = clip.duration_frames.unwrap_or(available as u64).min(available as u64) as usize;
        if clip.start_frame >= total_frames as u64 { continue; }
        let start = clip.start_frame as usize;
        let length = requested.min(total_frames - start);
        for index in 0..length * channels as usize {
            let target = start * channels as usize + index;
            samples[target] += clip.audio.samples[trim * channels as usize + index] * clip.gain;
            if !samples[target].is_finite() { return Err(AudioError("mix overflow".into())); }
        }
    }
    Ok(AudioBuffer { sample_rate, channels, samples })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn audio(samples: Vec<f32>) -> AudioBuffer { AudioBuffer { sample_rate: 4, channels: 1, samples } }
    #[test] fn placement_trim_gain_overlap_and_end_are_exact() {
        let clip = MixClip { audio: audio(vec![0.1, 0.2, 0.3]), start_frame: 1, trim_start_frame: 1, duration_frames: Some(2), gain: 2.0 };
        let mixed = mix_timeline(&[clip.clone(), clip], 4, 1, 3).unwrap();
        assert_eq!(mixed.samples, vec![0.0, 0.8, 1.2]);
    }
    #[test] fn waveform_keeps_transients_and_tail() {
        assert_eq!(waveform_peaks(&audio(vec![-0.2, 1.0, 0.3]), 2).unwrap(), vec![WaveformPeak { min: -0.2, max: 1.0 }, WaveformPeak { min: 0.3, max: 0.3 }]);
    }
    #[test] fn malformed_audio_is_rejected() { assert!(audio(vec![f32::NAN]).validate().is_err()); assert!(waveform_peaks(&audio(vec![]), 0).is_err()); }
    #[test] fn incompatible_mix_is_rejected() {
        let clip = MixClip { audio: audio(vec![0.0]), start_frame: 0, trim_start_frame: 0, duration_frames: None, gain: 1.0 };
        assert!(mix_timeline(&[clip], 24_000, 1, 4).is_err());
    }
}
