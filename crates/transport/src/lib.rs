use std::sync::Arc;

/// Pulses Per Quarter Note - defines timing resolution
pub const PPQN: u64 = 960;

#[derive(Debug, Clone)]
pub struct AudioBuffer {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Debug)]
pub enum Command {
    Play,
    Pause,
    Seek { tick: u64 },
}

#[derive(Debug)]
pub enum Status {
    Position(u64),
}

#[derive(Debug, Clone)]
pub struct Clip {
    pub id: ClipId,
    pub start: u64, // tick position on timeline
    pub audio: Arc<AudioBuffer>,
}

impl Clip {
    /// Calculate the duration of this clip in ticks based on audio buffer length
    pub fn duration_ticks(&self, tempo: f64) -> u64 {
        let samples_per_channel = self.audio.samples.len() / self.audio.channels as usize;
        samples_to_ticks(samples_per_channel as f64, tempo, self.audio.sample_rate)
    }
}

#[derive(Debug, Clone)]
pub struct ClipId(pub u64);

#[derive(Debug, Clone)]
pub struct Track {
    pub id: TrackId,
    pub clips: Vec<Clip>,
}

#[derive(Debug, Clone)]
pub struct TrackId(pub u64);

/// Convert samples to ticks based on tempo and sample rate
pub fn samples_to_ticks(samples: f64, tempo: f64, sample_rate: u32) -> u64 {
    let seconds_per_beat = 60.0 / tempo;
    let seconds_per_tick = seconds_per_beat / PPQN as f64;
    let seconds = samples / sample_rate as f64;
    (seconds / seconds_per_tick) as u64
}
