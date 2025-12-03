# Session & Engine Interaction

The DAW uses a lock-free architecture for real-time audio. The `Session` (in `daw_core`) manages state and communicates with the audio `Engine` (in `daw_engine`) without blocking the audio thread.

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

When tempo changes, call `update_tempo()`. This re-converts all clip positions to the new sample positions:

```rust
// Change tempo in session
session.time_context_mut().tempo = 140.0;

// Re-send tracks with new sample positions
session.update_tempo();
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
        session.time_context_mut().tempo = new_tempo;
        session.update_tempo();  // re-converts all positions
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
