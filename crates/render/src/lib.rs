use std::{path::Path, sync::Arc};

use daw_transport::{AudioBuffer, PPQN, Track, resample_audio};

pub fn ticks_to_samples(ticks: f64, tempo: f64, sample_rate: u32) -> f64 {
    let seconds_per_beat = 60.0 / tempo;
    let seconds_per_tick = seconds_per_beat / PPQN as f64;
    ticks * seconds_per_tick * sample_rate as f64
}

fn samples_to_ticks(samples: f64, tempo: f64, sample_rate: u32) -> f64 {
    let seconds_per_beat = 60.0 / tempo;
    let seconds_per_tick = seconds_per_beat / PPQN as f64;
    samples / (seconds_per_tick * sample_rate as f64)
}

fn calculate_end_tick(tracks: &[Track]) -> u64 {
    let mut max_end_tick = 0u64;
    for track in tracks {
        if !track.enabled {
            continue;
        }
        for segment in track.segments() {
            max_end_tick = max_end_tick.max(segment.end_tick);
        }
    }
    max_end_tick
}

pub fn render_timeline(
    tracks: &[Track],
    tempo: f64,
    sample_rate: u32,
    channels: u16,
) -> AudioBuffer {
    let end_tick = calculate_end_tick(tracks);
    let total_samples = ticks_to_samples(end_tick as f64, tempo, sample_rate) as usize;
    let output_channels = channels as usize;

    // Pre-convert all segments to sample space and resample to output sample rate
    struct RenderSegment {
        start_sample: u64,
        end_sample: u64,
        offset: u64, // offset into audio in samples
        audio: Arc<AudioBuffer>,
    }

    let mut render_tracks: Vec<(f32, Vec<RenderSegment>)> = Vec::new();

    for track in tracks {
        if !track.enabled {
            continue;
        }

        let mut render_segments = Vec::new();
        for segment in track.segments() {
            // Resample if needed
            let resampled_audio = if segment.audio.sample_rate != sample_rate {
                match resample_audio(&segment.audio, sample_rate) {
                    Ok(audio) => Arc::new(audio),
                    Err(_) => continue, // Skip segment if resampling fails
                }
            } else {
                segment.audio.clone()
            };

            // Convert tick positions to sample positions
            let start_sample = ticks_to_samples(segment.start_tick as f64, tempo, sample_rate) as u64;
            let end_sample = ticks_to_samples(segment.end_tick as f64, tempo, sample_rate) as u64;

            render_segments.push(RenderSegment {
                start_sample,
                end_sample,
                offset: segment.audio_offset,
                audio: resampled_audio,
            });
        }
        render_tracks.push((track.volume, render_segments));
    }

    // Render in sample space (like the engine does)
    let mut samples = vec![0.0f32; total_samples * output_channels];

    for frame_idx in 0..total_samples {
        let position = frame_idx as u64;

        for (track_volume, render_segments) in &render_tracks {
            for segment in render_segments {
                if position >= segment.start_sample && position < segment.end_sample {
                    let timeline_offset = position - segment.start_sample;
                    // Add segment.offset to get the actual position in the audio buffer
                    let source_frame_idx = (segment.offset as usize) + (timeline_offset as usize);
                    let segment_channels = segment.audio.channels as usize;

                    for ch in 0..output_channels {
                        let segment_ch = ch % segment_channels;
                        let src_idx = source_frame_idx * segment_channels + segment_ch;
                        let dst_idx = frame_idx * output_channels + ch;
                        if src_idx < segment.audio.samples.len() {
                            samples[dst_idx] += segment.audio.samples[src_idx] * track_volume;
                        }
                    }
                }
            }
        }
    }

    AudioBuffer {
        samples,
        sample_rate,
        channels,
    }
}

pub fn write_wav(buffer: &AudioBuffer, path: &Path) -> anyhow::Result<()> {
    let spec = hound::WavSpec {
        channels: buffer.channels,
        sample_rate: buffer.sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let mut writer = hound::WavWriter::create(path, spec)?;

    for &sample in &buffer.samples {
        writer.write_sample(sample)?;
    }

    writer.finalize()?;
    Ok(())
}
