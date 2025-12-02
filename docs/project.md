# Project Crate (`daw_project`)

The project crate handles saving and loading DAW project files. Projects are serialized using MessagePack format (`.dawproj` files).

## File Format

Projects are stored as MessagePack-encoded binary files containing:

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
| `clips` | Vec\<ClipData\> | List of clips on the track |

### ClipData

| Field | Type | Description |
|-------|------|-------------|
| `id` | u64 | Unique clip identifier |
| `start` | u64 | Start position in ticks (PPQN = 960) |
| `audio_path` | PathBuf | Path to the audio file |

## Audio Path Resolution

Audio paths can be either absolute or relative:

- **Absolute paths** are used as-is
- **Relative paths** are resolved relative to the project file's directory

For example, if your project is at `projects/my_song.dawproj` and references `../samples/kick.wav`, the audio file will be loaded from `samples/kick.wav`.

## Usage

### Saving a Project

```rust
use daw_project::save_project;
use daw_transport::{Track, TrackId, Clip, ClipId};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

let tracks: Vec<Track> = /* your tracks */;

// Map clip IDs to their audio file paths
let mut audio_paths = HashMap::new();
audio_paths.insert(0, PathBuf::from("../samples/kick.wav"));
audio_paths.insert(1, PathBuf::from("../samples/snare.wav"));

save_project(
    Path::new("projects/my_song.dawproj"),
    "My Song".to_string(),
    120.0,           // tempo
    (4, 4),          // time signature
    &tracks,
    &audio_paths,
)?;
```

### Loading a Project

```rust
use daw_project::load_project;
use std::path::Path;

let project = load_project(Path::new("projects/my_song.dawproj"))?;

println!("Project: {}", project.name);
println!("Tempo: {} BPM", project.tempo);
println!("Time Signature: {}/{}", project.time_signature.0, project.time_signature.1);
println!("Tracks: {}", project.tracks.len());

// Access loaded tracks (with decoded audio buffers)
for track in &project.tracks {
    println!("Track {} has {} clips", track.id.0, track.clips.len());
}

// Access original audio paths (for re-saving)
for (clip_id, path) in &project.audio_paths {
    println!("Clip {} -> {:?}", clip_id, path);
}
```

### LoadedProject

When loading a project, you receive a `LoadedProject` struct:

| Field | Type | Description |
|-------|------|-------------|
| `name` | String | Project name |
| `tempo` | f64 | Tempo in BPM |
| `time_signature` | (u32, u32) | Time signature |
| `tracks` | Vec\<Track\> | Tracks with decoded audio buffers |
| `audio_paths` | HashMap\<u64, PathBuf\> | Map of clip IDs to original audio paths |

The `audio_paths` map preserves the original paths from the project file, which is useful when re-saving the project.

## Error Handling

The crate uses `ProjectError` for error handling:

```rust
pub enum ProjectError {
    Io(std::io::Error),           // File I/O errors
    Serialize(rmp_serde::encode::Error),   // Serialization errors
    Deserialize(rmp_serde::decode::Error), // Deserialization errors
    AudioDecode { path: PathBuf, source: anyhow::Error }, // Audio file decoding errors
}
```

## Example: Complete Roundtrip

```rust
use daw_project::{load_project, save_project};
use std::path::Path;

// Load existing project
let project = load_project(Path::new("projects/original.dawproj"))?;

// Modify tempo
let new_tempo = 140.0;

// Save as new project
save_project(
    Path::new("projects/remixed.dawproj"),
    format!("{} (Remixed)", project.name),
    new_tempo,
    project.time_signature,
    &project.tracks,
    &project.audio_paths,
)?;
```

## Testing

Run the project crate tests:

```bash
cargo test -p daw_project
```
