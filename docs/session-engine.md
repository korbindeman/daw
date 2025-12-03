# Session & Engine Interaction

The DAW uses a lock-free architecture for real-time audio. The `Session` (in `daw_core`) manages state and communicates with the audio `Engine` (in `daw_engine`) without blocking the audio thread.

## Architecture Overview

```
┌─────────────────────────────────────────┐
│              UI Thread                  │
│         (App / Session)                 │
└──────────────────┬──────────────────────┘
                   │
         ┌─────────┴─────────┐
         │                   │
    rtrb queues         basedrop
    (Commands)       (Tracks, Tempo)
         │                   │
         └─────────┬─────────┘
                   ▼
┌─────────────────────────────────────────┐
│            Audio Thread                 │
│              (Engine)                   │
└─────────────────────────────────────────┘
```

## Communication Channels

| Channel | Direction | Type | Purpose |
|---------|-----------|------|---------|
| `commands` | UI → Engine | `rtrb` queue | Play, Pause, Seek |
| `status` | Engine → UI | `rtrb` queue | Position updates |
| `tracks` | UI → Engine | `rtrb` + basedrop | Track/clip updates |
| `tempo` | UI → Engine | `rtrb` + basedrop | Tempo changes |

## Updating the Engine

### Playback Control

Simple commands use the `rtrb` queue directly:

```rust
// Play
session.play();

// Pause
session.pause();

// Stop (pause + seek to 0)
session.stop();

// Seek to tick position
session.seek(tick);
```

### Updating Tracks

When tracks or clips change (add, remove, move, trim), call `update_tracks()`:

```rust
// Modify tracks in session
session.tracks_mut().push(new_track);
// or modify existing clips...

// Send to engine (lock-free)
session.update_tracks();
```

This creates a new `Shared<Vec<Track>>` via basedrop and sends it through the queue. The audio thread swaps it in without blocking.

### Updating Tempo

When tempo changes, call `update_tempo()`:

```rust
// Change tempo in session
session.time_context_mut().tempo = 140.0;

// Send to engine (lock-free)
session.update_tempo();
```

## Polling

The session must be polled periodically to:

1. **Collect garbage**: basedrop defers deallocation to avoid blocking the audio thread. `poll()` calls `collector.collect()` to free old data.
2. **Read status**: Get the current playback position from the engine.

```rust
// Returns Some(tick) if position changed, None otherwise
if let Some(tick) = session.poll() {
    // Update UI playhead position
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
        // Update session entity, which calls poll()
        session_entity.update(cx, |session, cx| {
            session.poll();
            cx.notify();
        });
    }
});
```

### Why Polling Matters

- **Too infrequent** (e.g., 200ms): Playhead jumps, delayed garbage collection may cause memory buildup
- **Too frequent** (e.g., 1ms): Wastes CPU cycles with no visual benefit
- **60 Hz**: Good balance for smooth UI without excessive overhead

## Memory Management with Basedrop

[basedrop](https://github.com/micahrj/basedrop) provides real-time safe shared ownership:

- `Shared<T>`: Like `Arc<T>` but defers deallocation
- `Collector`: Collects dropped `Shared` values for later deallocation
- `Handle`: Used to create new `Shared` values

When you send new tracks/tempo to the engine:

1. New `Shared<T>` is created via the `Handle`
2. Sent through `rtrb` queue (lock-free)
3. Audio thread swaps in new value, drops old `Shared`
4. Old data is queued for collection (not freed yet)
5. Next `poll()` calls `collector.collect()` to actually free memory

This ensures the audio thread never blocks on allocation or deallocation.

## Complete Example

```rust
use daw_core::Session;

// Create session
let mut session = Session::new(tracks, 120.0, (4, 4))?;

// Start playback
session.play();

// In your main loop (60 Hz):
loop {
    // Poll for position updates and garbage collection
    if let Some(tick) = session.poll() {
        update_playhead_ui(tick);
    }

    // Handle user input
    if user_changed_tempo {
        session.time_context_mut().tempo = new_tempo;
        session.update_tempo();
    }

    if user_added_clip {
        // Modify tracks...
        session.update_tracks();
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
