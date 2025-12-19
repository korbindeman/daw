# Project Crate (`daw_project`)

The project crate handles saving and loading DAW project files. Projects are serialized as JSON (`.dawproj` files).

## File Format

Projects are stored as JSON files containing:

| Field | Type | Description |
|-------|------|-------------|
| `name` | String | Project name |
| `tempo` | f64 | Tempo in BPM |
| `time_signature` | (u32, u32) | Time signature as (numerator, denominator) |
| `tracks` | Vec\<TrackData\> | List of tracks |

### TrackData

| Field | Type | Description |
|-------|------|-------------|
| `id` | u64 | Unique track identifier |
| `name` | String | Track name |
| `clips` | Vec\<ClipData\> | List of clips on the track |
| `volume` | f32 | Track volume (0.0 to 1.0) |
| `pan` | f32 | Track pan (-1.0 to 1.0) |
| `enabled` | bool | Whether track is enabled (not muted) |
| `solo` | bool | Whether track is soloed |

### ClipData

| Field | Type | Description |
|-------|------|-------------|
| `start_tick` | u64 | Start position in ticks (PPQN = 960) |
| `end_tick` | u64 | End position in ticks |
| `sample_ref` | SampleRef | Reference to the audio file (see below) |
| `audio_offset` | u64 | Offset into the audio in samples (for trimmed starts) |
| `name` | String | Display name for the clip |

## Audio Path Resolution (SampleRef)

Audio paths use a typed `SampleRef` enum for explicit path semantics:

```rust
enum SampleRef {
    /// Relative to {dev_root}/samples/
    DevRoot(PathBuf),

    /// Relative to the project file's directory
    ProjectRelative(PathBuf),
}
```

In the JSON file, this is serialized as:

```json
{
  "sample_ref": {
    "kind": "dev_root",
    "path": "cr78/kick-accent.wav"
  }
}
```

Resolution uses a `PathContext` that holds the root directories. See `docs/sample-refs.md` for full details.

## Project Metadata

For performance reasons, you can load just the project metadata without decoding audio files using `load_project_metadata()`. This is useful for displaying project information in file browsers or project lists.

### ProjectMetadata

| Field | Type | Description |
|-------|------|-------------|
| `name` | String | Project name |
| `tempo` | f64 | Tempo in BPM |
| `time_signature` | (u32, u32) | Time signature |
| `track_count` | usize | Number of tracks in the project |
| `clip_count` | usize | Total number of clips across all tracks |

## Usage

### Saving a Project

```rust
use daw_project::{save_project, SampleRef};
use daw_transport::{AudioArc, Track, TrackId, Clip, WaveformData};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

// Create example tracks with clips
let audio = AudioArc::new(vec![0.0; 44100 * 2], 44100, 2);
let waveform = Arc::new(WaveformData::from_audio_arc(&audio, 512));

let mut track = Track::new(TrackId(1), "Drums".to_string());
track.insert_clip(Clip {
    start_tick: 0,
    end_tick: 960,
    audio: audio.clone(),
    waveform: waveform.clone(),
    audio_offset: 0,
    name: "Kick".to_string(),
});
track.insert_clip(Clip {
    start_tick: 960,
    end_tick: 1920,
    audio: audio.clone(),
    waveform: waveform.clone(),
    audio_offset: 0,
    name: "Snare".to_string(),
});

let tracks = vec![track];

// Map clip names to their sample references
let mut sample_refs = HashMap::new();
sample_refs.insert(
    "Kick".to_string(),
    SampleRef::DevRoot(PathBuf::from("drums/kick.wav")),
);
sample_refs.insert(
    "Snare".to_string(),
    SampleRef::DevRoot(PathBuf::from("drums/snare.wav")),
);

save_project(
    Path::new("projects/my_song.dawproj"),
    "My Song".to_string(),
    120.0,           // tempo
    (4, 4),          // time signature
    &tracks,
    &sample_refs,
)?;
```

### Loading a Project

```rust
use daw_project::{load_project, PathContext};
use std::path::Path;

// Create a PathContext for resolving sample references
let project_path = Path::new("projects/my_song.dawproj");
let ctx = PathContext::from_project_path(project_path)
    .with_dev_root("/Users/me/dev/daw".into());

let project = load_project(project_path, &ctx)?;

println!("Project: {}", project.name);
println!("Tempo: {} BPM", project.tempo);
println!("Time Signature: {}/{}", project.time_signature.0, project.time_signature.1);
println!("Tracks: {}", project.tracks.len());

// Access loaded tracks (with decoded AudioArc buffers)
for track in &project.tracks {
    println!("Track '{}' (ID: {}) has {} clips", track.name, track.id.0, track.clips().len());
}

// Access sample references (for re-saving)
for (clip_name, sample_ref) in &project.sample_refs {
    println!("Clip '{}' -> {}", clip_name, sample_ref);
}

// Check for offline clips (missing audio files)
if !project.offline_clips.is_empty() {
    println!("Warning: {} clip(s) are offline:", project.offline_clips.len());
    for offline in &project.offline_clips {
        println!("  - {} ({}): {}", offline.name, offline.sample_ref, offline.error);
    }
}
```

### Loading Project Metadata Only

For better performance when you only need project information without audio:

```rust
use daw_project::load_project_metadata;
use std::path::Path;

let metadata = load_project_metadata(Path::new("projects/my_song.dawproj"))?;

println!("Project: {}", metadata.name);
println!("Tempo: {} BPM", metadata.tempo);
println!("Time Signature: {}/{}", metadata.time_signature.0, metadata.time_signature.1);
println!("Tracks: {}", metadata.track_count);
println!("Total Clips: {}", metadata.clip_count);
```

### LoadedProject

When loading a project, you receive a `LoadedProject` struct:

| Field | Type | Description |
|-------|------|-------------|
| `name` | String | Project name |
| `tempo` | f64 | Tempo in BPM |
| `time_signature` | (u32, u32) | Time signature |
| `tracks` | Vec\<Track\> | Tracks with decoded AudioArc buffers |
| `sample_refs` | HashMap\<String, SampleRef\> | Map of clip names to their sample references |
| `offline_clips` | Vec\<OfflineClip\> | Clips whose audio couldn't be loaded |

The `sample_refs` map preserves the original sample references from the project file, which is useful when re-saving the project.

### OfflineClip

When audio files are missing or fail to load, they appear in the `offline_clips` list:

| Field | Type | Description |
|-------|------|-------------|
| `name` | String | Clip name |
| `sample_ref` | SampleRef | The sample reference that failed |
| `error` | String | Description of the error |

## Error Handling

The crate uses `anyhow::Result` for error handling. Common errors include:

- File I/O errors (missing project file)
- JSON parse errors (corrupted project file)
- Audio decode errors (handled gracefully via `offline_clips`)

## Example: Complete Roundtrip

```rust
use daw_project::{load_project, save_project, PathContext};
use std::path::Path;

// Create path context
let ctx = PathContext::from_project_path(Path::new("projects/original.dawproj"))
    .with_dev_root("/Users/me/dev/daw".into());

// Load existing project
let project = load_project(Path::new("projects/original.dawproj"), &ctx)?;

// Modify tempo
let new_tempo = 140.0;

// Save as new project
save_project(
    Path::new("projects/remixed.dawproj"),
    format!("{} (Remixed)", project.name),
    new_tempo,
    project.time_signature,
    &project.tracks,
    &project.sample_refs,
)?;
```

## Testing

Run the project crate tests:

```bash
cargo test -p daw_project
```
