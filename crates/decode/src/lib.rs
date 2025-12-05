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

/// Resolve a sample path to an absolute path.
/// Accepts paths relative to the samples root (e.g., `cr78/hihat.wav`)
/// or paths that already include the samples root (e.g., `samples/cr78/hihat.wav`).
pub fn resolve_sample_path(path: &Path) -> Option<PathBuf> {
    // Check if path exists as-is
    if path.exists() {
        return Some(path.to_path_buf());
    }

    let root = Path::new(SAMPLES_ROOT);

    // Check if samples_root/path exists
    let with_root = root.join(path);
    if with_root.exists() {
        return Some(with_root);
    }

    None
}

/// Strip the samples root prefix from a path if present.
/// Use this when saving paths to project files.
pub fn strip_samples_root(path: &Path) -> PathBuf {
    let root = Path::new(SAMPLES_ROOT);
    path.strip_prefix(root)
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|_| path.to_path_buf())
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
