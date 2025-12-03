use basedrop::{Collector, Handle, Shared};
use cpal::{
    FromSample, SizedSample,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use daw_transport::{Command, PPQN, Status, Track};

type SharedTracks = Shared<Vec<Track>>;
type SharedTempo = Shared<f64>;

struct PlaybackState {
    playing: bool,
    position: f64, // (fractional) ticks
}

fn ticks_to_samples(ticks: f64, tempo: f64, sample_rate: u32) -> f64 {
    let seconds_per_beat = 60.0 / tempo;
    let seconds_per_tick = seconds_per_beat / PPQN as f64;
    ticks * seconds_per_tick * sample_rate as f64
}

fn samples_to_ticks(samples: f64, tempo: f64, sample_rate: u32) -> f64 {
    let seconds_per_beat = 60.0 / tempo;
    let seconds_per_tick = seconds_per_beat / PPQN as f64;
    samples / (seconds_per_tick * sample_rate as f64)
}

pub struct AudioEngineHandle {
    pub commands: rtrb::Producer<Command>,
    pub status: rtrb::Consumer<Status>,
    pub tracks: rtrb::Producer<SharedTracks>,
    pub tempo: rtrb::Producer<SharedTempo>,
    pub collector: Collector,
    pub handle: Handle,
    _stream: cpal::Stream,
}

pub fn start(tracks: Vec<Track>, tempo: f64) -> anyhow::Result<AudioEngineHandle> {
    let collector = Collector::new();
    let handle = collector.handle();

    let (command_tx, command_rx) = rtrb::RingBuffer::<Command>::new(64);
    let (status_tx, status_rx) = rtrb::RingBuffer::<Status>::new(64);
    let (tracks_tx, tracks_rx) = rtrb::RingBuffer::<SharedTracks>::new(4);
    let (tempo_tx, tempo_rx) = rtrb::RingBuffer::<SharedTempo>::new(4);

    let initial_tracks = Shared::new(&handle, tracks);
    let initial_tempo = Shared::new(&handle, tempo);

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| anyhow::anyhow!("no output device found"))?;

    let config = device.default_output_config()?;

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => build_stream::<f32>(
            &device,
            &config.into(),
            initial_tracks,
            initial_tempo,
            command_rx,
            tracks_rx,
            tempo_rx,
            status_tx,
        )?,
        sample_format => anyhow::bail!("unsupported sample format '{sample_format}'"),
    };

    stream.play()?;

    Ok(AudioEngineHandle {
        commands: command_tx,
        status: status_rx,
        tracks: tracks_tx,
        tempo: tempo_tx,
        collector,
        handle,
        _stream: stream,
    })
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    initial_tracks: SharedTracks,
    initial_tempo: SharedTempo,
    mut command_rx: rtrb::Consumer<Command>,
    mut tracks_rx: rtrb::Consumer<SharedTracks>,
    mut tempo_rx: rtrb::Consumer<SharedTempo>,
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
    };

    let mut current_tracks = initial_tracks;
    let mut current_tempo = initial_tempo;

    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            // Swap in new tracks/tempo if available (lock-free)
            while let Ok(new_tracks) = tracks_rx.pop() {
                current_tracks = new_tracks;
            }
            while let Ok(new_tempo) = tempo_rx.pop() {
                current_tempo = new_tempo;
            }

            let tempo = *current_tempo;
            let ticks_per_sample = samples_to_ticks(1.0, tempo, sample_rate);

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

                    for track in current_tracks.iter() {
                        for clip in &track.clips {
                            let clip_start_tick = clip.start as f64;
                            let clip_channels = clip.audio.channels as usize;
                            let clip_sample_rate = clip.audio.sample_rate;
                            let clip_total_frames = clip.audio.samples.len() / clip_channels;
                            let clip_length_ticks = samples_to_ticks(
                                clip_total_frames as f64,
                                tempo,
                                clip_sample_rate,
                            );
                            let clip_end_tick = clip_start_tick + clip_length_ticks;

                            if state.position >= clip_start_tick && state.position < clip_end_tick {
                                let tick_offset = state.position - clip_start_tick;
                                let sample_offset =
                                    ticks_to_samples(tick_offset, tempo, clip_sample_rate);
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
