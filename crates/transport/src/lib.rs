use std::sync::Arc;

/// Pulses Per Quarter Note - defines timing resolution
pub const PPQN: u64 = 960;

#[derive(Debug, Clone)]
pub struct AudioBuffer {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Debug, Clone)]
pub struct WaveformData {
    pub peaks: Vec<(f32, f32)>,
    pub samples_per_bucket: usize,
}

impl WaveformData {
    pub fn from_audio_buffer(buffer: &AudioBuffer, samples_per_bucket: usize) -> Self {
        let samples_per_channel = buffer.samples.len() / buffer.channels as usize;
        let num_buckets = (samples_per_channel + samples_per_bucket - 1) / samples_per_bucket;
        let mut peaks = Vec::with_capacity(num_buckets);

        for bucket_idx in 0..num_buckets {
            let start = bucket_idx * samples_per_bucket;
            let end = ((bucket_idx + 1) * samples_per_bucket).min(samples_per_channel);

            let mut min_val: f32 = 0.0;
            let mut max_val: f32 = 0.0;

            for sample_idx in start..end {
                let mut sum: f32 = 0.0;
                for ch in 0..buffer.channels as usize {
                    let idx = sample_idx * buffer.channels as usize + ch;
                    if idx < buffer.samples.len() {
                        sum += buffer.samples[idx];
                    }
                }
                let mono_sample = sum / buffer.channels as f32;
                min_val = min_val.min(mono_sample);
                max_val = max_val.max(mono_sample);
            }

            peaks.push((min_val, max_val));
        }

        Self {
            peaks,
            samples_per_bucket,
        }
    }
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
    pub waveform: Arc<WaveformData>,
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
