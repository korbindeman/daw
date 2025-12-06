use std::sync::Arc;

use basedrop::{Collector, Handle, Shared};
use cpal::{
    FromSample, SizedSample,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use daw_transport::{AudioBuffer, resample_audio};

/// Resample a clip's audio to the target sample rate
/// Returns a new EngineClip with resampled audio
pub fn resample_clip(clip: EngineClip, target_sample_rate: u32) -> anyhow::Result<EngineClip> {
    let resampled_audio = resample_audio(&clip.audio, target_sample_rate)?;
    Ok(EngineClip {
        start: clip.start,
        audio: Arc::new(resampled_audio),
    })
}

/// Engine-side clip with sample-based position (converted from ticks by core)
#[derive(Clone)]
pub struct EngineClip {
    pub start: u64, // sample position on timeline
    pub audio: Arc<AudioBuffer>,
}

/// Engine-side track
#[derive(Clone)]
pub struct EngineTrack {
    pub clips: Vec<EngineClip>,
    pub volume: f32, // Linear gain multiplier (0.0 = silence, 1.0 = unity)
}

type SharedTracks = Shared<Vec<EngineTrack>>;

struct PlaybackState {
    playing: bool,
    position: u64, // sample position
}

/// Commands sent from core to engine
#[derive(Debug)]
pub enum EngineCommand {
    Play,
    Pause,
    Seek { sample: u64 },
}

/// Status updates sent from engine to core
#[derive(Debug)]
pub enum EngineStatus {
    Position(u64), // current sample position
}

pub struct AudioEngineHandle {
    pub commands: rtrb::Producer<EngineCommand>,
    pub status: rtrb::Consumer<EngineStatus>,
    pub tracks: rtrb::Producer<SharedTracks>,
    pub collector: Collector,
    pub handle: Handle,
    pub sample_rate: u32,
    _stream: cpal::Stream,
}

pub fn start(tracks: Vec<EngineTrack>) -> anyhow::Result<AudioEngineHandle> {
    let collector = Collector::new();
    let handle = collector.handle();

    let (command_tx, command_rx) = rtrb::RingBuffer::<EngineCommand>::new(64);
    let (status_tx, status_rx) = rtrb::RingBuffer::<EngineStatus>::new(64);
    let (tracks_tx, tracks_rx) = rtrb::RingBuffer::<SharedTracks>::new(4);

    let initial_tracks = Shared::new(&handle, tracks);

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| anyhow::anyhow!("no output device found"))?;

    let config = device.default_output_config()?;
    let sample_rate = config.sample_rate().0;

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => build_stream::<f32>(
            &device,
            &config.into(),
            initial_tracks,
            command_rx,
            tracks_rx,
            status_tx,
        )?,
        sample_format => anyhow::bail!("unsupported sample format '{sample_format}'"),
    };

    stream.play()?;

    Ok(AudioEngineHandle {
        commands: command_tx,
        status: status_rx,
        tracks: tracks_tx,
        collector,
        handle,
        sample_rate,
        _stream: stream,
    })
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    initial_tracks: SharedTracks,
    mut command_rx: rtrb::Consumer<EngineCommand>,
    mut tracks_rx: rtrb::Consumer<SharedTracks>,
    mut status_tx: rtrb::Producer<EngineStatus>,
) -> anyhow::Result<cpal::Stream>
where
    T: SizedSample + FromSample<f32>,
{
    let output_channels = config.channels as usize;

    let mut state = PlaybackState {
        playing: false,
        position: 0,
    };

    let mut current_tracks = initial_tracks;

    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            // Swap in new tracks if available (lock-free)
            while let Ok(new_tracks) = tracks_rx.pop() {
                current_tracks = new_tracks;
            }

            while let Ok(cmd) = command_rx.pop() {
                match cmd {
                    EngineCommand::Play => state.playing = true,
                    EngineCommand::Pause => state.playing = false,
                    EngineCommand::Seek { sample } => state.position = sample,
                }
            }

            let _ = status_tx.push(EngineStatus::Position(state.position));

            for frame in data.chunks_mut(output_channels) {
                if state.playing {
                    let mut mixed = vec![0.0f32; output_channels];

                    for track in current_tracks.iter() {
                        for clip in &track.clips {
                            let clip_channels = clip.audio.channels as usize;
                            let clip_total_frames = clip.audio.samples.len() / clip_channels;

                            // clip.start and clip.audio are both in output sample rate (converted by core)
                            let clip_start = clip.start;
                            let clip_end = clip_start + clip_total_frames as u64;

                            if state.position >= clip_start && state.position < clip_end {
                                let sample_offset = state.position - clip_start;
                                let frame_index = sample_offset as usize;

                                if frame_index < clip_total_frames {
                                    for (ch, mix_sample) in mixed.iter_mut().enumerate() {
                                        let clip_ch = ch % clip_channels;
                                        let idx = frame_index * clip_channels + clip_ch;
                                        if idx < clip.audio.samples.len() {
                                            *mix_sample += clip.audio.samples[idx] * track.volume;
                                        }
                                    }
                                }
                            }
                        }
                    }

                    for (ch, sample) in frame.iter_mut().enumerate() {
                        *sample = T::from_sample(mixed[ch]);
                    }

                    state.position += 1;
                } else {
                    for sample in frame.iter_mut() {
                        *sample = T::from_sample(0.0);
                    }
                }
            }
        },
        |err| eprintln!("stream error: {err}"),
        None,
    )?;

    Ok(stream)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    /// Generate a sine wave at a specific frequency
    fn generate_sine_wave(
        frequency: f32,
        sample_rate: u32,
        duration_secs: f32,
        channels: u16,
    ) -> AudioBuffer {
        let num_samples = (sample_rate as f32 * duration_secs) as usize;
        let mut samples = Vec::with_capacity(num_samples * channels as usize);

        for i in 0..num_samples {
            let t = i as f32 / sample_rate as f32;
            let sample = (2.0 * PI * frequency * t).sin();
            // Duplicate for all channels
            for _ in 0..channels {
                samples.push(sample);
            }
        }

        AudioBuffer {
            samples,
            sample_rate,
            channels,
        }
    }

    #[test]
    fn test_resample_no_change_same_rate() {
        let buffer = generate_sine_wave(440.0, 44100, 0.1, 2);
        let original_len = buffer.samples.len();
        let original_rate = buffer.sample_rate;

        let resampled = resample_audio(&buffer, 44100).expect("resample failed");

        assert_eq!(resampled.sample_rate, original_rate);
        assert_eq!(resampled.channels, 2);
        assert_eq!(resampled.samples.len(), original_len);
    }

    #[test]
    fn test_resample_upsampling() {
        // Resample from 44100 to 48000
        let buffer = generate_sine_wave(440.0, 44100, 0.1, 2);
        let original_frames = buffer.samples.len() / buffer.channels as usize;

        let resampled = resample_audio(&buffer, 48000).expect("resample failed");

        assert_eq!(resampled.sample_rate, 48000);
        assert_eq!(resampled.channels, 2);

        // Output should be approximately scaled by the ratio
        let expected_frames = (original_frames as f64 * 48000.0 / 44100.0) as usize;
        let resampled_frames = resampled.samples.len() / resampled.channels as usize;

        // Allow some tolerance for filter delay and rounding (about 3% tolerance)
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
    fn test_resample_downsampling() {
        // Resample from 48000 to 44100
        let buffer = generate_sine_wave(440.0, 48000, 0.1, 2);
        let original_frames = buffer.samples.len() / buffer.channels as usize;

        let resampled = resample_audio(&buffer, 44100).expect("resample failed");

        assert_eq!(resampled.sample_rate, 44100);
        assert_eq!(resampled.channels, 2);

        // Output should be approximately scaled by the ratio
        let expected_frames = (original_frames as f64 * 44100.0 / 48000.0) as usize;
        let resampled_frames = resampled.samples.len() / resampled.channels as usize;

        // Allow some tolerance for filter delay and rounding (about 3% tolerance)
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
    fn test_resample_preserves_frequency() {
        // Generate a 440 Hz sine wave at 44100 Hz
        let buffer = generate_sine_wave(440.0, 44100, 0.1, 1);

        // Resample to 48000 Hz
        let resampled = resample_audio(&buffer, 48000).expect("resample failed");

        // The frequency content should be preserved
        // We'll check this by verifying zero crossings are at the expected rate
        let zero_crossings = count_zero_crossings(&resampled.samples);
        let duration = resampled.samples.len() as f32 / resampled.sample_rate as f32;
        let estimated_frequency = zero_crossings as f32 / (2.0 * duration);

        // Allow 5% tolerance
        assert!(
            (estimated_frequency - 440.0).abs() < 22.0,
            "expected ~440 Hz, got {} Hz",
            estimated_frequency
        );
    }

    #[test]
    fn test_resample_clip() {
        let audio = generate_sine_wave(440.0, 44100, 0.1, 2);
        let clip = EngineClip {
            start: 0,
            audio: Arc::new(audio),
        };

        let resampled_clip = resample_clip(clip, 48000).expect("resample clip failed");

        assert_eq!(resampled_clip.start, 0);
        assert_eq!(resampled_clip.audio.sample_rate, 48000);
        assert_eq!(resampled_clip.audio.channels, 2);
    }

    #[test]
    fn test_resample_mono_to_mono() {
        let buffer = generate_sine_wave(440.0, 44100, 0.05, 1);
        let resampled = resample_audio(&buffer, 48000).expect("resample failed");

        assert_eq!(resampled.channels, 1);
        assert_eq!(resampled.sample_rate, 48000);
    }

    #[test]
    fn test_resample_extreme_ratio() {
        // Test a more extreme resampling ratio
        let buffer = generate_sine_wave(440.0, 22050, 0.05, 2);
        let resampled = resample_audio(&buffer, 96000).expect("resample failed");

        assert_eq!(resampled.sample_rate, 96000);
        assert_eq!(resampled.channels, 2);

        // Check the length is approximately scaled
        let original_frames = buffer.samples.len() / buffer.channels as usize;
        let expected_frames = (original_frames as f64 * 96000.0 / 22050.0) as usize;
        let resampled_frames = resampled.samples.len() / resampled.channels as usize;

        // Allow some tolerance for filter delay and rounding (about 12% tolerance for extreme ratios)
        let tolerance = (expected_frames as f64 * 0.12) as i32;
        assert!(
            (resampled_frames as i32 - expected_frames as i32).abs() <= tolerance,
            "expected ~{} frames, got {} (diff: {})",
            expected_frames,
            resampled_frames,
            (resampled_frames as i32 - expected_frames as i32).abs()
        );
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
