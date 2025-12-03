use std::fs::File;
use std::path::{Path, PathBuf};

use daw_transport::AudioBuffer;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

const SAMPLES_ROOT: &str = "samples";

pub fn resolve_sample_path(path: &Path) -> Option<PathBuf> {
    if path.exists() {
        return Some(path.to_path_buf());
    }

    let file_name = path.file_name()?;
    let root = Path::new(SAMPLES_ROOT);

    if root.join(path).exists() {
        return Some(root.join(path));
    }

    for entry in root.read_dir().ok()?.flatten() {
        if entry.path().is_dir() {
            let candidate = entry.path().join(file_name);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    None
}

pub fn decode_file(path: &Path) -> anyhow::Result<AudioBuffer> {
    let resolved = resolve_sample_path(path)
        .ok_or_else(|| anyhow::anyhow!("sample not found: {}", path.display()))?;
    decode_file_direct(&resolved)
}

pub fn decode_file_direct(path: &Path) -> anyhow::Result<AudioBuffer> {
    let file = File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;

    let mut format = probed.format;

    let track = format
        .default_track()
        .ok_or_else(|| anyhow::anyhow!("no default track"))?;

    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
    let channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(2) as u16;
    let track_id = track.id;

    let mut decoder =
        symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;

    let mut samples = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => return Err(e.into()),
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = decoder.decode(&packet)?;
        let spec = *decoded.spec();
        let duration = decoded.capacity() as u64;

        let mut sample_buf = SampleBuffer::<f32>::new(duration, spec);
        sample_buf.copy_interleaved_ref(decoded);
        samples.extend_from_slice(sample_buf.samples());
    }

    Ok(AudioBuffer {
        samples,
        sample_rate,
        channels,
    })
}
