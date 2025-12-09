use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use daw_transport::{AudioArc, AudioBuffer};
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

/// Decode an audio file and return an `AudioArc`.
///
/// This is the new preferred API for decoding audio files. It resolves the path
/// using `resolve_sample_path()` and decodes the audio into an `AudioArc`.
///
/// # Arguments
///
/// * `path` - Path to the audio file (can be relative to samples root)
/// * `target_sample_rate` - Optional target sample rate for resampling. If `None`,
///   returns audio at its original sample rate.
///
/// # Examples
///
/// ```no_run
/// use daw_decode::decode_audio_arc;
/// use std::path::Path;
///
/// // Decode at original sample rate
/// let audio = decode_audio_arc(Path::new("kick.wav"), None).unwrap();
///
/// // Decode and resample to 48kHz
/// let audio = decode_audio_arc(Path::new("kick.wav"), Some(48000)).unwrap();
/// ```
pub fn decode_audio_arc(path: &Path, target_sample_rate: Option<u32>) -> anyhow::Result<AudioArc> {
    let resolved = resolve_sample_path(path)
        .ok_or_else(|| anyhow::anyhow!("sample not found: {}", path.display()))?;
    decode_audio_arc_direct(&resolved, target_sample_rate)
}

/// Decode an audio file directly (without path resolution) and return an `AudioArc`.
///
/// # Arguments
///
/// * `path` - Absolute path to the audio file
/// * `target_sample_rate` - Optional target sample rate for resampling
pub fn decode_audio_arc_direct(
    path: &Path,
    target_sample_rate: Option<u32>,
) -> anyhow::Result<AudioArc> {
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

    let audio = AudioArc::new(samples, sample_rate, channels);

    // Resample if requested
    match target_sample_rate {
        Some(target_rate) if target_rate != sample_rate => audio.resample(target_rate),
        _ => Ok(audio),
    }
}

/// Two-tier audio cache for efficient loading and resampling.
///
/// `AudioCache` stores decoded audio files and their resampled versions to avoid
/// redundant decoding and resampling operations. It uses a two-tier strategy:
///
/// 1. **Original tier**: Stores decoded audio at its original sample rate
/// 2. **Resampled tier**: Stores resampled versions keyed by (file, target_rate)
///
/// All cached audio is stored as `AudioArc`, making clones very cheap (just a
/// reference count increment).
///
/// # Design
///
/// The cache is session-owned and lazy - it only loads and resamples audio when
/// requested. This allows for:
/// - Playback at DAW sample rate (e.g., 48kHz)
/// - Rendering at different sample rates (e.g., 44.1kHz for CD)
/// - Both using the same cache without duplicate work
///
/// # Examples
///
/// ```no_run
/// use daw_decode::AudioCache;
/// use std::path::Path;
///
/// let mut cache = AudioCache::new();
///
/// // Load audio at 48kHz for playback
/// let audio = cache.get_or_load(Path::new("kick.wav"), Some(48000)).unwrap();
///
/// // Later, load same file at 44.1kHz for rendering
/// // This reuses the decoded original and caches the 44.1kHz version
/// let audio_cd = cache.get_or_load(Path::new("kick.wav"), Some(44100)).unwrap();
/// ```
pub struct AudioCache {
    /// Original decoded audio (no resampling): file_hash -> AudioArc
    originals: HashMap<u64, AudioArc>,
    /// Resampled versions: (file_hash, target_rate) -> AudioArc
    resampled: HashMap<(u64, u32), AudioArc>,
    /// Map from file hash to resolved path for debugging
    paths: HashMap<u64, PathBuf>,
}

impl AudioCache {
    /// Create a new empty audio cache.
    pub fn new() -> Self {
        Self {
            originals: HashMap::new(),
            resampled: HashMap::new(),
            paths: HashMap::new(),
        }
    }

    /// Get audio from cache or load it from disk.
    ///
    /// This is the main entry point for loading audio. It:
    /// 1. Resolves the path using `resolve_sample_path()`
    /// 2. Checks if the original is cached, loads if not
    /// 3. If target_sample_rate is specified, checks if resampled version is cached
    /// 4. If not cached, resamples from the original and caches it
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the audio file (can be relative to samples root)
    /// * `target_sample_rate` - Optional target sample rate. If `None`, returns original.
    ///
    /// # Returns
    ///
    /// Returns an `AudioArc` which is cheap to clone. Multiple calls with the same
    /// parameters will return clones sharing the same underlying audio data.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use daw_decode::AudioCache;
    /// use std::path::Path;
    ///
    /// let mut cache = AudioCache::new();
    ///
    /// // First call: decodes from disk
    /// let audio1 = cache.get_or_load(Path::new("kick.wav"), Some(48000)).unwrap();
    ///
    /// // Second call: returns from cache (cheap clone)
    /// let audio2 = cache.get_or_load(Path::new("kick.wav"), Some(48000)).unwrap();
    /// ```
    pub fn get_or_load(
        &mut self,
        path: &Path,
        target_sample_rate: Option<u32>,
    ) -> anyhow::Result<AudioArc> {
        let resolved = resolve_sample_path(path)
            .ok_or_else(|| anyhow::anyhow!("sample not found: {}", path.display()))?;

        let hash = hash_path(&resolved);

        // Load original if not cached
        if !self.originals.contains_key(&hash) {
            let audio = decode_audio_arc_direct(&resolved, None)?;
            self.originals.insert(hash, audio);
            self.paths.insert(hash, resolved.clone());
        }

        let original = self.originals.get(&hash).unwrap();

        // If no target rate specified, return original
        let target_rate = match target_sample_rate {
            Some(rate) => rate,
            None => return Ok(original.clone()),
        };

        // If original is already at target rate, return it
        if original.sample_rate() == target_rate {
            return Ok(original.clone());
        }

        // Check if resampled version is cached
        let key = (hash, target_rate);
        if let Some(resampled) = self.resampled.get(&key) {
            return Ok(resampled.clone());
        }

        // Resample from original and cache it
        let resampled = original.resample(target_rate)?;
        self.resampled.insert(key, resampled.clone());
        Ok(resampled)
    }

    /// Clear all cached audio.
    ///
    /// This frees memory but requires re-decoding on next access.
    pub fn clear(&mut self) {
        self.originals.clear();
        self.resampled.clear();
        self.paths.clear();
    }

    /// Get the number of cached original audio files.
    pub fn originals_count(&self) -> usize {
        self.originals.len()
    }

    /// Get the number of cached resampled versions.
    pub fn resampled_count(&self) -> usize {
        self.resampled.len()
    }

    /// Get total cache entry count (originals + resampled).
    pub fn total_count(&self) -> usize {
        self.originals_count() + self.resampled_count()
    }

    /// Get cache statistics for debugging.
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            originals: self.originals_count(),
            resampled: self.resampled_count(),
            total: self.total_count(),
        }
    }
}

impl Default for AudioCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about the audio cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheStats {
    /// Number of original (non-resampled) audio files cached
    pub originals: usize,
    /// Number of resampled versions cached
    pub resampled: usize,
    /// Total number of cache entries
    pub total: usize,
}

/// Hash a file path for use as a cache key.
fn hash_path(path: &Path) -> u64 {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;
    use std::sync::Arc;
    use tempfile::TempDir;

    /// Helper: Create a test WAV file with a sine wave
    fn create_test_wav(
        path: &Path,
        frequency: f32,
        sample_rate: u32,
        duration_secs: f32,
        channels: u16,
    ) {
        let num_frames = (sample_rate as f32 * duration_secs) as usize;
        let mut samples = Vec::with_capacity(num_frames * channels as usize);

        for i in 0..num_frames {
            let t = i as f32 / sample_rate as f32;
            let sample = (2.0 * PI * frequency * t).sin() * 0.5; // 0.5 amplitude
            for _ in 0..channels {
                samples.push(sample);
            }
        }

        // Convert to i16 PCM
        let pcm_samples: Vec<i16> = samples.iter().map(|&s| (s * 32767.0) as i16).collect();

        // Write WAV file using hound
        let spec = hound::WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for &sample in &pcm_samples {
            writer.write_sample(sample).unwrap();
        }
        writer.finalize().unwrap();
    }

    #[test]
    fn test_decode_audio_arc_direct() {
        let temp_dir = TempDir::new().unwrap();
        let wav_path = temp_dir.path().join("test.wav");

        create_test_wav(&wav_path, 440.0, 44100, 0.1, 2);

        let audio = decode_audio_arc_direct(&wav_path, None).unwrap();

        assert_eq!(audio.sample_rate(), 44100);
        assert_eq!(audio.channels(), 2);
        assert!(audio.frames() > 0);
    }

    #[test]
    fn test_decode_audio_arc_with_resample() {
        let temp_dir = TempDir::new().unwrap();
        let wav_path = temp_dir.path().join("test.wav");

        create_test_wav(&wav_path, 440.0, 44100, 0.1, 2);

        // Decode and resample to 48kHz
        let audio = decode_audio_arc_direct(&wav_path, Some(48000)).unwrap();

        assert_eq!(audio.sample_rate(), 48000);
        assert_eq!(audio.channels(), 2);
    }

    #[test]
    fn test_audio_cache_basic() {
        let temp_dir = TempDir::new().unwrap();
        let wav_path = temp_dir.path().join("test.wav");

        create_test_wav(&wav_path, 440.0, 44100, 0.1, 2);

        let mut cache = AudioCache::new();

        // First load
        let audio1 = cache.get_or_load(&wav_path, Some(48000)).unwrap();
        assert_eq!(audio1.sample_rate(), 48000);
        assert_eq!(cache.originals_count(), 1);
        assert_eq!(cache.resampled_count(), 1);

        // Second load (should hit cache)
        let audio2 = cache.get_or_load(&wav_path, Some(48000)).unwrap();
        assert_eq!(audio2.sample_rate(), 48000);
        assert_eq!(cache.originals_count(), 1); // Still 1
        assert_eq!(cache.resampled_count(), 1); // Still 1

        // Should share the same Arc
        assert_eq!(Arc::strong_count(audio1.samples_arc()), 3); // cache + audio1 + audio2
    }

    #[test]
    fn test_audio_cache_multiple_rates() {
        let temp_dir = TempDir::new().unwrap();
        let wav_path = temp_dir.path().join("test.wav");

        create_test_wav(&wav_path, 440.0, 44100, 0.1, 2);

        let mut cache = AudioCache::new();

        // Load at 48kHz
        let audio_48k = cache.get_or_load(&wav_path, Some(48000)).unwrap();
        assert_eq!(audio_48k.sample_rate(), 48000);
        assert_eq!(cache.originals_count(), 1);
        assert_eq!(cache.resampled_count(), 1);

        // Load same file at 44.1kHz (should reuse original)
        let audio_44k = cache.get_or_load(&wav_path, Some(44100)).unwrap();
        assert_eq!(audio_44k.sample_rate(), 44100);
        assert_eq!(cache.originals_count(), 1); // Still 1
        assert_eq!(cache.resampled_count(), 1); // Still 1 (44.1k is original rate)

        // Load at 96kHz (new resampled version)
        let audio_96k = cache.get_or_load(&wav_path, Some(96000)).unwrap();
        assert_eq!(audio_96k.sample_rate(), 96000);
        assert_eq!(cache.originals_count(), 1); // Still 1
        assert_eq!(cache.resampled_count(), 2); // Now 2 (48k and 96k)
    }

    #[test]
    fn test_audio_cache_multiple_files() {
        let temp_dir = TempDir::new().unwrap();
        let wav1_path = temp_dir.path().join("test1.wav");
        let wav2_path = temp_dir.path().join("test2.wav");

        create_test_wav(&wav1_path, 440.0, 44100, 0.1, 2);
        create_test_wav(&wav2_path, 880.0, 44100, 0.1, 2);

        let mut cache = AudioCache::new();

        // Load file 1 at 48kHz
        let audio1 = cache.get_or_load(&wav1_path, Some(48000)).unwrap();
        assert_eq!(cache.originals_count(), 1);
        assert_eq!(cache.resampled_count(), 1);

        // Load file 2 at 48kHz
        let audio2 = cache.get_or_load(&wav2_path, Some(48000)).unwrap();
        assert_eq!(cache.originals_count(), 2);
        assert_eq!(cache.resampled_count(), 2);

        // Files should be different
        assert_ne!(audio1.samples().as_ptr(), audio2.samples().as_ptr());
    }

    #[test]
    fn test_audio_cache_original_at_target_rate() {
        let temp_dir = TempDir::new().unwrap();
        let wav_path = temp_dir.path().join("test.wav");

        create_test_wav(&wav_path, 440.0, 48000, 0.1, 2);

        let mut cache = AudioCache::new();

        // Request at same rate as file (48kHz)
        let audio = cache.get_or_load(&wav_path, Some(48000)).unwrap();
        assert_eq!(audio.sample_rate(), 48000);
        assert_eq!(cache.originals_count(), 1);
        assert_eq!(cache.resampled_count(), 0); // No resampling needed
    }

    #[test]
    fn test_audio_cache_no_target_rate() {
        let temp_dir = TempDir::new().unwrap();
        let wav_path = temp_dir.path().join("test.wav");

        create_test_wav(&wav_path, 440.0, 44100, 0.1, 2);

        let mut cache = AudioCache::new();

        // Request with no target rate (get original)
        let audio = cache.get_or_load(&wav_path, None).unwrap();
        assert_eq!(audio.sample_rate(), 44100);
        assert_eq!(cache.originals_count(), 1);
        assert_eq!(cache.resampled_count(), 0);
    }

    #[test]
    fn test_audio_cache_clear() {
        let temp_dir = TempDir::new().unwrap();
        let wav_path = temp_dir.path().join("test.wav");

        create_test_wav(&wav_path, 440.0, 44100, 0.1, 2);

        let mut cache = AudioCache::new();

        // Load some audio
        cache.get_or_load(&wav_path, Some(48000)).unwrap();
        assert_eq!(cache.total_count(), 2);

        // Clear cache
        cache.clear();
        assert_eq!(cache.total_count(), 0);
    }

    #[test]
    fn test_audio_cache_stats() {
        let temp_dir = TempDir::new().unwrap();
        let wav_path = temp_dir.path().join("test.wav");

        create_test_wav(&wav_path, 440.0, 44100, 0.1, 2);

        let mut cache = AudioCache::new();

        cache.get_or_load(&wav_path, Some(48000)).unwrap();
        cache.get_or_load(&wav_path, Some(96000)).unwrap();

        let stats = cache.stats();
        assert_eq!(stats.originals, 1);
        assert_eq!(stats.resampled, 2);
        assert_eq!(stats.total, 3);
    }

    #[test]
    fn test_audio_cache_repeated_loads_cheap() {
        let temp_dir = TempDir::new().unwrap();
        let wav_path = temp_dir.path().join("test.wav");

        create_test_wav(&wav_path, 440.0, 44100, 0.1, 2);

        let mut cache = AudioCache::new();

        // Load multiple times
        let audio1 = cache.get_or_load(&wav_path, Some(48000)).unwrap();
        let audio2 = cache.get_or_load(&wav_path, Some(48000)).unwrap();
        let audio3 = cache.get_or_load(&wav_path, Some(48000)).unwrap();

        // All should share the same Arc (cache + 3 clones = 4)
        assert_eq!(Arc::strong_count(audio1.samples_arc()), 4);
        assert_eq!(Arc::strong_count(audio2.samples_arc()), 4);
        assert_eq!(Arc::strong_count(audio3.samples_arc()), 4);
    }

    #[test]
    fn test_audio_cache_missing_file() {
        let mut cache = AudioCache::new();

        let result = cache.get_or_load(Path::new("nonexistent.wav"), Some(48000));
        assert!(result.is_err());
    }

    #[test]
    fn test_cache_stats_equality() {
        let stats1 = CacheStats {
            originals: 5,
            resampled: 10,
            total: 15,
        };
        let stats2 = CacheStats {
            originals: 5,
            resampled: 10,
            total: 15,
        };
        let stats3 = CacheStats {
            originals: 3,
            resampled: 10,
            total: 13,
        };

        assert_eq!(stats1, stats2);
        assert_ne!(stats1, stats3);
    }

    #[test]
    fn test_audio_cache_default() {
        let cache = AudioCache::default();
        assert_eq!(cache.total_count(), 0);
    }
}
