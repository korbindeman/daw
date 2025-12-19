//! Application state management.
//!
//! The AppState holds the DAW session and is shared across all Tauri commands.

use daw_core::Session;
use std::sync::Mutex;

/// Shared application state.
///
/// This is managed by Tauri and accessible from all commands.
/// The Session is wrapped in Option because it's only created when a project is loaded.
pub struct AppState {
    /// The current DAW session, if one is loaded.
    pub session: Mutex<Option<Session>>,
}

impl AppState {
    /// Create a new AppState with no session loaded.
    pub fn new() -> Self {
        Self {
            session: Mutex::new(None),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

