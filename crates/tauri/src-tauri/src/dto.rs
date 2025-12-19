//! Data Transfer Objects (DTOs) for communication between Tauri backend and frontend.
//!
//! These types are serialized to JSON and sent to the Svelte frontend.
//! They represent snapshots of the Session state at a point in time.

use serde::{Deserialize, Serialize};

/// Complete snapshot of the session state.
///
/// This is returned by most commands to keep the frontend in sync
/// without requiring a separate `get_state` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSnapshot {
    pub name: String,
    pub tempo: f64,
    pub time_signature: TimeSignatureDto,
    pub max_tick: u64,
    pub current_tick: u64,
    pub playback_state: PlaybackStateDto,
    pub tracks: Vec<TrackSummary>,
    pub metronome: MetronomeState,
}

/// Time signature representation for the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeSignatureDto {
    pub numerator: u32,
    pub denominator: u32,
}

/// Playback state as a string enum for easy frontend consumption.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlaybackStateDto {
    Stopped,
    Playing,
    Paused,
}

/// Summary of a track with its clips.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackSummary {
    pub id: u64,
    pub name: String,
    pub enabled: bool,
    pub solo: bool,
    pub volume: f32,
    pub pan: f32,
    pub clips: Vec<ClipSummary>,
}

/// Summary of a clip with its timeline position.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipSummary {
    pub start_tick: u64,
    pub end_tick: u64,
    pub audio_offset: u64,
    pub name: String,
}

/// Metronome state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetronomeState {
    pub enabled: bool,
    pub volume: f32,
}

/// Event payload for session tick updates.
///
/// Emitted by the background poll loop to update the frontend's playhead.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionTickEvent {
    pub tick: u64,
    pub playback_state: PlaybackStateDto,
}

impl From<daw_core::PlaybackState> for PlaybackStateDto {
    fn from(state: daw_core::PlaybackState) -> Self {
        match state {
            daw_core::PlaybackState::Stopped => PlaybackStateDto::Stopped,
            daw_core::PlaybackState::Playing => PlaybackStateDto::Playing,
            daw_core::PlaybackState::Paused => PlaybackStateDto::Paused,
        }
    }
}

impl From<daw_core::TimeSignature> for TimeSignatureDto {
    fn from(ts: daw_core::TimeSignature) -> Self {
        Self {
            numerator: ts.numerator,
            denominator: ts.denominator,
        }
    }
}

/// Convert a Session into a SessionSnapshot.
///
/// This is the main conversion function used by all commands.
pub fn session_to_snapshot(session: &daw_core::Session) -> SessionSnapshot {
    SessionSnapshot {
        name: session.name().to_string(),
        tempo: session.tempo(),
        time_signature: session.time_signature().into(),
        max_tick: session.max_tick(),
        current_tick: session.current_tick(),
        playback_state: session.playback_state().into(),
        tracks: session
            .tracks()
            .iter()
            .map(|track| TrackSummary {
                id: track.id.0,
                name: track.name.clone(),
                enabled: track.enabled,
                solo: track.solo,
                volume: track.volume,
                pan: track.pan,
                clips: track
                    .clips()
                    .iter()
                    .map(|clip| ClipSummary {
                        start_tick: clip.start_tick,
                        end_tick: clip.end_tick,
                        audio_offset: clip.audio_offset,
                        name: clip.name.clone(),
                    })
                    .collect(),
            })
            .collect(),
        metronome: MetronomeState {
            enabled: session.metronome_enabled(),
            volume: session.metronome_volume(),
        },
    }
}

