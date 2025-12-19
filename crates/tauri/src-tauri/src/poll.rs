//! Background polling loop for session updates.
//!
//! This module runs an async task that polls the Session at ~60 Hz (every 16ms)
//! to retrieve playback position updates and emit events to the frontend.

use crate::dto::SessionTickEvent;
use crate::state::AppState;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// Start the background poll loop.
///
/// This spawns an async task that runs for the lifetime of the application.
/// It polls the session every 16ms and emits "session-tick" events when
/// the playback position changes.
pub fn start_poll_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(16));
        
        loop {
            interval.tick().await;
            
            // Try to get the app state
            let state = match app.try_state::<AppState>() {
                Some(state) => state,
                None => continue,
            };
            
            // Try to lock the session (non-blocking)
            let mut session_lock = match state.session.try_lock() {
                Ok(lock) => lock,
                Err(_) => continue, // Skip this tick if we can't get the lock
            };
            
            // If there's a session, poll it
            if let Some(session) = session_lock.as_mut() {
                // Poll the session for updates
                let tick_changed = session.poll();
                let current_tick = session.current_tick();
                let playback_state = session.playback_state();
                
                // Emit event if tick changed or if we're playing (for smooth updates)
                if tick_changed.is_some() || session.is_playing() {
                    let event = SessionTickEvent {
                        tick: current_tick,
                        playback_state: playback_state.into(),
                    };
                    
                    // Emit the event to all frontend listeners
                    let _ = app.emit("session-tick", event);
                }
            }
        }
    });
}

