use cpal::{
    FromSample, SizedSample,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use daw_transport::{Command, Status, Track};

const PPQN: f64 = 960.0;

struct PlaybackState {
    playing: bool,
    position: f64, // (fractional) ticks
    tempo: f64,    // BPM
}

fn ticks_to_samples(ticks: f64, tempo: f64, sample_rate: u32) -> f64 {
    let seconds_per_beat = 60.0 / tempo;
    let seconds_per_tick = seconds_per_beat / PPQN;
    ticks * seconds_per_tick * sample_rate as f64
}

fn samples_to_ticks(samples: f64, tempo: f64, sample_rate: u32) -> f64 {
    let seconds_per_beat = 60.0 / tempo;
    let seconds_per_tick = seconds_per_beat / PPQN;
    samples / (seconds_per_tick * sample_rate as f64)
}

pub struct AudioEngineHandle {
    pub commands: rtrb::Producer<Command>,
    pub status: rtrb::Consumer<Status>,
    _stream: cpal::Stream,
}

pub fn start(tracks: Vec<Track>, tempo: f64) -> anyhow::Result<AudioEngineHandle> {
    let (command_tx, command_rx) = rtrb::RingBuffer::<Command>::new(64);
    let (status_tx, status_rx) = rtrb::RingBuffer::<Status>::new(64);

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| anyhow::anyhow!("no output device found"))?;

    let config = device.default_output_config()?;

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => build_stream::<f32>(
            &device,
            &config.into(),
            tracks,
            tempo,
            command_rx,
            status_tx,
        )?,
        sample_format => anyhow::bail!("unsupported sample format '{sample_format}'"),
    };

    stream.play()?;

    Ok(AudioEngineHandle {
        commands: command_tx,
        status: status_rx,
        _stream: stream,
    })
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    tracks: Vec<Track>,
    tempo: f64,
    mut command_rx: rtrb::Consumer<Command>,
    mut status_tx: rtrb::Producer<Status>,
) -> anyhow::Result<cpal::Stream>
where
    T: SizedSample + FromSample<f32>,
{
    let output_channels = config.channels as usize;
    let sample_rate = config.sample_rate.0;

    let mut state = PlaybackState {
        playing: false,
        position: 0.0,
        tempo,
    };

    let ticks_per_sample = samples_to_ticks(1.0, tempo, sample_rate);

    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            while let Ok(cmd) = command_rx.pop() {
                match cmd {
                    Command::Play => state.playing = true,
                    Command::Pause => state.playing = false,
                    Command::Seek { tick } => state.position = tick as f64,
                }
            }

            let _ = status_tx.push(Status::Position(state.position as u64));

            for frame in data.chunks_mut(output_channels) {
                if state.playing {
                    let mut mixed = vec![0.0f32; output_channels];

                    for track in &tracks {
                        for clip in &track.clips {
                            let clip_start_tick = clip.start as f64;
                            let clip_channels = clip.audio.channels as usize;
                            let clip_sample_rate = clip.audio.sample_rate;
                            let clip_total_frames = clip.audio.samples.len() / clip_channels;
                            let clip_length_ticks = samples_to_ticks(
                                clip_total_frames as f64,
                                state.tempo,
                                clip_sample_rate,
                            );
                            let clip_end_tick = clip_start_tick + clip_length_ticks;

                            if state.position >= clip_start_tick && state.position < clip_end_tick {
                                let tick_offset = state.position - clip_start_tick;
                                let sample_offset =
                                    ticks_to_samples(tick_offset, state.tempo, clip_sample_rate);
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

                    state.position += ticks_per_sample;
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
