//! Tauri backend for the DAW application.
//!
//! This crate provides the Tauri commands and state management for the DAW frontend.
//! It wraps the `daw_core::Session` API and exposes it to the Svelte frontend via IPC.

mod commands;
mod dto;
mod poll;
mod state;

use state::AppState;
use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::Emitter;

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
            commands::session_render,
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
            // Build the main app menu
            let app_menu = SubmenuBuilder::new(app, &app.package_info().name)
                .quit()
                .build()?;

            // Build the File menu with keyboard shortcuts
            let open_item = MenuItemBuilder::with_id("open_project", "Open Project...")
                .accelerator("CmdOrCtrl+O")
                .build(app)?;
            let save_item = MenuItemBuilder::with_id("save", "Save")
                .accelerator("CmdOrCtrl+S")
                .build(app)?;
            let save_as_item = MenuItemBuilder::with_id("save_as", "Save As...")
                .accelerator("CmdOrCtrl+Shift+S")
                .build(app)?;
            let render_item = MenuItemBuilder::with_id("render", "Render...")
                .accelerator("CmdOrCtrl+R")
                .build(app)?;

            let file_menu = SubmenuBuilder::new(app, "File")
                .item(&open_item)
                .separator()
                .item(&save_item)
                .item(&save_as_item)
                .separator()
                .item(&render_item)
                .build()?;

            // Build the full menu bar
            let menu = MenuBuilder::new(app)
                .item(&app_menu)
                .item(&file_menu)
                .build()?;

            app.set_menu(menu)?;

            // Start the background poll loop
            poll::start_poll_loop(app.handle().clone());
            Ok(())
        })
        .on_menu_event(|app, event| {
            let event_id = event.id().0.as_str();
            // Emit menu events to the frontend so it can handle dialogs
            let _ = app.emit("menu-event", event_id);
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
