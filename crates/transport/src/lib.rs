use std::sync::Arc;

use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

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

#[derive(Debug, Clone)]
pub struct Clip {
    pub id: ClipId,
    pub name: String,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipId(pub u64);

#[derive(Debug, Clone)]
pub struct Track {
    pub id: TrackId,
    pub name: String,
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

/// Resample an audio buffer to a target sample rate
pub fn resample_audio(
    buffer: &AudioBuffer,
    target_sample_rate: u32,
) -> anyhow::Result<AudioBuffer> {
    // If already at target rate, return a clone
    if buffer.sample_rate == target_sample_rate {
        return Ok(buffer.clone());
    }

    let channels = buffer.channels as usize;
    let input_frames = buffer.samples.len() / channels;

    // Calculate output length
    let resample_ratio = target_sample_rate as f64 / buffer.sample_rate as f64;
    let output_frames = (input_frames as f64 * resample_ratio).ceil() as usize;

    // Convert interleaved samples to per-channel format for rubato
    let mut input_channels = vec![Vec::with_capacity(input_frames); channels];
    for frame_idx in 0..input_frames {
        for ch in 0..channels {
            input_channels[ch].push(buffer.samples[frame_idx * channels + ch]);
        }
    }

    // Create resampler with high quality settings
    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    let mut resampler =
        SincFixedIn::<f32>::new(resample_ratio, 2.0, params, input_frames, channels)?;

    // Process resampling
    let output_channels = resampler.process(&input_channels, None)?;

    // Convert back to interleaved format
    let mut output_samples = Vec::with_capacity(output_frames * channels);
    for frame_idx in 0..output_channels[0].len() {
        for ch in 0..channels {
            output_samples.push(output_channels[ch][frame_idx]);
        }
    }

    Ok(AudioBuffer {
        samples: output_samples,
        sample_rate: target_sample_rate,
        channels: buffer.channels,
    })
}
