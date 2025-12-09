# Session & Engine Interaction

The DAW uses a lock-free architecture for real-time audio. The `Session` (in `daw_core`) manages state and communicates with the audio `Engine` (in `daw_engine`) without blocking the audio thread.

## What is Session?

The **Session** is the central abstraction layer and primary interface for frontend code. It encapsulates:

- **Project state**: Tracks, tempo, time signature, metronome settings
- **Audio engine coordination**: Lock-free communication with the real-time audio thread
- **Time conversion**: Translates between musical time (ticks) and physical time (samples)
- **Project persistence**: Save/load functionality
- **Automatic synchronization**: All modifications automatically update the engine

### Design Philosophy

Session follows these key principles:

1. **Single Source of Truth**: All project state lives in Session - frontends never touch the engine directly
2. **Musical Time Abstraction**: Frontend works in ticks (tempo-aware), engine works in samples (tempo-agnostic)
3. **Automatic Synchronization**: Methods like `set_tempo()` automatically recalculate and update the engine
4. **Lock-Free Safety**: No locks or blocking calls - safe for real-time audio
5. **Simple API**: High-level methods hide complexity (e.g., `session.play()` vs manual queue pushing)

### Why Session Exists

Without Session, frontends would need to:
- Manually convert ticks ↔ samples based on tempo and sample rate
- Directly manage lock-free queues and basedrop handles
- Remember to update the engine after every state change
- Handle project serialization themselves
- Coordinate multiple state changes atomically

Session does all of this automatically, providing a clean, safe API.

## How to Use Session

### Creating a Session

```rust
use daw_core::{Session, TimeSignature, Track};

// Create a new empty session
let mut session = Session::new(vec![], 120.0, (4, 4))?;

// Or load from a project file
let mut session = Session::from_project(Path::new("my_song.dawproj"))?;
```

### Basic Playback Control

```rust
// Start playback
session.play();

// Pause (maintains position)
session.pause();

// Stop (returns to beginning)
session.stop();

// Seek to a specific tick
session.seek(1920);  // 1920 ticks (4 beats at 480 PPQN)

// Check playback state
if session.is_playing() {
    println!("Currently playing at tick {}", session.current_tick());
}
```

### Polling (Required!)

Session **must** be polled regularly (60 Hz recommended) to:
1. Get position updates from the audio engine
2. Free memory from old track data (basedrop garbage collection)

```rust
// In your main loop, every ~16ms:
if let Some(tick) = session.poll() {
    // Position changed - update UI
    update_playhead(tick);
}
```

**Important**: Forgetting to poll will:
- Prevent position updates
- Leak memory (old track data won't be freed)
- Make the UI appear frozen during playback

### Modifying Session State

All modifications automatically sync with the engine:

```rust
// Change tempo
session.set_tempo(140.0);  // Automatically recalculates sample positions

// Change time signature
session.set_time_signature(TimeSignature::new(3, 4));

// Control tracks
session.set_tracks(new_tracks);         // Replace all tracks
session.add_segment(track_id, segment); // Add a clip to a track
session.set_track_volume(0, 0.75);      // Set track volume (0-1)
session.toggle_track_enabled(0);        // Mute/unmute track

// Metronome
session.toggle_metronome();             // Enable/disable metronome
session.set_metronome_volume(0.5);      // Set metronome volume
```

### Project Management

```rust
// Save project
session.set_name("My Song".to_string());
session.save(Path::new("projects/my_song.dawproj"))?;

// Save in place (if loaded from file)
session.save_in_place()?;

// Render to WAV
session.render_to_file(Path::new("output.wav"))?;

// Access project info
println!("Project: {}", session.name());
println!("Tempo: {} BPM", session.tempo());
println!("Time Sig: {:?}", session.time_signature());
```

### Reading Session State

```rust
// Get current state (read-only)
let tempo = session.tempo();
let time_sig = session.time_signature();
let tracks = session.tracks();  // &[Track]
let sample_rate = session.sample_rate();

// Access time context for conversions
let time_ctx = session.time_context();
let pixels = time_ctx.ticks_to_pixels(tick);
```

### Complete Example: Main Loop

```rust
use daw_core::Session;
use std::time::{Duration, Instant};

let mut session = Session::from_project("song.dawproj")?;
session.play();

let mut last_poll = Instant::now();

loop {
    // Poll at 60 Hz
    if last_poll.elapsed() >= Duration::from_millis(16) {
        if let Some(tick) = session.poll() {
            update_ui_position(tick);
        }
        last_poll = Instant::now();
    }

    // Handle user events
    if user_clicked_play_button {
        if session.is_playing() {
            session.stop();
        } else {
            session.play();
        }
    }

    if let Some(new_tempo) = user_changed_tempo {
        session.set_tempo(new_tempo);
    }

    // ... handle other UI events
}
```

## Session API Summary

### Construction
- `Session::new(tracks, tempo, time_sig)` - Create new session
- `Session::from_project(path)` - Load from file

### Playback Control
- `play()` - Start playback
- `pause()` - Pause (maintain position)
- `stop()` - Stop and reset to beginning
- `seek(tick)` - Jump to position
- `poll()` - **Must call at 60 Hz** - Returns position updates

### State Queries
- `is_playing()` - Check if playing
- `current_tick()` - Get current position
- `tempo()` - Get tempo
- `time_signature()` - Get time signature
- `tracks()` - Get tracks slice
- `sample_rate()` - Get engine sample rate

### State Modification
- `set_tempo(bpm)` - Change tempo (auto-updates engine)
- `set_time_signature(sig)` - Change time sig (auto-updates)
- `set_tracks(tracks)` - Replace all tracks
- `add_segment(id, segment)` - Add clip to track
- `set_track_volume(id, vol)` - Set track volume
- `toggle_track_enabled(id)` - Mute/unmute track

### Metronome
- `toggle_metronome()` - Enable/disable
- `set_metronome_volume(vol)` - Set volume
- `metronome_enabled()` - Check if enabled

### Project Management
- `save(path)` - Save to file
- `save_in_place()` - Save to current path
- `render_to_file(path)` - Export to WAV
- `name()` / `set_name()` - Project name

## Architecture Overview

```
┌─────────────────────────────────────────┐
│              UI Thread                  │
│         (App / Session)                 │
│                                         │
│   Stores: Ticks (musical time)          │
│   Converts: Ticks → Samples             │
└──────────────────┬──────────────────────┘
                   │
         ┌─────────┴─────────┐
         │                   │
    rtrb queues         basedrop
    (Commands)       (EngineTrack)
         │                   │
         └─────────┬─────────┘
                   ▼
┌─────────────────────────────────────────┐
│            Audio Thread                 │
│              (Engine)                   │
│                                         │
│   Works in: Samples only                │
│   No tempo/BPM knowledge                │
└─────────────────────────────────────────┘
```

## Key Design Principle: Sample-Based Engine

The engine is **tempo-agnostic**. It only understands sample positions:

- **Core** stores clip positions in **ticks** (musical time, tempo-independent)
- **Core** converts ticks → samples before sending to engine
- **Engine** receives `EngineTrack`/`EngineClip` with **sample positions**
- **Engine** reports position back in **samples**
- **Core** converts samples → ticks for UI display

When tempo changes, core re-converts all positions and sends updated tracks to the engine.

## Data Types

### Core (Musical Time)
```rust
// daw_transport
struct Clip {
    start: u64,  // ticks
    audio: Arc<AudioBuffer>,
}

struct Track {
    clips: Vec<Clip>,
}
```

### Engine (Sample Time)
```rust
// daw_engine
struct EngineClip {
    start: u64,  // samples
    audio: Arc<AudioBuffer>,
}

struct EngineTrack {
    clips: Vec<EngineClip>,
}
```

## Communication Channels

| Channel | Direction | Type | Purpose |
|---------|-----------|------|---------|
| `commands` | UI → Engine | `rtrb` queue | Play, Pause, Seek (in samples) |
| `status` | Engine → UI | `rtrb` queue | Position updates (in samples) |
| `tracks` | UI → Engine | `rtrb` + basedrop | Track/clip updates |

## Updating the Engine

### Playback Control

```rust
session.play();   // sends EngineCommand::Play
session.pause();  // sends EngineCommand::Pause
session.stop();   // pause + seek to sample 0

// Seek to tick - Session converts to samples internally
session.seek(tick);
```

### Updating Tracks

When tracks or clips change, call `update_tracks()`:

```rust
// Modify tracks in session (tick-based)
session.tracks_mut().push(new_track);

// Send to engine (converted to samples, lock-free)
session.update_tracks();
```

### Updating Tempo

When tempo changes, call `set_tempo()`. This updates the tempo and automatically re-sends tracks with new sample positions:

```rust
// Set tempo - automatically updates the engine
session.set_tempo(140.0);
```

You can also change the time signature:

```rust
session.set_time_signature(TimeSignature::new(3, 4));
```

## Polling

The session must be polled periodically to:

1. **Collect garbage**: basedrop defers deallocation. `poll()` calls `collector.collect()`.
2. **Read status**: Get current position (in samples), convert to ticks.

```rust
// Returns Some(tick) if position changed
if let Some(tick) = session.poll() {
    update_playhead_ui(tick);
}
```

### Polling Frequency

**Recommended: 60 Hz (every ~16ms)**

This matches typical display refresh rates and provides:
- Smooth playhead animation
- Responsive UI feedback
- Timely garbage collection

Example setup with GPUI:

```rust
cx.spawn(async move {
    loop {
        Timer::after(Duration::from_millis(16)).await;
        session_entity.update(cx, |session, cx| {
            session.poll();
            cx.notify();
        });
    }
});
```

## Memory Management with Basedrop

[basedrop](https://github.com/micahrj/basedrop) provides real-time safe shared ownership:

- `Shared<T>`: Like `Arc<T>` but defers deallocation
- `Collector`: Collects dropped `Shared` values for later deallocation
- `Handle`: Used to create new `Shared` values

When you send new tracks to the engine:

1. Session converts `Track` → `EngineTrack` (ticks → samples)
2. New `Shared<Vec<EngineTrack>>` is created via the `Handle`
3. Sent through `rtrb` queue (lock-free)
4. Audio thread swaps in new value, drops old `Shared`
5. Old data is queued for collection (not freed yet)
6. Next `poll()` calls `collector.collect()` to actually free memory

## Complete Example

```rust
use daw_core::Session;

// Create session (tracks use tick positions)
let mut session = Session::new(tracks, 120.0, (4, 4))?;

// Start playback
session.play();

// Main loop (60 Hz):
loop {
    // Poll for position updates (returns ticks) and garbage collection
    if let Some(tick) = session.poll() {
        update_playhead_ui(tick);
    }

    // Handle user input
    if user_changed_tempo {
        session.set_tempo(new_tempo);  // automatically re-converts all positions
    }

    if user_added_clip {
        // Modify tracks (tick-based)...
        session.update_tracks();  // converts to samples, sends to engine
    }

    sleep(Duration::from_millis(16));
}
```

## Thread Safety Summary

| Operation | Thread Safe? | Blocks Audio? |
|-----------|--------------|---------------|
| `play()` / `pause()` / `stop()` | Yes | No |
| `seek(tick)` | Yes | No |
| `update_tracks()` | Yes | No |
| `update_tempo()` | Yes | No |
| `poll()` | No (call from UI thread only) | No |

## Sample Rate

The engine exposes its sample rate (from CPAL) via `session.sample_rate()`. This is used internally for tick↔sample conversion but is also available if needed:

```rust
let sample_rate = session.sample_rate();  // e.g., 44100 or 48000
```
