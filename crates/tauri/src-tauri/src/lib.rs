//! Tauri backend for the DAW application.
//!
//! This crate provides the Tauri commands and state management for the DAW frontend.
//! It wraps the `daw_core::Session` API and exposes it to the Svelte frontend via IPC.

mod commands;
mod dto;
mod poll;
mod state;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            // Project commands
            commands::session_load_project,
            commands::session_get_state,
            commands::session_save,
            commands::session_save_as,
            // Transport commands
            commands::transport_play,
            commands::transport_pause,
            commands::transport_stop,
            commands::transport_seek_to_tick,
            // Track commands
            commands::track_toggle_enabled,
            commands::track_solo_exclusive,
            commands::track_set_volume,
            commands::track_set_pan,
            // Metronome commands
            commands::metronome_toggle,
            commands::metronome_set_volume,
        ])
        .setup(|app| {
            // Start the background poll loop
            poll::start_poll_loop(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
