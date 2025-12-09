# Audio Architecture

This document describes the audio data management system used throughout the DAW codebase.

## Overview

The audio system is built around two core abstractions:

- **AudioArc**: A reference-counted, immutable audio buffer with metadata
- **AudioCache**: A two-tier cache for decoded and resampled audio

These work together to provide efficient, zero-copy audio sharing across the application.

## AudioArc

`AudioArc` is an immutable, reference-counted audio buffer type inspired by the `imgref` crate. It separates sample data ownership from metadata, making clones cheap.

### Type Definition

```rust
/// Shared, immutable audio sample data
#[derive(Clone)]
pub struct AudioArc {
    samples: Arc<[f32]>,  // Shared sample data
    sample_rate: u32,     // Sample rate in Hz
    channels: u16,        // Number of channels (interleaved)
}
```

### Key Properties

- **Cheap cloning**: Cloning an `AudioArc` only increments the reference count on the shared sample data
- **Immutable**: Once created, the audio data cannot be modified
- **Metadata alongside data**: Sample rate and channel count are bundled with the samples
- **Small size**: Only 16-24 bytes (pointer + two integers)

### Core Methods

#### Construction

```rust
// Create from owned samples
let audio = AudioArc::new(
    vec![0.0, 0.1, 0.2, 0.3],  // Interleaved samples
    44100,                      // Sample rate
    2,                          // Channels (stereo)
);
```

#### Access

```rust
// Get sample data as slice
let samples: &[f32] = audio.samples();

// Get metadata
let rate = audio.sample_rate();
let channels = audio.channels();

// Get number of frames
let frames = audio.frames();  // samples.len() / channels

// Get duration
let duration = audio.duration();  // Duration in seconds
```

#### Channel Iteration

```rust
// Iterate over samples for a specific channel
for sample in audio.channel(0) {  // Left channel
    // Process sample
}
```

#### Resampling

```rust
// Resample to different sample rate
let resampled = audio.resample(48000)?;

// If already at target rate, returns cheap clone
let same = audio.resample(44100)?;  // Just clones if rate matches
```

### Usage Example

```rust
use daw_transport::AudioArc;

// Decode creates AudioArc
let audio = decode_audio_arc("sample.wav", None)?;

// Clone is cheap - just Arc refcount bump
let audio_clone = audio.clone();

// Both share the same sample data
assert_eq!(
    Arc::strong_count(audio.samples_arc()),
    2  // Original + clone
);

// Resample creates new AudioArc with new sample data
let resampled = audio.resample(48000)?;
```

## AudioCache

`AudioCache` is a two-tier cache that stores both original decoded audio and resampled versions. It ensures that files are decoded once and resampled once per target sample rate.

### Type Definition

```rust
pub struct AudioCache {
    /// Original decoded audio (no resampling)
    originals: HashMap<u64, AudioArc>,
    
    /// Resampled versions: (file_hash, target_rate) -> AudioArc
    resampled: HashMap<(u64, u32), AudioArc>,
    
    /// File paths for hash lookup
    paths: HashMap<u64, PathBuf>,
}
```

### Cache Strategy

**Tier 1 - Originals**: Stores audio at its original sample rate (decoded once)
**Tier 2 - Resampled**: Stores resampled versions keyed by `(file, target_rate)`

This allows:
- Multiple renders at different sample rates without re-decoding
- Session playback at engine sample rate separate from render sample rate
- Minimal memory overhead (AudioArc clones are cheap)

### Core Methods

#### Loading Audio

```rust
let mut cache = AudioCache::new();

// Load at original sample rate
let original = cache.get_or_load(path, None)?;

// Load resampled to 48kHz
let resampled = cache.get_or_load(path, Some(48000))?;

// Second call returns cached version (no re-decoding or re-resampling)
let cached = cache.get_or_load(path, Some(48000))?;
```

#### Cache Management

```rust
// Clear all cached audio
cache.clear();

// Get cache statistics
let stats = cache.stats();
println!("Originals: {}, Resampled: {}", 
    stats.original_count, 
    stats.resampled_count
);
```

### Usage Example

```rust
use daw_decode::AudioCache;

let mut cache = AudioCache::new();

// Load audio at engine sample rate for playback
let playback_audio = cache.get_or_load(
    "samples/kick.wav",
    Some(48000),  // Engine sample rate
)?;

// Later, render at CD sample rate
let render_audio = cache.get_or_load(
    "samples/kick.wav",
    Some(44100),  // CD sample rate
)?;

// Both operations used the same cached original,
// only resampling was done twice (once per rate)
```

## Integration with Core Types

### Segment

Segments store audio as `AudioArc`:

```rust
pub struct Segment {
    pub start_tick: u64,
    pub end_tick: u64,
    pub audio: AudioArc,        // Efficient reference-counted audio
    pub waveform: Arc<WaveformData>,
    pub audio_offset: u64,
    pub name: String,
}
```

### EngineClip

Engine clips also use `AudioArc`:

```rust
pub struct EngineClip {
    pub audio: AudioArc,        // Shared with segment
    pub start_sample: u64,
    pub offset_samples: u64,
}
```

### Session

Sessions own an `AudioCache` for managing loaded audio:

```rust
pub struct Session {
    cache: AudioCache,          // Manages all audio for this session
    tracks: Vec<Track>,
    // ... other fields
}
```

## Sample Rate Management

The system handles multiple sample rates efficiently:

### Session (Playback)

When creating a session, all audio is decoded and resampled to the engine's sample rate:

```rust
// Engine runs at 48kHz
let engine_rate = 48000;

// Load project audio through cache
for segment in &mut track.segments {
    segment.audio = cache.get_or_load(
        &segment.path,
        Some(engine_rate),
    )?;
}
```

### Render (Export)

Renders can use a different sample rate without re-decoding:

```rust
pub fn render_timeline(
    tracks: &[Track],
    sample_rate: u32,  // e.g., 44100 for CD
) -> AudioArc {
    // Segments already have audio at engine rate
    // Resample inline for render
    for segment in track.segments() {
        let audio = if segment.audio.sample_rate() == sample_rate {
            segment.audio.clone()  // Cheap if rates match
        } else {
            segment.audio.resample(sample_rate)?
        };
        // Use resampled audio for rendering
    }
}
```

## Decode Functions

The `daw_decode` crate provides functions for loading audio:

### decode_audio_arc

Main function for loading audio with optional resampling:

```rust
use daw_decode::decode_audio_arc;

// Load at original sample rate
let audio = decode_audio_arc("sample.wav", None)?;

// Load and resample to 48kHz
let audio = decode_audio_arc("sample.wav", Some(48000))?;
```

### decode_audio_arc_direct

Direct decoding without using a cache (useful for one-off loads):

```rust
use daw_decode::decode_audio_arc_direct;

let audio = decode_audio_arc_direct("sample.wav", Some(44100))?;
```

## Best Practices

### Do

✅ Use `AudioArc::clone()` freely - it's cheap (just a refcount increment)
✅ Use `AudioCache` for session/project audio to avoid redundant decoding
✅ Resample at decode time when you know the target rate
✅ Use `audio.resample()` for inline resampling when needed

### Don't

❌ Convert `AudioArc` to owned data unless necessary
❌ Decode the same file multiple times - use the cache
❌ Resample repeatedly - cache resampled versions
❌ Modify sample data - `AudioArc` is immutable by design

## Performance Characteristics

### AudioArc Clone

- **Time**: O(1) - just increment refcount
- **Memory**: 16-24 bytes (Arc pointer + metadata)
- **Allocation**: None

### AudioArc Resample

- **Time**: O(n) where n = number of output samples
- **Memory**: Allocates new sample buffer at target rate
- **Allocation**: Single allocation for resampled buffer

### Cache Hit

- **Time**: O(1) hash lookup + O(1) Arc clone
- **Memory**: No new allocation
- **Disk I/O**: None

### Cache Miss

- **Time**: O(n) file decode + optional O(m) resample
- **Memory**: Allocates original buffer + optional resampled buffer
- **Disk I/O**: One file read

## Example: Loading a Project

```rust
use daw_decode::AudioCache;
use daw_core::Session;

fn load_project(path: &Path, engine_sample_rate: u32) -> Result<Session> {
    // Load project metadata
    let project = daw_project::load_project(path)?;
    
    // Create cache for this session
    let mut cache = AudioCache::new();
    
    // Build tracks with cached audio
    let mut tracks = Vec::new();
    for track_data in project.tracks {
        let mut track = Track::new(
            TrackId(track_data.id),
            track_data.name,
        );
        
        for segment_data in track_data.segments {
            // Load audio at engine rate (cached)
            let audio = cache.get_or_load(
                &segment_data.audio_path,
                Some(engine_sample_rate),
            )?;
            
            // Create waveform from audio
            let waveform = WaveformData::from_audio_arc(&audio, 512);
            
            // Add segment to track
            track.insert_segment(Segment {
                start_tick: segment_data.start_tick,
                end_tick: segment_data.end_tick,
                audio,
                waveform: Arc::new(waveform),
                audio_offset: segment_data.audio_offset,
                name: segment_data.name,
            });
        }
        
        tracks.push(track);
    }
    
    // Create session with cached audio
    Session::new_with_cache(tracks, project.tempo, cache)
}
```

## Summary

The audio architecture provides:

- **Efficient sharing** through cheap `AudioArc` clones
- **Minimal decoding** with `AudioCache` two-tier strategy  
- **Flexible resampling** at decode time or inline as needed
- **Clean separation** between data ownership and metadata
- **Zero-copy design** where sample data is shared via Arc

This design eliminates redundant decoding and resampling while keeping the API simple and the memory footprint low.
