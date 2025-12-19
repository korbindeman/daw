# Frontend MVP: Tauri + Svelte with `daw_core::Session`

This document describes how to build the first Tauri + Svelte frontend against the existing `daw_core::Session` API. It assumes the "Level 1" contract discipline: `core` is musical/engine-only, UI code owns pixels/zoom.

## 1. MVP scope

The first frontend should support:

- Load a `.dawproj` file into a single `Session`.
- Display project name, tempo, and time signature.
- Display a vertical stack of tracks with their clips laid out on a timeline.
- Basic transport: play, pause, stop, seek-to-tick.
- Visual playhead that advances while playing.
- Track controls: enabled/mute, solo (exclusive), volume, **pan**.
- Metronome on/off and volume.
- Timeline zoom (pixels-per-beat) and scroll handled entirely in the frontend.

No editing yet (no adding/removing/moving clips or tracks) beyond the above controls.

## 2. Core requirements (Session API shape)

`daw_core::Session` must expose a small, UI-facing public API. For the MVP we rely on:

- Construction & lifecycle
  - `Session::from_project(path: &Path) -> Result<Session>`
  - `fn save(&self, path: &Path) -> Result<()>`
  - `fn save_in_place(&self) -> Result<()>`
  - `fn render_to_file(&mut self, path: &Path) -> Result<()>` (optional for MVP UI)
  - `fn name(&self) -> &str`
- Time & tempo
  - `fn tempo(&self) -> f64`
  - `fn set_tempo(&mut self, tempo: f64)`
  - `fn time_signature(&self) -> TimeSignature`
  - `fn set_time_signature(&mut self, ts: TimeSignature)`
  - `fn max_tick(&self) -> u64` (public wrapper around internal `calculate_max_tick()`)
- Transport & position
  - `fn play(&mut self)`
  - `fn pause(&mut self)`
  - `fn stop(&mut self)`
  - `fn seek(&mut self, tick: u64)`
  - `fn poll(&mut self) -> Option<u64>` (drains engine status, updates `current_tick`)
  - `fn current_tick(&self) -> u64`
  - `fn playback_state(&self) -> PlaybackState` or `fn is_playing(&self) -> bool`
- Tracks and mix controls
  - `fn tracks(&self) -> &[Track]`
  - `fn toggle_track_enabled(&mut self, track_id: u64)`
  - `fn solo_track_exclusive(&mut self, track_id: u64)`
  - `fn set_track_volume(&mut self, track_id: u64, volume: f32)`
  - `fn set_track_pan(&mut self, track_id: u64, pan: f32)`
- Metronome
  - `fn toggle_metronome(&mut self)`
  - `fn metronome_enabled(&self) -> bool`
  - `fn set_metronome_volume(&mut self, volume: f32)`

`TimeContext` remains musical-only (tempo, time signature, ticks↔beats/bars/seconds/samples). UI code must not depend on any pixel-based methods.

## 3. Tauri backend architecture

### 3.1. App state

The Tauri app owns a single `Session` in shared state:

- Define an `AppState` struct in the new `app_tauri` crate, e.g.:
  - `struct AppState { session: Mutex<Option<Session>> }`
- Register it with `tauri::Builder::manage(AppState { session: Mutex::new(None) })`.

All commands lock `AppState.session`, access the `Session`, and return JSON DTOs.

### 3.2. Commands

Define a small set of Tauri commands as the UI surface:

- Project commands
  - `session_load_project(path: String) -> Result<SessionSnapshot>`
  - `session_get_state() -> Result<SessionSnapshot>` (idempotent snapshot for refresh)
  - `session_save() -> Result<()>` and `session_save_as(path: String) -> Result<()>`
- Transport commands
  - `transport_play() -> Result<()>`
  - `transport_pause() -> Result<()>`
  - `transport_stop() -> Result<()>`
  - `transport_seek_to_tick(tick: u64) -> Result<()>`
- Track/mix commands
  - `track_toggle_enabled(track_id: u64) -> Result<SessionSnapshot>`
  - `track_solo_exclusive(track_id: u64) -> Result<SessionSnapshot>`
  - `track_set_volume(track_id: u64, volume: f32) -> Result<SessionSnapshot>`
  - `track_set_pan(track_id: u64, pan: f32) -> Result<SessionSnapshot>`
- Metronome commands
  - `metronome_toggle() -> Result<SessionSnapshot>`
  - `metronome_set_volume(volume: f32) -> Result<SessionSnapshot>`

Each command applies the action to `Session`, then returns an updated snapshot so the UI can stay in sync without issuing a separate `get_state` call.

### 3.3. Background poll loop and events

To drive the playhead:

- Start a background async task when the app launches.
- Every ~16ms:
  - Lock `AppState.session`.
  - Call `session.poll()`; if it returns `Some(tick)` or `session.is_playing()` changed, emit a Tauri event.
- Suggested event:
  - Name: `"session-tick"`.
  - Payload: `{ tick: u64, playbackState: "stopped" | "playing" | "paused" }`.

The Svelte frontend subscribes to this event and updates its transport/playhead store.

## 4. DTOs sent to the frontend

Define serializable DTOs in `app_tauri` that mirror the `Session` state:

- `SessionSnapshot`
  - `name: String`
  - `tempo: f64`
  - `timeSignature: { numerator: u32, denominator: u32 }`
  - `maxTick: u64`
  - `currentTick: u64`
  - `playbackState: "stopped" | "playing" | "paused"`
  - `tracks: TrackSummary[]`
  - `metronome: { enabled: bool, volume: f32 }`
- `TrackSummary`
  - `id: u64`
  - `name: String`
  - `enabled: bool`
  - `solo: bool`
  - `volume: f32`
  - `pan: f32` (range -1.0..1.0)
  - `clips: ClipSummary[]`
- `ClipSummary`
  - `startTick: u64`
  - `endTick: u64`
  - `audioOffset: u64`
  - `name: String`

All DTOs are derived by reading from `Session` and `daw_transport::{Track, Clip}`.

## 5. Svelte frontend architecture

### 5.1. Stores

At minimum:

- `sessionStore` (writable)
  - Holds the latest `SessionSnapshot` or `null`.
- `transportStore` (derived or separate)
  - `{ currentTick, playbackState }`, updated from `session-tick` events.
- `uiStore` (writable)
  - `{ pixelsPerBeat, scrollX, selectedTrackId, selectedClipId }`.

### 5.2. Timeline layout

- Each track row component receives:
  - `track: TrackSummary`
  - `pixelsPerBeat: number`
- To compute clip positions in pixels:
  - `beats = (clip.startTick / PPQN)` (PPQN is a shared constant from core).
  - `x = beats * pixelsPerBeat`.
  - Width is based on `(clip.endTick - clip.startTick)`.

Zoom is implemented by changing `pixelsPerBeat` in `uiStore`; the core is not aware of pixels at all.

### 5.3. Interaction flow

- On app start:
  - Ask user for a project path (or open a recent one).
  - Call `session_load_project`, store the returned `SessionSnapshot`.
  - Subscribe to `"session-tick"` events and update `transportStore`.
- On play/pause/stop/seek or track/metronome changes:
  - Call the corresponding command.
  - Merge the returned `SessionSnapshot` back into `sessionStore`.

## 6. Implementation checklist

1. ~~Finalize and implement the Level 1 `Session` API in `daw_core`.~~ ✅ Done
2. ~~Remove pixel-based concerns from `TimeContext` and `Session` (UI owns zoom).~~ ✅ Done
3. Introduce the `app_tauri` crate with `AppState` and a basic Tauri window.
4. Implement the commands listed above, plus the background poll loop and `session-tick` event.
5. Scaffold a minimal Svelte app (TypeScript) with the described stores.
6. Implement timeline rendering using ticks + `pixelsPerBeat`.
7. Wire up transport buttons, track controls, and metronome to Tauri commands.
8. Iterate on UI without touching `daw_core` as long as the Session API contract holds.

## 7. API changes completed

The following changes were made to `daw_core` to support the UI-agnostic contract:

### `TimeContext` (crates/core/src/time.rs)

- **Removed** `pixels_per_beat` field
- **Removed** `ticks_to_pixels()` and `pixels_to_ticks()` methods
- **Updated** constructor: `TimeContext::new(tempo, time_signature)` (no longer takes pixels_per_beat)
- **Kept** all musical time conversions: `ticks_to_beats`, `beats_to_ticks`, `ticks_to_bars`, `bars_to_ticks`, `ticks_to_seconds`, `seconds_to_ticks`, `ticks_to_samples`, `samples_to_ticks`, `format_position`

### `Session` (crates/core/src/session.rs)

- **Added** `max_tick() -> u64` - public getter for the maximum tick position across all clips
- **Removed** `calculate_timeline_width()` - UI should compute this from `max_tick()` and its own `pixelsPerBeat`
- **Made private** internal engine sync methods (`sync_tracks_to_engine`, `sync_tempo_to_engine`)

### UI responsibilities (moved to app crate)

The gpui app now owns:

```rust
// Constants
const DEFAULT_PIXELS_PER_BEAT: f64 = 100.0;
const MIN_TIMELINE_WIDTH: f64 = 1200.0;

// Helper functions
fn ticks_to_pixels(ticks: u64, pixels_per_beat: f64) -> f64;
fn pixels_to_ticks(pixels: f64, pixels_per_beat: f64) -> u64;
fn calculate_timeline_width(max_tick: u64, pixels_per_beat: f64) -> f64;
```

The Tauri/Svelte frontend should implement equivalent helpers in TypeScript.

