use std::path::Path;
use std::sync::Arc;

use cpal::{
    FromSample, SizedSample,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};

fn main() -> anyhow::Result<()> {
    let audio_buffer = daw_decode::decode_file(Path::new("bongo-l.wav"))?;
    println!(
        "Loaded audio: {} samples, {} Hz, {} channels",
        audio_buffer.samples.len(),
        audio_buffer.sample_rate,
        audio_buffer.channels
    );

    let samples = Arc::new(audio_buffer.samples);
    let file_channels = audio_buffer.channels as usize;

    let host = cpal::default_host();

    let device = host
        .default_output_device()
        .expect("failed to find output device");
    println!("Default output device: {:?}", device.name());

    let config = device.default_output_config().unwrap();
    println!("Default output config: {config:?}");

    match config.sample_format() {
        cpal::SampleFormat::F32 => run::<f32>(&device, &config.into(), samples, file_channels),
        sample_format => panic!("Unsupported sample format '{sample_format}'"),
    }
}

pub fn run<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    samples: Arc<Vec<f32>>,
    file_channels: usize,
) -> Result<(), anyhow::Error>
where
    T: SizedSample + FromSample<f32>,
{
    let output_channels = config.channels as usize;
    let mut position = 0usize;
    let sample_count = samples.len();

    let err_fn = |err| eprintln!("an error occurred on stream: {err}");

    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            for frame in data.chunks_mut(output_channels) {
                for (ch, sample) in frame.iter_mut().enumerate() {
                    let file_ch = ch % file_channels;
                    let idx = position * file_channels + file_ch;
                    let value = if idx < samples.len() {
                        samples[idx]
                    } else {
                        0.0
                    };
                    *sample = T::from_sample(value);
                }
                if position * file_channels < samples.len() {
                    position += 1;
                }
            }
        },
        err_fn,
        None,
    )?;
    stream.play()?;

    let duration_secs = sample_count as f32 / file_channels as f32 / config.sample_rate.0 as f32;
    std::thread::sleep(std::time::Duration::from_secs_f32(duration_secs + 0.1));

    Ok(())
}
