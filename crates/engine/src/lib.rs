use std::sync::Arc;

use basedrop::{Collector, Handle, Shared};
use cpal::{
    FromSample, SizedSample,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use daw_transport::AudioBuffer;

/// Engine-side clip with sample-based position (converted from ticks by core)
#[derive(Clone)]
pub struct EngineClip {
    pub start: u64,  // sample position on timeline
    pub audio: Arc<AudioBuffer>,
}

/// Engine-side track
#[derive(Clone)]
pub struct EngineTrack {
    pub clips: Vec<EngineClip>,
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

                            // clip.start is already in output sample rate (converted by core)
                            // TODO: handle sample rate conversion if clip rate != output rate
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
                                            *mix_sample += clip.audio.samples[idx];
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
