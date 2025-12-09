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

/// A segment of audio on the timeline with explicit start and end positions.
/// Segments are non-overlapping within a track - the Track enforces this invariant.
#[derive(Debug, Clone)]
pub struct Segment {
    pub start_tick: u64,
    pub end_tick: u64,
    pub audio: Arc<AudioBuffer>,
    pub waveform: Arc<WaveformData>,
    /// Offset into the audio in samples (for trimmed starts)
    pub audio_offset: u64,
    /// Display name for UI
    pub name: String,
}

impl Segment {
    /// Duration of this segment in ticks
    pub fn duration_ticks(&self) -> u64 {
        self.end_tick - self.start_tick
    }
}

#[derive(Debug, Clone)]
pub struct Track {
    pub id: TrackId,
    pub name: String,
    /// Segments are always sorted by start_tick and non-overlapping.
    /// Use insert_segment() to add segments - it enforces the invariant.
    segments: Vec<Segment>,
    pub volume: f32,
    pub enabled: bool,
}

impl Track {
    pub fn new(id: TrackId, name: String) -> Self {
        Self {
            id,
            name,
            segments: Vec::new(),
            volume: 1.0,
            enabled: true,
        }
    }

    /// Get read-only access to segments
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// Clear all segments
    pub fn clear_segments(&mut self) {
        self.segments.clear();
    }

    /// Insert a segment, trimming/splitting/removing any overlapping segments.
    /// The new segment takes priority - existing segments in its range are modified.
    pub fn insert_segment(&mut self, new_seg: Segment) {
        let new_start = new_seg.start_tick;
        let new_end = new_seg.end_tick;

        // Process existing segments
        let mut result: Vec<Segment> = Vec::new();

        for existing in self.segments.drain(..) {
            let ex_start = existing.start_tick;
            let ex_end = existing.end_tick;

            // Check for overlap
            if new_start < ex_end && ex_start < new_end {
                // They overlap - determine how to handle

                if new_start <= ex_start && new_end >= ex_end {
                    // New completely covers existing - drop it
                    continue;
                } else if new_start > ex_start && new_end < ex_end {
                    // New is in the middle - split existing into two parts

                    // Left part: from ex_start to new_start
                    let left = Segment {
                        start_tick: ex_start,
                        end_tick: new_start,
                        audio: existing.audio.clone(),
                        waveform: existing.waveform.clone(),
                        audio_offset: existing.audio_offset,
                        name: existing.name.clone(),
                    };
                    result.push(left);

                    // Right part: from new_end to ex_end
                    // Calculate the audio offset for the right part
                    let ticks_into_audio = new_end - ex_start;
                    let samples_per_tick = existing.audio.sample_rate as f64 / 960.0 * 0.5; // At 120 BPM
                    // More accurate: we need tempo, but for now approximate
                    // Actually, store audio_offset in samples, so we need to convert ticks to samples
                    // This is tricky without tempo - let's use a simpler approach
                    let right_offset = existing.audio_offset + ticks_to_samples_approx(ticks_into_audio, existing.audio.sample_rate);

                    let right = Segment {
                        start_tick: new_end,
                        end_tick: ex_end,
                        audio: existing.audio,
                        waveform: existing.waveform,
                        audio_offset: right_offset,
                        name: existing.name,
                    };
                    result.push(right);
                } else if new_start <= ex_start {
                    // New covers the start - trim existing's start
                    let trim_ticks = new_end - ex_start;
                    let trim_samples = ticks_to_samples_approx(trim_ticks, existing.audio.sample_rate);

                    let trimmed = Segment {
                        start_tick: new_end,
                        end_tick: ex_end,
                        audio: existing.audio,
                        waveform: existing.waveform,
                        audio_offset: existing.audio_offset + trim_samples,
                        name: existing.name,
                    };

                    if trimmed.start_tick < trimmed.end_tick {
                        result.push(trimmed);
                    }
                } else {
                    // New covers the end - trim existing's end
                    let trimmed = Segment {
                        start_tick: ex_start,
                        end_tick: new_start,
                        audio: existing.audio,
                        waveform: existing.waveform,
                        audio_offset: existing.audio_offset,
                        name: existing.name,
                    };

                    if trimmed.start_tick < trimmed.end_tick {
                        result.push(trimmed);
                    }
                }
            } else {
                // No overlap - keep as is
                result.push(existing);
            }
        }

        // Add the new segment
        result.push(new_seg);

        // Sort by start_tick
        result.sort_by_key(|s| s.start_tick);

        self.segments = result;
    }

    /// Build from a list of segments, inserting each one (resolving overlaps)
    pub fn from_segments(id: TrackId, name: String, segments: Vec<Segment>) -> Self {
        let mut track = Self::new(id, name);
        for seg in segments {
            track.insert_segment(seg);
        }
        track
    }
}

/// Approximate tick to sample conversion (assumes 120 BPM)
/// For more accurate conversion, use the tempo-aware version in daw_render
fn ticks_to_samples_approx(ticks: u64, sample_rate: u32) -> u64 {
    // At 120 BPM: 0.5 seconds per beat, PPQN=960 ticks per beat
    // seconds_per_tick = 0.5 / 960
    let seconds_per_tick = 0.5 / PPQN as f64;
    let seconds = ticks as f64 * seconds_per_tick;
    (seconds * sample_rate as f64) as u64
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
