//! Tauri commands for session control.
//!
//! These functions are exposed to the frontend via Tauri's IPC mechanism.
//! Each command locks the AppState, performs an operation on the Session,
//! and returns a SessionSnapshot to keep the frontend in sync.

use crate::dto::{session_to_snapshot, SessionSnapshot};
use crate::state::AppState;
use daw_core::Session;
use std::path::Path;
use tauri::State;

// Use anyhow::Error directly as Tauri supports it via InvokeError
type CommandResult<T> = Result<T, String>;

// ============================================================================
// Project Commands
// ============================================================================

/// Load a project file and create a new session.
///
/// Returns a snapshot of the loaded session.
#[tauri::command]
pub fn session_load_project(
    path: String,
    state: State<AppState>,
) -> CommandResult<SessionSnapshot> {
    let session = Session::from_project(Path::new(&path)).map_err(|e| e.to_string())?;
    let snapshot = session_to_snapshot(&session);

    let mut session_lock = state
        .session
        .lock()
        .map_err(|_| "Failed to acquire session lock".to_string())?;
    *session_lock = Some(session);

    Ok(snapshot)
}

/// Get the current session state without modifying it.
///
/// Returns an error if no session is loaded.
#[tauri::command]
pub fn session_get_state(state: State<AppState>) -> CommandResult<SessionSnapshot> {
    let session_lock = state
        .session
        .lock()
        .map_err(|_| "Failed to acquire session lock".to_string())?;

    let session = session_lock
        .as_ref()
        .ok_or_else(|| "No session loaded".to_string())?;

    Ok(session_to_snapshot(session))
}

/// Save the current session to its current path.
///
/// Returns an error if no session is loaded or if the session has no path.
#[tauri::command]
pub fn session_save(state: State<AppState>) -> CommandResult<()> {
    let session_lock = state
        .session
        .lock()
        .map_err(|_| "Failed to acquire session lock".to_string())?;

    let session = session_lock
        .as_ref()
        .ok_or_else(|| "No session loaded".to_string())?;

    session.save_in_place().map_err(|e| e.to_string())?;
    Ok(())
}

/// Save the current session to a new path.
///
/// Returns an error if no session is loaded.
#[tauri::command]
pub fn session_save_as(path: String, state: State<AppState>) -> CommandResult<()> {
    let mut session_lock = state
        .session
        .lock()
        .map_err(|_| "Failed to acquire session lock".to_string())?;

    let session = session_lock
        .as_mut()
        .ok_or_else(|| "No session loaded".to_string())?;

    session.save(Path::new(&path)).map_err(|e| e.to_string())?;
    session.set_project_path(path.into());
    Ok(())
}

/// Render the current session to a WAV file.
///
/// Returns an error if no session is loaded.
#[tauri::command]
pub fn session_render(path: String, state: State<AppState>) -> CommandResult<()> {
    let session_lock = state
        .session
        .lock()
        .map_err(|_| "Failed to acquire session lock".to_string())?;

    let session = session_lock
        .as_ref()
        .ok_or_else(|| "No session loaded".to_string())?;

    session
        .render_to_file(Path::new(&path))
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ============================================================================
// Transport Commands
// ============================================================================

/// Start playback.
#[tauri::command]
pub fn transport_play(state: State<AppState>) -> CommandResult<SessionSnapshot> {
    let mut session_lock = state
        .session
        .lock()
        .map_err(|_| "Failed to acquire session lock".to_string())?;

    let session = session_lock
        .as_mut()
        .ok_or_else(|| "No session loaded".to_string())?;

    session.play();
    Ok(session_to_snapshot(session))
}

/// Pause playback.
#[tauri::command]
pub fn transport_pause(state: State<AppState>) -> CommandResult<SessionSnapshot> {
    let mut session_lock = state
        .session
        .lock()
        .map_err(|_| "Failed to acquire session lock".to_string())?;

    let session = session_lock
        .as_mut()
        .ok_or_else(|| "No session loaded".to_string())?;

    session.pause();
    Ok(session_to_snapshot(session))
}

/// Stop playback.
#[tauri::command]
pub fn transport_stop(state: State<AppState>) -> CommandResult<SessionSnapshot> {
    let mut session_lock = state
        .session
        .lock()
        .map_err(|_| "Failed to acquire session lock".to_string())?;

    let session = session_lock
        .as_mut()
        .ok_or_else(|| "No session loaded".to_string())?;

    session.stop();
    Ok(session_to_snapshot(session))
}

/// Seek to a specific tick position.
#[tauri::command]
pub fn transport_seek_to_tick(tick: u64, state: State<AppState>) -> CommandResult<SessionSnapshot> {
    let mut session_lock = state
        .session
        .lock()
        .map_err(|_| "Failed to acquire session lock".to_string())?;

    let session = session_lock
        .as_mut()
        .ok_or_else(|| "No session loaded".to_string())?;

    session.seek(tick);
    Ok(session_to_snapshot(session))
}

// ============================================================================
// Track Commands
// ============================================================================

/// Toggle a track's enabled state (mute/unmute).
#[tauri::command]
pub fn track_toggle_enabled(track_id: u64, state: State<AppState>) -> CommandResult<SessionSnapshot> {
    let mut session_lock = state
        .session
        .lock()
        .map_err(|_| "Failed to acquire session lock".to_string())?;

    let session = session_lock
        .as_mut()
        .ok_or_else(|| "No session loaded".to_string())?;

    session.toggle_track_enabled(track_id);
    Ok(session_to_snapshot(session))
}

/// Exclusively solo a track (unsolos all others).
#[tauri::command]
pub fn track_solo_exclusive(track_id: u64, state: State<AppState>) -> CommandResult<SessionSnapshot> {
    let mut session_lock = state
        .session
        .lock()
        .map_err(|_| "Failed to acquire session lock".to_string())?;

    let session = session_lock
        .as_mut()
        .ok_or_else(|| "No session loaded".to_string())?;

    session.solo_track_exclusive(track_id);
    Ok(session_to_snapshot(session))
}

/// Set a track's volume.
#[tauri::command]
pub fn track_set_volume(track_id: u64, volume: f32, state: State<AppState>) -> CommandResult<SessionSnapshot> {
    let mut session_lock = state
        .session
        .lock()
        .map_err(|_| "Failed to acquire session lock".to_string())?;

    let session = session_lock
        .as_mut()
        .ok_or_else(|| "No session loaded".to_string())?;

    session.set_track_volume(track_id, volume);
    Ok(session_to_snapshot(session))
}

/// Set a track's pan (-1.0 to 1.0).
#[tauri::command]
pub fn track_set_pan(track_id: u64, pan: f32, state: State<AppState>) -> CommandResult<SessionSnapshot> {
    let mut session_lock = state
        .session
        .lock()
        .map_err(|_| "Failed to acquire session lock".to_string())?;

    let session = session_lock
        .as_mut()
        .ok_or_else(|| "No session loaded".to_string())?;

    session.set_track_pan(track_id, pan);
    Ok(session_to_snapshot(session))
}

// ============================================================================
// Metronome Commands
// ============================================================================

/// Toggle the metronome on/off.
#[tauri::command]
pub fn metronome_toggle(state: State<AppState>) -> CommandResult<SessionSnapshot> {
    let mut session_lock = state
        .session
        .lock()
        .map_err(|_| "Failed to acquire session lock".to_string())?;

    let session = session_lock
        .as_mut()
        .ok_or_else(|| "No session loaded".to_string())?;

    session.toggle_metronome();
    Ok(session_to_snapshot(session))
}

/// Set the metronome volume (0.0 to 1.0).
#[tauri::command]
pub fn metronome_set_volume(volume: f32, state: State<AppState>) -> CommandResult<SessionSnapshot> {
    let mut session_lock = state
        .session
        .lock()
        .map_err(|_| "Failed to acquire session lock".to_string())?;

    let session = session_lock
        .as_mut()
        .ok_or_else(|| "No session loaded".to_string())?;

    session.set_metronome_volume(volume);
    Ok(session_to_snapshot(session))
}

