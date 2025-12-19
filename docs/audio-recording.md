# Audio Recording

> ⚠️ **STATUS: PLANNED / NOT YET IMPLEMENTED**
>
> This document describes the *planned* audio recording system. The features described here are part of the roadmap but have **not yet been implemented** in the codebase. This serves as a design document for future development.

This document describes the audio recording system, including track arming, monitoring modes, count-in functionality, and the recording state machine.

## Overview

The recording system will allow users to record audio from their input device (e.g., microphone or audio interface) onto armed tracks. Planned features include:

- **Track Arming**: Select which track receives the recorded audio
- **Monitoring Modes**: Listen to input with different monitoring behaviors (In/Auto/Off)
- **Count-in**: 1-bar metronome count-in before recording starts
- **Real-time Capture**: Record while other tracks play (overdubbing)
- **Sample-accurate**: Recordings are precisely aligned to the timeline

## Architecture

### Components

The recording system will span three main crates:

1. **transport** - Track arming state and monitoring modes
2. **engine** - Real-time audio capture via CPAL input stream
3. **core** - Recording state machine and clip finalization

### Data Flow

```
Input Device → CPAL Input Stream → RecordedChunks → rtrb Queue → Session → AudioArc → Clip
                                                    ↓
                                         (monitoring) → Output Stream → Speakers
```

## Track Arming

### MonitorMode Enum

Will be defined in `crates/transport/src/lib.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonitorMode {
    Off,   // No input monitoring
    In,    // Always monitor input
    Auto,  // Monitor when armed or recording
}
```

**Mode Behaviors:**

- **Off**: Input is never routed to output (useful for preventing feedback)
- **In**: Input always passes through to output (like Ableton's "In" mode)
- **Auto**: Input monitored only when track is armed or recording (most common)

### Track Fields

```rust
pub struct Track {
    // ... existing fields ...
    pub armed: bool,              // Track is ready to record
    pub monitoring: MonitorMode,  // How to handle input monitoring
}
```

### Single-Track Arming Constraint

Only one track will be armable at a time. This will be enforced by `Session::arm_track()`:

```rust
pub fn arm_track(&mut self, track_id: u64) {
    // Disarm all tracks
    for track in &mut self.tracks {
        track.armed = false;
    }
    
    // Arm the specified track
    if let Some(track) = self.tracks.iter_mut().find(|t| t.id.0 == track_id) {
        track.armed = true;
    }
    
    self.update_tracks();
}
```

## Recording State Machine

Will be defined in `crates/core/src/session.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingState {
    Idle,
    CountIn { 
        start_tick: u64,
        end_tick: u64,
    },
    Recording { 
        start_tick: u64,
    },
}
```

### State Transitions

```
       start_recording()
Idle ──────────────────────> CountIn
                                │
                                │ (1 bar elapsed)
                                ↓
                             Recording
                                │
                                │ stop_recording()
                                ↓
                              Idle
```

### State Descriptions

**Idle**
- Default state when not recording
- No audio capture happening
- User can arm tracks and adjust monitoring

**CountIn**
- Triggered when user presses record button
- Playback starts if stopped
- Metronome plays at 1.2x volume for 1 bar
- Duration: 1 bar based on time signature (e.g., 4 beats at 4/4)
- Transitions to Recording when `current_tick >= end_tick`

**Recording**
- Engine captures audio from input device
- Input samples sent as chunks via rtrb queue
- Other tracks continue playing (overdubbing)
- Monitoring respects armed track's MonitorMode
- Ends when user presses stop or record button again

## Engine Recording System

### RecordedChunk

Will be defined in `crates/engine/src/lib.rs`:

```rust
pub struct RecordedChunk {
    pub samples: Vec<f32>,      // Interleaved samples
    pub start_position: u64,     // Timeline position in samples
    pub channels: u16,           // Number of input channels
}
```

Chunks are sent from the input callback to Session via lock-free queue.

**Chunk Size**: 8192 samples (~185ms at 44.1kHz)
- Balances latency vs queue pressure
- Large enough to minimize queue overhead
- Small enough for responsive finalization

### Input Stream

The engine creates a separate CPAL input stream alongside the output stream:

**Input Callback Responsibilities:**
1. Read samples from input device buffer
2. Accumulate in local 8192-sample buffer
3. Read current playback position from shared `Arc<AtomicU64>`
4. When buffer full, create RecordedChunk with timestamp
5. Send chunk via rtrb queue to Session
6. Send copy to monitoring queue (if monitoring enabled)

**Sample Rate Conversion:**
If input device sample rate differs from output, the input callback uses a `rubato` resampler to convert in real-time.

### Engine Commands

```rust
pub enum EngineCommand {
    Play,
    Pause,
    Seek { sample: u64 },
    StartRecording,    // Begin capturing audio
    StopRecording,     // Stop capturing, send RecordingFinished
}
```

### Engine Status

```rust
pub enum EngineStatus {
    Position(u64),
    RecordedChunk(RecordedChunk),  // Chunk of recorded audio
    RecordingFinished,              // Recording stopped, ready to finalize
}
```

## Count-in Implementation

### Trigger

When `Session::start_recording()` is called:

1. Calculate current bar position
2. Calculate end of count-in: `current_tick + ticks_per_bar`
3. Set state to `CountIn { start_tick, end_tick }`
4. Start playback if stopped
5. Enable metronome

### Detection

In `Session::poll()`:

```rust
if let RecordingState::CountIn { end_tick, .. } = self.recording_state {
    if self.position >= end_tick {
        // Count-in complete, start recording
        self.recording_state = RecordingState::Recording {
            start_tick: end_tick,
        };
        self.engine.commands.push(EngineCommand::StartRecording).ok();
    }
}
```

### Metronome Enhancement

The existing `generate_metronome_track()` is enhanced to play during count-in:

```rust
fn generate_metronome_track(&mut self, sample_rate: u32) -> Option<EngineTrack> {
    if !self.metronome.enabled && !matches!(self.recording_state, RecordingState::CountIn { .. }) {
        return None;
    }
    
    let volume = if matches!(self.recording_state, RecordingState::CountIn { .. }) {
        self.metronome.volume * 1.2  // 20% louder during count-in
    } else {
        self.metronome.volume
    };
    
    // ... rest of existing implementation ...
}
```

This ensures the count-in is clearly audible even if the user's metronome is normally disabled.

## Recording Finalization

### Chunk Collection

During recording, Session collects chunks in `Session::poll()`:

```rust
while let Ok(EngineStatus::RecordedChunk(chunk)) = self.engine.status.pop() {
    self.recorded_chunks.push(chunk);
}
```

### Finalization Trigger

When `EngineStatus::RecordingFinished` is received:

```rust
if let Ok(EngineStatus::RecordingFinished) = self.engine.status.pop() {
    self.finalize_recording();
    self.recording_state = RecordingState::Idle;
}
```

### Finalization Process

`Session::finalize_recording()` performs these steps:

1. **Sort chunks** by `start_position` (should already be sorted, but defensive)
2. **Concatenate samples** from all chunks into a single `Vec<f32>`
3. **Create AudioArc** from concatenated samples
4. **Generate waveform** using `WaveformData::from_audio_arc()`
5. **Calculate timeline positions**:
   - Convert first chunk's `start_position` (samples) to `start_tick`
   - Calculate `end_tick` based on audio duration
6. **Create Clip** with timestamp-based name (e.g., "Recording 14:32:05")
7. **Insert into armed track** via `track.insert_clip(clip)`
   - Existing clips in the time range are trimmed/replaced
8. **Update engine** with new track state via `self.update_tracks()`
9. **Clear temporary data** (`recorded_chunks.clear()`)

### Clip Insertion Behavior

The existing `Track::insert_clip()` handles overlapping clips:
- New clip trims/splits existing clips in its time range
- Maintains non-overlapping invariant
- Preserves clips outside the recording range

This means recordings automatically replace existing audio in the same time region.

## Input Monitoring

### Monitoring Queue

A separate rtrb queue passes audio from input callback to output callback:

```
Input Callback → monitoring_queue → Output Callback → Mixed into output
```

### Monitoring Logic

In the output callback:

```rust
let should_monitor = match (monitoring_mode, is_armed, is_recording) {
    (MonitorMode::Off, _, _) => false,
    (MonitorMode::In, _, _) => true,
    (MonitorMode::Auto, true, _) => true,
    (MonitorMode::Auto, _, true) => true,
    (MonitorMode::Auto, false, false) => false,
};

if should_monitor {
    if let Ok(monitor_samples) = monitor_queue.pop() {
        for (i, sample) in monitor_samples.iter().enumerate() {
            if i < output_frame.len() {
                output_frame[i] += sample * monitor_gain;
            }
        }
    }
}
```

### Latency

Monitoring latency = input_buffer_size + output_buffer_size

**Example**: 512 samples each @ 44.1kHz = ~23ms round-trip
- Acceptable for overdubbing
- Low enough to feel responsive
- High enough for stable performance

## Session API

### Methods

**arm_track(track_id: u64)**
- Arms specified track for recording
- Disarms all other tracks (single-track constraint)
- Updates engine with new track state

**start_recording()**
- Enters CountIn state if track is armed
- Starts playback if stopped
- Enables metronome for count-in
- After 1 bar, automatically transitions to Recording

**stop_recording()**
- Sends StopRecording command to engine
- Engine responds with RecordingFinished status
- Triggers finalize_recording() in next poll()

**is_recording() -> bool**
- Returns true if in Recording state
- Used by UI to show recording indicator

**armed_track_id() -> Option<u64>**
- Returns ID of currently armed track
- None if no track is armed

### Example Usage

```rust
// Arm track for recording
session.arm_track(track_id);

// Set monitoring mode
if let Some(track) = session.tracks.iter_mut().find(|t| t.id.0 == track_id) {
    track.monitoring = MonitorMode::Auto;
}
session.update_tracks();

// Start recording (begins count-in)
session.start_recording();

// ... user records audio ...

// Stop recording (finalizes and creates clip)
session.stop_recording();
```

## Error Handling

### No Input Device

If no input device is available, input stream creation fails gracefully:
- Recording button disabled in UI
- Error message shown to user
- Playback continues to work normally

### Buffer Overflow

If recorded chunks queue fills up (128 chunks ≈ 24 seconds):
- Input callback drops new chunks
- Sets error flag
- Session shows warning but continues recording
- User doesn't lose all progress

### Sample Rate Mismatch

If input and output sample rates differ:
- Rubato resampler converts input to output rate
- Resampling happens in input callback (real-time)
- Transparent to user
- May add ~1ms latency

### Device Disconnection

If input device disconnects during recording:
- CPAL error callback triggered
- Session automatically stops recording
- Finalizes whatever was captured
- Shows error notification

## Performance Considerations

### Memory Usage

**During Recording:**
- Local buffer: 8192 samples × 4 bytes = 32KB
- Queue capacity: 128 chunks × 32KB = 4MB max
- Acceptable for modern systems

**After Recording:**
- Chunks concatenated into single AudioArc
- Original chunks dropped
- Memory footprint: audio_duration × sample_rate × channels × 4 bytes

### CPU Usage

**Input Callback:**
- Sample buffer copy: ~O(n) where n = buffer size
- Optional resampling: ~O(n × quality)
- Queue push: O(1) lock-free
- Total: Very low, runs on audio thread priority

**Output Callback:**
- Monitoring mix: O(buffer_size)
- Only when monitoring enabled
- Negligible impact

**Finalization:**
- Concatenation: O(total_samples)
- Waveform generation: O(total_samples)
- Happens off audio thread
- One-time cost

### Real-time Safety

**Audio Thread (input/output callbacks):**
- ✅ No allocations
- ✅ No locks
- ✅ Lock-free queues only
- ✅ Pre-allocated buffers
- ✅ Bounded execution time

**Main Thread (Session):**
- ❌ Allocations allowed
- ❌ Can use locks
- Finalization happens here (safe)

## Future Enhancements

### Multiple Armed Tracks
- Allow recording to multiple tracks simultaneously
- Each track gets its own RecordedChunk queue
- Useful for recording full band simultaneously

### Punch In/Out
- Start/stop recording at specific timeline positions
- Set in/out points before recording
- Auto-start and auto-stop

### Loop Recording
- Record multiple takes while looping
- Each take creates a new lane/layer
- User can comp best parts from each take

### Configurable Count-in
- UI setting for count-in duration (0-4 bars)
- Option to disable count-in
- Different count-in sounds

### Input Device Selection
- UI dropdown to select input device
- Support multiple input devices
- Per-track input routing

### Recording to Disk
- Stream recording directly to WAV file
- Useful for long recordings
- Requires project-as-directory structure

### Latency Compensation
- Measure round-trip latency
- Automatically shift recorded clips earlier
- Compensates for monitoring delay

### Pre-roll Recording
- Start capturing audio before count-in
- User can trim to exact start point
- Prevents missing the beginning

## Dependencies

### External Crates

- **cpal**: Cross-platform audio I/O (input stream)
- **rtrb**: Lock-free ring buffers (chunk/monitoring queues)
- **rubato**: High-quality sample rate conversion
- **chrono**: Timestamp generation for clip names

### Internal Crates

- **transport**: AudioArc, Track, Clip, WaveformData
- **engine**: CPAL integration, real-time audio thread
- **core**: Session, time conversion, state machine

## Testing Strategy

### Unit Tests

- RecordingState transitions
- MonitorMode logic
- Chunk concatenation
- Time conversion (samples ↔ ticks)

### Integration Tests

- Record 1 second of silence
- Verify clip created with correct duration
- Check clip positioned at correct tick
- Verify existing clips trimmed correctly

### Manual Testing

- Record with different time signatures
- Test all monitoring modes (Off/In/Auto)
- Record while other tracks playing
- Test count-in at different tempos
- Disconnect device during recording
- Record with mismatched sample rates

## Known Limitations

1. **Single armed track**: Can only record one track at a time
2. **In-memory only**: Recordings not saved to disk until project saved
3. **Fixed chunk size**: 8192 samples, not configurable
4. **Fixed count-in**: Always 1 bar, not configurable
5. **No latency compensation**: User must manually adjust if needed
6. **No pre-roll**: Can't capture audio before pressing record

These limitations are intentional for the initial implementation and can be addressed in future versions.
