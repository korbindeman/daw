use std::sync::Arc;

use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

/// Pulses Per Quarter Note - defines timing resolution
pub const PPQN: u64 = 960;

/// Legacy audio buffer type - use AudioArc for new code
#[derive(Debug, Clone)]
pub struct AudioBuffer {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

/// Shared, immutable audio sample data inspired by imgref.
///
/// `AudioArc` provides cheap cloning through reference counting while keeping
/// the sample data immutable and shareable. Unlike `Arc<AudioBuffer>`, this type
/// stores the sample data in an `Arc<[f32]>`, making the entire structure small
/// and allowing multiple instances to share the same underlying audio data without
/// wrapping the metadata in the Arc.
///
/// # Memory Layout
///
/// ```text
/// AudioArc (24 bytes on stack)
/// ├─ samples: Arc<[f32]> (16 bytes) ────> Heap: [f32; N]
/// ├─ sample_rate: u32 (4 bytes)
/// └─ channels: u16 (2 bytes)
/// ```
///
/// Cloning an `AudioArc` only increments the reference count, making it very cheap.
///
/// # Examples
///
/// ```
/// use daw_transport::AudioArc;
///
/// // Create from owned samples
/// let samples = vec![0.0, 0.5, 1.0, 0.5];
/// let audio = AudioArc::new(samples, 44100, 2);
///
/// // Clone is cheap - just bumps refcount
/// let audio2 = audio.clone();
/// assert_eq!(audio.frames(), 2);
/// assert_eq!(audio2.frames(), 2);
///
/// // Access samples
/// assert_eq!(audio.samples()[0], 0.0);
/// ```
#[derive(Clone)]
pub struct AudioArc {
    /// Raw interleaved samples stored in a reference-counted slice.
    /// This is the actual audio data that can be shared between multiple AudioArc instances.
    samples: Arc<[f32]>,
    /// Sample rate in Hz (e.g., 44100, 48000)
    sample_rate: u32,
    /// Number of interleaved channels (e.g., 1 for mono, 2 for stereo)
    channels: u16,
}

impl AudioArc {
    /// Create a new `AudioArc` from owned sample data.
    ///
    /// # Arguments
    ///
    /// * `samples` - Interleaved audio samples. For stereo, the format is [L, R, L, R, ...].
    /// * `sample_rate` - Sample rate in Hz (e.g., 44100, 48000)
    /// * `channels` - Number of channels (e.g., 1 for mono, 2 for stereo)
    ///
    /// # Panics
    ///
    /// Panics if `channels` is 0 or if `samples.len()` is not divisible by `channels`.
    ///
    /// # Examples
    ///
    /// ```
    /// use daw_transport::AudioArc;
    ///
    /// // Stereo audio with 2 frames
    /// let samples = vec![0.0, 0.1, 0.2, 0.3]; // [L1, R1, L2, R2]
    /// let audio = AudioArc::new(samples, 44100, 2);
    /// assert_eq!(audio.frames(), 2);
    /// ```
    pub fn new(samples: Vec<f32>, sample_rate: u32, channels: u16) -> Self {
        assert!(channels > 0, "channels must be greater than 0");
        assert_eq!(
            samples.len() % channels as usize,
            0,
            "samples.len() must be divisible by channels"
        );
        Self {
            samples: Arc::from(samples),
            sample_rate,
            channels,
        }
    }

    /// Create an `AudioArc` from an existing `Arc<[f32]>`.
    ///
    /// This is useful when you already have sample data in an Arc and want to avoid
    /// an extra allocation.
    ///
    /// # Panics
    ///
    /// Panics if `channels` is 0 or if `samples.len()` is not divisible by `channels`.
    pub fn from_arc(samples: Arc<[f32]>, sample_rate: u32, channels: u16) -> Self {
        assert!(channels > 0, "channels must be greater than 0");
        assert_eq!(
            samples.len() % channels as usize,
            0,
            "samples.len() must be divisible by channels"
        );
        Self {
            samples,
            sample_rate,
            channels,
        }
    }

    /// Get a slice of all interleaved samples.
    ///
    /// For stereo audio, the format is [L, R, L, R, ...].
    #[inline]
    pub fn samples(&self) -> &[f32] {
        &self.samples
    }

    /// Get a reference to the inner `Arc<[f32]>` for advanced use cases.
    ///
    /// This allows access to the underlying Arc for checking reference counts
    /// or other Arc-specific operations.
    pub fn samples_arc(&self) -> &Arc<[f32]> {
        &self.samples
    }

    /// Get the sample rate in Hz.
    #[inline]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Get the number of channels.
    #[inline]
    pub fn channels(&self) -> u16 {
        self.channels
    }

    /// Get the number of frames (samples per channel).
    ///
    /// For stereo with 4 samples, this returns 2 frames.
    #[inline]
    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels as usize
    }

    /// Get the total number of samples (frames * channels).
    #[inline]
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Check if the audio buffer is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Get the duration in seconds.
    pub fn duration_secs(&self) -> f64 {
        self.frames() as f64 / self.sample_rate as f64
    }

    /// Get an iterator over a specific channel's samples.
    ///
    /// # Panics
    ///
    /// Panics if `channel` is >= `self.channels()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use daw_transport::AudioArc;
    ///
    /// let samples = vec![0.0, 1.0, 0.5, 1.5]; // [L1, R1, L2, R2]
    /// let audio = AudioArc::new(samples, 44100, 2);
    ///
    /// let left: Vec<f32> = audio.channel(0).collect();
    /// assert_eq!(left, vec![0.0, 0.5]);
    ///
    /// let right: Vec<f32> = audio.channel(1).collect();
    /// assert_eq!(right, vec![1.0, 1.5]);
    /// ```
    pub fn channel(&self, channel: usize) -> impl Iterator<Item = f32> + '_ {
        assert!(
            channel < self.channels as usize,
            "channel index out of bounds"
        );
        let channels = self.channels as usize;
        (0..self.frames()).map(move |frame| self.samples[frame * channels + channel])
    }

    /// Resample this audio to a target sample rate.
    ///
    /// If the audio is already at the target rate, returns a clone (cheap refcount bump).
    /// Otherwise, performs high-quality sinc interpolation resampling.
    ///
    /// # Errors
    ///
    /// Returns an error if resampling fails (e.g., invalid parameters).
    ///
    /// # Examples
    ///
    /// ```
    /// use daw_transport::AudioArc;
    ///
    /// let audio = AudioArc::new(vec![0.0; 44100], 44100, 1);
    /// let resampled = audio.resample(48000).unwrap();
    /// assert_eq!(resampled.sample_rate(), 48000);
    /// ```
    pub fn resample(&self, target_sample_rate: u32) -> anyhow::Result<Self> {
        // If already at target rate, return cheap clone
        if self.sample_rate == target_sample_rate {
            return Ok(self.clone());
        }

        resample_audio_arc(self, target_sample_rate)
    }

    /// Convert to the legacy `AudioBuffer` format.
    ///
    /// This creates a new owned copy of the sample data. Use only when necessary
    /// for compatibility with old APIs.
    pub fn to_audio_buffer(&self) -> AudioBuffer {
        AudioBuffer {
            samples: self.samples.to_vec(),
            sample_rate: self.sample_rate,
            channels: self.channels,
        }
    }

    /// Create an `AudioArc` from a legacy `AudioBuffer`.
    ///
    /// This takes ownership of the buffer's sample data.
    pub fn from_audio_buffer(buffer: AudioBuffer) -> Self {
        Self::new(buffer.samples, buffer.sample_rate, buffer.channels)
    }
}

impl std::fmt::Debug for AudioArc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioArc")
            .field("frames", &self.frames())
            .field("sample_rate", &self.sample_rate)
            .field("channels", &self.channels)
            .field("duration_secs", &self.duration_secs())
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct WaveformData {
    pub peaks: Vec<(f32, f32)>,
    pub samples_per_bucket: usize,
}

impl WaveformData {
    /// Generate waveform data from an `AudioArc`.
    ///
    /// This computes min/max peaks for visualization, downsampling the audio into
    /// buckets of `samples_per_bucket` frames each. The resulting peaks represent
    /// the mix-down to mono of all channels.
    ///
    /// # Arguments
    ///
    /// * `audio` - The audio to generate waveform data from
    /// * `samples_per_bucket` - Number of frames per bucket (e.g., 512)
    ///
    /// # Examples
    ///
    /// ```
    /// use daw_transport::{AudioArc, WaveformData};
    ///
    /// let audio = AudioArc::new(vec![0.0; 44100 * 2], 44100, 2);
    /// let waveform = WaveformData::from_audio_arc(&audio, 512);
    /// ```
    pub fn from_audio_arc(audio: &AudioArc, samples_per_bucket: usize) -> Self {
        let frames = audio.frames();
        let num_buckets = (frames + samples_per_bucket - 1) / samples_per_bucket;
        let mut peaks = Vec::with_capacity(num_buckets);
        let channels = audio.channels() as usize;
        let samples = audio.samples();

        for bucket_idx in 0..num_buckets {
            let start = bucket_idx * samples_per_bucket;
            let end = ((bucket_idx + 1) * samples_per_bucket).min(frames);

            let mut min_val: f32 = 0.0;
            let mut max_val: f32 = 0.0;

            for frame_idx in start..end {
                // Mix down to mono
                let mut sum: f32 = 0.0;
                for ch in 0..channels {
                    let idx = frame_idx * channels + ch;
                    if idx < samples.len() {
                        sum += samples[idx];
                    }
                }
                let mono_sample = sum / channels as f32;
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

    /// Legacy method for generating waveform data from AudioBuffer
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

/// A clip of audio on the timeline with explicit start and end positions.
/// Clips are non-overlapping within a track - the Track enforces this invariant.
#[derive(Debug, Clone)]
pub struct Clip {
    pub start_tick: u64,
    pub end_tick: u64,
    pub audio: AudioArc,
    pub waveform: Arc<WaveformData>,
    /// Offset into the audio in samples (for trimmed starts)
    pub audio_offset: u64,
    /// Display name for UI
    pub name: String,
}

impl Clip {
    /// Duration of this clip in ticks
    pub fn duration_ticks(&self) -> u64 {
        self.end_tick - self.start_tick
    }
}

#[derive(Debug, Clone)]
pub struct Track {
    pub id: TrackId,
    pub name: String,
    /// Clips are always sorted by start_tick and non-overlapping.
    /// Use insert_clip() to add clips - it enforces the invariant.
    clips: Vec<Clip>,
    pub volume: f32,
    pub pan: f32,
    pub enabled: bool,
    pub solo: bool,
}

impl Track {
    pub fn new(id: TrackId, name: String) -> Self {
        Self {
            id,
            name,
            clips: Vec::new(),
            volume: 1.0,
            pan: 0.0,
            enabled: true,
            solo: false,
        }
    }

    /// Get read-only access to clips
    pub fn clips(&self) -> &[Clip] {
        &self.clips
    }

    /// Clear all clips
    pub fn clear_clips(&mut self) {
        self.clips.clear();
    }

    /// Insert a clip, trimming/splitting/removing any overlapping clips.
    /// The new clip takes priority - existing clips in its range are modified.
    pub fn insert_clip(&mut self, new_clip: Clip) {
        let new_start = new_clip.start_tick;
        let new_end = new_clip.end_tick;

        // Process existing clips
        let mut result: Vec<Clip> = Vec::new();

        for existing in self.clips.drain(..) {
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
                    let left = Clip {
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
                    let _samples_per_tick = existing.audio.sample_rate() as f64 / 960.0 * 0.5; // At 120 BPM
                    // More accurate: we need tempo, but for now approximate
                    // Actually, store audio_offset in samples, so we need to convert ticks to samples
                    // This is tricky without tempo - let's use a simpler approach
                    let right_offset = existing.audio_offset
                        + ticks_to_samples_approx(ticks_into_audio, existing.audio.sample_rate());

                    let right = Clip {
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
                    let trim_samples =
                        ticks_to_samples_approx(trim_ticks, existing.audio.sample_rate());

                    let trimmed = Clip {
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
                    let trimmed = Clip {
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

        // Add the new clip
        result.push(new_clip);

        // Sort by start_tick
        result.sort_by_key(|s| s.start_tick);

        self.clips = result;
    }

    /// Build from a list of clips, inserting each one (resolving overlaps)
    pub fn from_clips(id: TrackId, name: String, clips: Vec<Clip>) -> Self {
        let mut track = Self::new(id, name);
        for clip in clips {
            track.insert_clip(clip);
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

/// Resample an `AudioArc` to a target sample rate.
///
/// This performs high-quality sinc interpolation resampling. If the audio is already
/// at the target rate, returns a cheap clone.
///
/// # Examples
///
/// ```
/// use daw_transport::{AudioArc, resample_audio_arc};
///
/// let audio = AudioArc::new(vec![0.0; 44100], 44100, 1);
/// let resampled = resample_audio_arc(&audio, 48000).unwrap();
/// assert_eq!(resampled.sample_rate(), 48000);
/// ```
pub fn resample_audio_arc(audio: &AudioArc, target_sample_rate: u32) -> anyhow::Result<AudioArc> {
    // If already at target rate, return a cheap clone
    if audio.sample_rate == target_sample_rate {
        return Ok(audio.clone());
    }

    let channels = audio.channels as usize;
    let input_frames = audio.frames();

    // Calculate output length
    let resample_ratio = target_sample_rate as f64 / audio.sample_rate as f64;
    let output_frames = (input_frames as f64 * resample_ratio).ceil() as usize;

    // Convert interleaved samples to per-channel format for rubato
    let mut input_channels = vec![Vec::with_capacity(input_frames); channels];
    for frame_idx in 0..input_frames {
        for ch in 0..channels {
            input_channels[ch].push(audio.samples()[frame_idx * channels + ch]);
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

    Ok(AudioArc::new(
        output_samples,
        target_sample_rate,
        audio.channels,
    ))
}

/// Resample an audio buffer to a target sample rate (legacy API)
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

#[cfg(test)]
mod audio_arc_tests {
    use super::*;
    use std::f32::consts::PI;

    /// Helper: Generate a sine wave
    fn generate_sine_wave(
        frequency: f32,
        sample_rate: u32,
        duration_secs: f32,
        channels: u16,
    ) -> AudioArc {
        let num_frames = (sample_rate as f32 * duration_secs) as usize;
        let mut samples = Vec::with_capacity(num_frames * channels as usize);

        for i in 0..num_frames {
            let t = i as f32 / sample_rate as f32;
            let sample = (2.0 * PI * frequency * t).sin();
            for _ in 0..channels {
                samples.push(sample);
            }
        }

        AudioArc::new(samples, sample_rate, channels)
    }

    #[test]
    fn test_audio_arc_new() {
        let samples = vec![0.0, 0.1, 0.2, 0.3];
        let audio = AudioArc::new(samples, 44100, 2);

        assert_eq!(audio.sample_rate(), 44100);
        assert_eq!(audio.channels(), 2);
        assert_eq!(audio.frames(), 2);
        assert_eq!(audio.len(), 4);
        assert!(!audio.is_empty());
    }

    #[test]
    #[should_panic(expected = "channels must be greater than 0")]
    fn test_audio_arc_zero_channels() {
        AudioArc::new(vec![0.0], 44100, 0);
    }

    #[test]
    #[should_panic(expected = "samples.len() must be divisible by channels")]
    fn test_audio_arc_invalid_length() {
        // 5 samples with 2 channels is invalid
        AudioArc::new(vec![0.0, 0.1, 0.2, 0.3, 0.4], 44100, 2);
    }

    #[test]
    fn test_audio_arc_clone_is_cheap() {
        let samples = vec![0.0; 100000];
        let audio = AudioArc::new(samples, 44100, 2);

        // Clone should share the same Arc
        let audio2 = audio.clone();

        // Both should point to the same data
        assert_eq!(Arc::strong_count(audio.samples_arc()), 2);
        assert_eq!(Arc::strong_count(audio2.samples_arc()), 2);
    }

    #[test]
    fn test_audio_arc_samples_access() {
        let samples = vec![1.0, 2.0, 3.0, 4.0];
        let audio = AudioArc::new(samples.clone(), 44100, 2);

        assert_eq!(audio.samples(), &[1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn test_audio_arc_duration() {
        // 44100 frames at 44100 Hz = 1 second
        let audio = AudioArc::new(vec![0.0; 44100 * 2], 44100, 2);
        assert!((audio.duration_secs() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_audio_arc_channel_iterator() {
        let samples = vec![0.0, 1.0, 0.5, 1.5, 0.25, 1.25]; // 3 frames, 2 channels
        let audio = AudioArc::new(samples, 44100, 2);

        let left: Vec<f32> = audio.channel(0).collect();
        assert_eq!(left, vec![0.0, 0.5, 0.25]);

        let right: Vec<f32> = audio.channel(1).collect();
        assert_eq!(right, vec![1.0, 1.5, 1.25]);
    }

    #[test]
    #[should_panic(expected = "channel index out of bounds")]
    fn test_audio_arc_channel_out_of_bounds() {
        let audio = AudioArc::new(vec![0.0, 0.0], 44100, 2);
        let _: Vec<f32> = audio.channel(2).collect(); // Only has channels 0 and 1
    }

    #[test]
    fn test_audio_arc_from_arc() {
        let samples = vec![1.0, 2.0, 3.0, 4.0];
        let arc_samples = Arc::from(samples.clone());

        let audio = AudioArc::from_arc(arc_samples, 44100, 2);
        assert_eq!(audio.samples(), &[1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn test_audio_arc_empty() {
        let audio = AudioArc::new(vec![], 44100, 1);
        assert!(audio.is_empty());
        assert_eq!(audio.len(), 0);
        assert_eq!(audio.frames(), 0);
    }

    #[test]
    fn test_audio_arc_to_from_audio_buffer() {
        let buffer = AudioBuffer {
            samples: vec![1.0, 2.0, 3.0, 4.0],
            sample_rate: 48000,
            channels: 2,
        };

        // Convert to AudioArc
        let audio_arc = AudioArc::from_audio_buffer(buffer);
        assert_eq!(audio_arc.sample_rate(), 48000);
        assert_eq!(audio_arc.channels(), 2);
        assert_eq!(audio_arc.samples(), &[1.0, 2.0, 3.0, 4.0]);

        // Convert back to AudioBuffer
        let buffer2 = audio_arc.to_audio_buffer();
        assert_eq!(buffer2.sample_rate, 48000);
        assert_eq!(buffer2.channels, 2);
        assert_eq!(buffer2.samples, vec![1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn test_resample_audio_arc_same_rate() {
        let audio = generate_sine_wave(440.0, 44100, 0.1, 2);
        let original_len = audio.len();

        // Resampling to same rate should return a cheap clone
        let resampled = audio.resample(44100).unwrap();

        assert_eq!(resampled.sample_rate(), 44100);
        assert_eq!(resampled.channels(), 2);
        assert_eq!(resampled.len(), original_len);

        // Should share the same Arc
        assert_eq!(Arc::strong_count(audio.samples_arc()), 2);
    }

    #[test]
    fn test_resample_audio_arc_upsampling() {
        // Resample from 44100 to 48000
        let audio = generate_sine_wave(440.0, 44100, 0.1, 2);
        let original_frames = audio.frames();

        let resampled = audio.resample(48000).unwrap();

        assert_eq!(resampled.sample_rate(), 48000);
        assert_eq!(resampled.channels(), 2);

        // Output should be approximately scaled by the ratio
        let expected_frames = (original_frames as f64 * 48000.0 / 44100.0) as usize;
        let resampled_frames = resampled.frames();

        // Allow 3% tolerance for filter delay and rounding
        let tolerance = (expected_frames as f64 * 0.03) as i32;
        assert!(
            (resampled_frames as i32 - expected_frames as i32).abs() <= tolerance,
            "expected ~{} frames, got {} (diff: {})",
            expected_frames,
            resampled_frames,
            (resampled_frames as i32 - expected_frames as i32).abs()
        );
    }

    #[test]
    fn test_resample_audio_arc_downsampling() {
        // Resample from 48000 to 44100
        let audio = generate_sine_wave(440.0, 48000, 0.1, 2);
        let original_frames = audio.frames();

        let resampled = audio.resample(44100).unwrap();

        assert_eq!(resampled.sample_rate(), 44100);
        assert_eq!(resampled.channels(), 2);

        // Output should be approximately scaled by the ratio
        let expected_frames = (original_frames as f64 * 44100.0 / 48000.0) as usize;
        let resampled_frames = resampled.frames();

        // Allow 3% tolerance
        let tolerance = (expected_frames as f64 * 0.03) as i32;
        assert!(
            (resampled_frames as i32 - expected_frames as i32).abs() <= tolerance,
            "expected ~{} frames, got {} (diff: {})",
            expected_frames,
            resampled_frames,
            (resampled_frames as i32 - expected_frames as i32).abs()
        );
    }

    #[test]
    fn test_resample_audio_arc_preserves_frequency() {
        // Generate a 440 Hz sine wave at 44100 Hz
        let audio = generate_sine_wave(440.0, 44100, 0.1, 1);

        // Resample to 48000 Hz
        let resampled = audio.resample(48000).unwrap();

        // The frequency content should be preserved
        // Check by counting zero crossings
        let zero_crossings = count_zero_crossings(resampled.samples());
        let duration = resampled.frames() as f32 / resampled.sample_rate() as f32;
        let estimated_frequency = zero_crossings as f32 / (2.0 * duration);

        // Allow 5% tolerance
        assert!(
            (estimated_frequency - 440.0).abs() < 22.0,
            "expected ~440 Hz, got {} Hz",
            estimated_frequency
        );
    }

    #[test]
    fn test_resample_audio_arc_mono() {
        let audio = generate_sine_wave(440.0, 44100, 0.05, 1);
        let resampled = audio.resample(48000).unwrap();

        assert_eq!(resampled.channels(), 1);
        assert_eq!(resampled.sample_rate(), 48000);
    }

    #[test]
    fn test_resample_audio_arc_extreme_ratio() {
        // Test a more extreme resampling ratio
        let audio = generate_sine_wave(440.0, 22050, 0.05, 2);
        let resampled = audio.resample(96000).unwrap();

        assert_eq!(resampled.sample_rate(), 96000);
        assert_eq!(resampled.channels(), 2);

        let original_frames = audio.frames();
        let expected_frames = (original_frames as f64 * 96000.0 / 22050.0) as usize;
        let resampled_frames = resampled.frames();

        // Allow 12% tolerance for extreme ratios
        let tolerance = (expected_frames as f64 * 0.12) as i32;
        assert!(
            (resampled_frames as i32 - expected_frames as i32).abs() <= tolerance,
            "expected ~{} frames, got {} (diff: {})",
            expected_frames,
            resampled_frames,
            (resampled_frames as i32 - expected_frames as i32).abs()
        );
    }

    #[test]
    fn test_resample_audio_arc_creates_new_arc() {
        let audio = generate_sine_wave(440.0, 44100, 0.05, 1);
        let original_strong_count = Arc::strong_count(audio.samples_arc());

        // Resampling to different rate should create new Arc
        let resampled = audio.resample(48000).unwrap();

        // Original should still have only 1 strong reference
        assert_eq!(
            Arc::strong_count(audio.samples_arc()),
            original_strong_count
        );
        // Resampled should have its own Arc
        assert_eq!(Arc::strong_count(resampled.samples_arc()), 1);
    }

    #[test]
    fn test_audio_arc_debug_format() {
        let audio = AudioArc::new(vec![0.0; 44100], 44100, 1);
        let debug_str = format!("{:?}", audio);

        assert!(debug_str.contains("AudioArc"));
        assert!(debug_str.contains("frames"));
        assert!(debug_str.contains("sample_rate"));
        assert!(debug_str.contains("channels"));
        assert!(debug_str.contains("duration_secs"));
    }

    /// Helper function to count zero crossings in a signal
    fn count_zero_crossings(samples: &[f32]) -> usize {
        let mut count = 0;
        for i in 1..samples.len() {
            if (samples[i - 1] < 0.0 && samples[i] >= 0.0)
                || (samples[i - 1] >= 0.0 && samples[i] < 0.0)
            {
                count += 1;
            }
        }
        count
    }
}
