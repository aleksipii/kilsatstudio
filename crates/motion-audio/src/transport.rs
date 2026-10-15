//! Deterministic playback position driven by frames consumed by an audio sink.
//!
//! A frame contains one sample per channel. Call `advance_frames` only for frames
//! actually consumed, never for frames merely queued or for elapsed wall time.
use crate::AudioError;

#[derive(Clone, Debug)]
pub struct AudioTransport {
    sample_rate: u32,
    total_frames: u64,
    position_frame: u64,
    playing: bool,
}

impl AudioTransport {
    pub fn new(sample_rate: u32, total_frames: u64) -> Result<Self, AudioError> {
        if sample_rate == 0 {
            return Err(AudioError("transport sample rate must be positive".into()));
        }
        Ok(Self {
            sample_rate,
            total_frames,
            position_frame: 0,
            playing: false,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn total_frames(&self) -> u64 {
        self.total_frames
    }

    pub fn position_frame(&self) -> u64 {
        self.position_frame
    }

    pub fn position_seconds(&self) -> f64 {
        self.position_frame as f64 / self.sample_rate as f64
    }

    pub fn duration_seconds(&self) -> f64 {
        self.total_frames as f64 / self.sample_rate as f64
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn is_finished(&self) -> bool {
        self.position_frame == self.total_frames
    }

    /// Resumes at the current position; reaching the end requires an explicit seek or restart.
    pub fn play(&mut self) {
        self.playing = !self.is_finished();
    }

    pub fn pause(&mut self) {
        self.playing = false;
    }

    /// Starts from frame zero. An empty timeline remains stopped.
    pub fn restart(&mut self) {
        self.position_frame = 0;
        self.play();
    }

    /// Seeks to a frame, clamping at the end and preserving pause/play state
    /// except that seeking to the end always pauses.
    pub fn seek_frame(&mut self, frame: u64) {
        self.position_frame = frame.min(self.total_frames);
        if self.is_finished() {
            self.pause();
        }
    }

    /// Seeks to the frame containing this time (rounding down), clamped at the end.
    /// Invalid times leave the transport unchanged.
    pub fn seek_seconds(&mut self, seconds: f64) -> Result<(), AudioError> {
        if !seconds.is_finite() || seconds < 0.0 {
            return Err(AudioError(
                "transport seek time must be finite and nonnegative".into(),
            ));
        }
        // Rust's saturating float-to-integer conversion also handles multiplication
        // overflow for huge, but finite, input. Frame clamping remains exact.
        let frame = (seconds * self.sample_rate as f64).floor() as u64;
        self.seek_frame(frame);
        Ok(())
    }

    /// Advances by consumed frames and returns the count accepted before the end.
    /// Paused transports consume zero frames. Addition cannot overflow.
    pub fn advance_frames(&mut self, consumed_frames: u64) -> u64 {
        if !self.playing {
            return 0;
        }
        let advanced = consumed_frames.min(self.total_frames - self.position_frame);
        self.position_frame += advanced;
        if self.is_finished() {
            self.pause();
        }
        advanced
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_clock_pause_resume_and_end_are_exact() {
        let mut transport = AudioTransport::new(4, 8).unwrap();
        assert_eq!(transport.advance_frames(2), 0);
        transport.play();
        assert_eq!(transport.advance_frames(3), 3);
        assert_eq!(transport.position_frame(), 3);
        assert_eq!(transport.position_seconds(), 0.75);
        transport.pause();
        assert_eq!(transport.advance_frames(100), 0);
        transport.play();
        assert_eq!(transport.advance_frames(4), 4);
        assert!(!transport.is_finished());
        assert_eq!(transport.advance_frames(u64::MAX), 1);
        assert_eq!(transport.position_frame(), 8);
        assert!(transport.is_finished());
        assert!(!transport.is_playing());
        transport.play();
        assert!(!transport.is_playing());
        transport.restart();
        assert_eq!(transport.position_frame(), 0);
        assert!(transport.is_playing());
    }

    #[test]
    fn seeks_clamp_and_preserve_state_except_at_end() {
        let mut transport = AudioTransport::new(4, 8).unwrap();
        transport.seek_seconds(0.499).unwrap();
        assert_eq!(transport.position_frame(), 1);
        assert!(!transport.is_playing());
        transport.play();
        transport.seek_seconds(0.5).unwrap();
        assert_eq!(transport.position_frame(), 2);
        assert!(transport.is_playing());
        transport.seek_frame(u64::MAX);
        assert_eq!(transport.position_frame(), 8);
        assert!(!transport.is_playing());
        transport.seek_frame(1);
        assert!(!transport.is_playing());
        transport.seek_seconds(f64::MAX).unwrap();
        assert_eq!(transport.position_frame(), 8);
    }

    #[test]
    fn invalid_times_do_not_mutate_state() {
        let mut transport = AudioTransport::new(48_000, 100).unwrap();
        transport.play();
        transport.seek_frame(42);
        for time in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.1] {
            assert!(transport.seek_seconds(time).is_err());
            assert_eq!(transport.position_frame(), 42);
            assert!(transport.is_playing());
        }
        assert!(AudioTransport::new(0, 10).is_err());
    }

    #[test]
    fn empty_and_maximum_length_do_not_overflow() {
        let mut empty = AudioTransport::new(24_000, 0).unwrap();
        empty.restart();
        assert!(empty.is_finished());
        assert!(!empty.is_playing());
        assert_eq!(empty.advance_frames(1), 0);
        let mut huge = AudioTransport::new(1, u64::MAX).unwrap();
        huge.seek_frame(u64::MAX - 1);
        huge.play();
        assert_eq!(huge.advance_frames(u64::MAX), 1);
        assert_eq!(huge.position_frame(), u64::MAX);
    }
}
