use std::path::Path;

use daw_transport::{AudioArc, PPQN, Track};

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
        for clip in track.clips() {
            max_end_tick = max_end_tick.max(clip.end_tick);
        }
    }
    max_end_tick
}

pub fn render_timeline(tracks: &[Track], tempo: f64, sample_rate: u32, channels: u16) -> AudioArc {
    let end_tick = calculate_end_tick(tracks);
    let total_samples = ticks_to_samples(end_tick as f64, tempo, sample_rate) as usize;
    let output_channels = channels as usize;

    // Pre-convert all clips to sample space and resample to output sample rate
    struct RenderClip {
        start_sample: u64,
        end_sample: u64,
        offset: u64, // offset into audio in samples
        audio: AudioArc,
    }

    let mut render_tracks: Vec<(f32, Vec<RenderClip>)> = Vec::new();

    for track in tracks {
        if !track.enabled {
            continue;
        }

        let mut render_clips = Vec::new();
        for clip in track.clips() {
            // Resample if needed (cheap clone if already at target rate)
            let resampled_audio = if clip.audio.sample_rate() != sample_rate {
                match clip.audio.resample(sample_rate) {
                    Ok(audio) => audio,
                    Err(_) => continue, // Skip clip if resampling fails
                }
            } else {
                clip.audio.clone()
            };

            // Convert tick positions to sample positions
            let start_sample = ticks_to_samples(clip.start_tick as f64, tempo, sample_rate) as u64;
            let end_sample = ticks_to_samples(clip.end_tick as f64, tempo, sample_rate) as u64;

            render_clips.push(RenderClip {
                start_sample,
                end_sample,
                offset: clip.audio_offset,
                audio: resampled_audio,
            });
        }
        render_tracks.push((track.volume, render_clips));
    }

    // Render in sample space (like the engine does)
    let mut samples = vec![0.0f32; total_samples * output_channels];

    for frame_idx in 0..total_samples {
        let position = frame_idx as u64;

        for (track_volume, render_clips) in &render_tracks {
            for clip in render_clips {
                if position >= clip.start_sample && position < clip.end_sample {
                    let timeline_offset = position - clip.start_sample;
                    // Add clip.offset to get the actual position in the audio buffer
                    let source_frame_idx = (clip.offset as usize) + (timeline_offset as usize);
                    let clip_channels = clip.audio.channels() as usize;

                    for ch in 0..output_channels {
                        let clip_ch = ch % clip_channels;
                        let src_idx = source_frame_idx * clip_channels + clip_ch;
                        let dst_idx = frame_idx * output_channels + ch;
                        if src_idx < clip.audio.samples().len() {
                            samples[dst_idx] += clip.audio.samples()[src_idx] * track_volume;
                        }
                    }
                }
            }
        }
    }

    AudioArc::new(samples, sample_rate, channels)
}

pub fn write_wav(buffer: &AudioArc, path: &Path) -> anyhow::Result<()> {
    let spec = hound::WavSpec {
        channels: buffer.channels(),
        sample_rate: buffer.sample_rate(),
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let mut writer = hound::WavWriter::create(path, spec)?;

    for &sample in buffer.samples() {
        writer.write_sample(sample)?;
    }

    writer.finalize()?;
    Ok(())
}
