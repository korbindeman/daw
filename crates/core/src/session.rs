//! # Session - The Core Abstraction Layer
//!
//! The `Session` is the central interface between frontend UI code and the backend audio engine.
//! It provides a safe, high-level API for managing a DAW project while handling all the complex
//! real-time audio coordination behind the scenes.
//!
//! ## Design Philosophy
//!
//! ### Separation of Concerns
//!
//! The Session architecture cleanly separates **musical time** (tempo-aware) from **physical time**
//! (sample-accurate):
//!
//! - **Frontend (UI)**: Works in **ticks** (musical time) - understands tempo, beats, bars
//! - **Backend (Engine)**: Works in **samples** (physical time) - tempo-agnostic, just audio
//! - **Session**: Translates between the two worlds
//!
//! This separation allows tempo changes without touching any audio code, and sample-accurate
//! playback without understanding musical notation.
//!
//! ### Lock-Free Real-Time Communication
//!
//! The Session uses lock-free data structures to communicate with the audio thread:
//!
//! - **`rtrb` queues**: Send commands (play/pause/seek) and receive status updates
//! - **`basedrop`**: Share track data without blocking the audio thread
//! - **No locks**: The audio thread never blocks, ensuring glitch-free playback
//!
//! When you call `session.play()`, it pushes a command to a lock-free queue. The audio thread
//! reads it at its leisure. When you call `session.set_tracks()`, the new tracks are wrapped
//! in a `basedrop::Shared` and sent through a queue. The old tracks are queued for deallocation
//! later (during `poll()`), not immediately.
//!
//! ### Automatic Synchronization
//!
//! Session methods automatically keep the engine in sync:
//!
//! - `set_tempo()` → recalculates sample positions and updates engine
//! - `set_tracks()` → converts ticks to samples and sends to engine
//! - `add_segment()` → modifies track and updates engine
//! - `set_track_volume()` → updates track and resends to engine
//!
//! The frontend never directly touches the engine - all interactions go through Session.
//!
//! ## Usage Guide
//!
//! ### Creating a Session
//!
//! ```rust,no_run
//! use daw_core::{Session, TimeSignature, Track};
//!
//! // Create a new session
//! let tracks = vec![];
//! let tempo = 120.0;
//! let time_sig = TimeSignature::new(4, 4);
//! let mut session = Session::new(tracks, tempo, time_sig)?;
//!
//! // Or load from a project file
//! use std::path::Path;
//! let mut session = Session::from_project(Path::new("my_song.dawproj"))?;
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! ### Playback Control
//!
//! ```rust,no_run
//! # use daw_core::Session;
//! # let mut session = Session::new(vec![], 120.0, (4, 4))?;
//! // Start playback
//! session.play();
//!
//! // Pause (maintains position)
//! session.pause();
//!
//! // Stop (resets to beginning)
//! session.stop();
//!
//! // Seek to a specific tick
//! session.seek(1920); // Seek to tick 1920
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! ### Polling for Updates
//!
//! The Session must be polled regularly (recommended: 60 Hz) to:
//! 1. Get playback position updates
//! 2. Collect garbage from dropped audio data
//!
//! ```rust,no_run
//! # use daw_core::Session;
//! # let mut session = Session::new(vec![], 120.0, (4, 4))?;
//! // In your main loop (every ~16ms for 60 Hz):
//! if let Some(tick) = session.poll() {
//!     // Position changed, update UI
//!     println!("Now at tick: {}", tick);
//! }
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! ### Modifying the Session
//!
//! ```rust,no_run
//! # use daw_core::{Session, TimeSignature};
//! # let mut session = Session::new(vec![], 120.0, (4, 4))?;
//! // Change tempo
//! session.set_tempo(140.0);
//!
//! // Change time signature
//! session.set_time_signature(TimeSignature::new(3, 4));
//!
//! // Adjust track volume
//! session.set_track_volume(0, 0.75); // Track 0, 75% volume
//!
//! // Toggle track mute
//! session.toggle_track_enabled(0);
//!
//! // Control metronome
//! session.toggle_metronome();
//! session.set_metronome_volume(0.5);
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! ### Project Management
//!
//! ```rust,no_run
//! # use daw_core::Session;
//! # use std::path::Path;
//! # let mut session = Session::new(vec![], 120.0, (4, 4))?;
//! // Save project
//! session.save(Path::new("my_song.dawproj"))?;
//!
//! // Save in place (if loaded from file)
//! session.save_in_place()?;
//!
//! // Render to WAV
//! session.render_to_file(Path::new("output.wav"))?;
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! ## Implementation Details
//!
//! ### Time Conversion
//!
//! Internally, the Session maintains a `TimeContext` that handles tick ↔ sample conversions
//! based on tempo and sample rate. When tempo changes:
//!
//! 1. All clip positions (stored in ticks) remain unchanged
//! 2. Session recalculates sample positions for current tempo
//! 3. Updated tracks are sent to engine
//!
//! This is why clips don't drift when you change tempo - they're stored in musical time.
//!
//! ### Memory Management
//!
//! Audio buffers are reference-counted (`Arc`) and can be cheaply cloned. When you update
//! tracks, the Session doesn't copy audio data - it clones `Arc` pointers. The `basedrop`
//! collector ensures old data is freed outside the audio thread during `poll()`.
//!
//! ### Thread Safety
//!
//! - Session is `!Send` - keep it on the main/UI thread
//! - The audio engine runs on a separate thread
//! - All communication is lock-free via queues
//! - `poll()` is the only method that reads from the engine
//!
//! ## See Also
//!
//! - [`TimeContext`] - Handles tick/sample conversion
//! - [`Track`] - Track and segment data structures
//! - [Session & Engine Interaction](../../docs/session-engine.md) - Detailed architecture guide
//!
//! [`TimeContext`]: crate::time::TimeContext
//! [`Track`]: daw_transport::Track

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use basedrop::Shared;

use crate::time::{TimeContext, TimeSignature};
use daw_decode::{AudioCache, decode_audio_arc_direct, strip_samples_root};
use daw_engine::{AudioEngineHandle, EngineClip, EngineCommand, EngineStatus, EngineTrack};
use daw_project::save_project;
use daw_render::{render_timeline, write_wav};
use daw_transport::{AudioArc, PPQN, Segment, Track, TrackId};

/// Metronome samples and state
pub struct Metronome {
    /// Sample for beat 1 (downbeat)
    pub hi: AudioArc,
    /// Sample for other beats
    pub lo: AudioArc,
    /// Whether metronome is enabled
    pub enabled: bool,
    /// Volume (0.0 to 1.0)
    pub volume: f32,
}

impl Metronome {
    /// Load metronome samples from the assets directory
    pub fn load() -> anyhow::Result<Self> {
        let hi = decode_audio_arc_direct(Path::new("assets/metronome_hi.wav"), None)?;
        let lo = decode_audio_arc_direct(Path::new("assets/metronome_lo.wav"), None)?;

        Ok(Self {
            hi,
            lo,
            enabled: false,
            volume: 0.8,
        })
    }
}

/// Current playback state of the session
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    /// Playback is stopped, position is at 0
    Stopped,
    /// Audio is currently playing
    Playing,
    /// Playback is paused, position is maintained
    Paused,
}

impl PlaybackState {
    pub fn is_playing(&self) -> bool {
        matches!(self, PlaybackState::Playing)
    }
}

/// The main DAW session - manages a project and coordinates with the audio engine.
///
/// Session is the primary interface for frontend code. It handles:
/// - Real-time audio playback via lock-free communication with the engine
/// - Musical time (ticks) ↔ physical time (samples) conversion
/// - Project state (tracks, tempo, time signature)
/// - Project persistence (save/load)
///
/// All modifications to tracks, tempo, or playback automatically synchronize with the
/// audio engine without blocking the audio thread.
///
/// # Example
///
/// ```no_run
/// use daw_core::Session;
///
/// let mut session = Session::new(vec![], 120.0, (4, 4))?;
/// session.play();
///
/// // Poll at 60 Hz
/// loop {
///     if let Some(tick) = session.poll() {
///         println!("Position: {}", tick);
///     }
///     std::thread::sleep(std::time::Duration::from_millis(16));
/// }
/// # Ok::<(), anyhow::Error>(())
/// ```
pub struct Session {
    engine: AudioEngineHandle,
    tracks: Vec<Track>,
    time_context: TimeContext,
    current_tick: u64,
    playback_state: PlaybackState,
    /// Cache for decoded and resampled audio
    cache: AudioCache,
    /// Mapping from segment name to audio file path (relative to samples root)
    audio_paths: HashMap<String, PathBuf>,
    /// Path to the project file (if loaded from or saved to a file)
    project_path: Option<PathBuf>,
    /// Project name
    name: String,
    /// Metronome state and samples
    metronome: Metronome,
}

impl Session {
    /// Create a new session with the given tracks, tempo, and time signature.
    ///
    /// This starts the audio engine and sends the initial tracks to it.
    ///
    /// # Arguments
    ///
    /// * `tracks` - Initial tracks (can be empty)
    /// * `tempo` - Tempo in BPM (e.g., 120.0)
    /// * `time_signature` - Time signature (e.g., `(4, 4)` or `TimeSignature::new(4, 4)`)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use daw_core::Session;
    ///
    /// let session = Session::new(vec![], 120.0, (4, 4))?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn new(
        tracks: Vec<Track>,
        tempo: f64,
        time_signature: impl Into<TimeSignature>,
    ) -> anyhow::Result<Self> {
        Self::new_with_audio_paths(tracks, tempo, time_signature, HashMap::new())
    }

    /// Create a new session with audio path mappings.
    ///
    /// This is typically used internally when loading projects that contain
    /// references to audio files.
    pub fn new_with_audio_paths(
        tracks: Vec<Track>,
        tempo: f64,
        time_signature: impl Into<TimeSignature>,
        audio_paths: HashMap<String, PathBuf>,
    ) -> anyhow::Result<Self> {
        let time_context = TimeContext::new(tempo, time_signature.into(), 100.0);

        // Load metronome samples
        let metronome = Metronome::load()?;

        // Start the audio engine
        let engine = daw_engine::start(vec![])?;
        let sample_rate = engine.sample_rate;

        let mut session = Self {
            engine,
            tracks,
            time_context,
            current_tick: 0,
            playback_state: PlaybackState::Stopped,
            cache: AudioCache::new(),
            audio_paths,
            project_path: None,
            name: "Untitled".to_string(),
            metronome,
        };

        // Now send the real tracks with correct sample rate conversion
        session.send_tracks_to_engine(sample_rate);

        Ok(session)
    }

    /// Load a session from a project file.
    ///
    /// This loads all project settings (tempo, time signature, tracks) and starts
    /// the audio engine.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use daw_core::Session;
    /// use std::path::Path;
    ///
    /// let session = Session::from_project(Path::new("my_song.dawproj"))?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn from_project(path: &Path) -> anyhow::Result<Self> {
        // Start engine first to get sample rate
        let engine = daw_engine::start(vec![])?;
        let sample_rate = engine.sample_rate;

        // Load project with audio resampled to engine sample rate
        let project = daw_project::load_project_with_sample_rate(path, Some(sample_rate))?;

        // Create time context and load metronome
        let time_context = TimeContext::new(project.tempo, project.time_signature, 100.0);
        let metronome = Metronome::load()?;

        // Create session with loaded cache
        let mut session = Self {
            engine,
            tracks: project.tracks,
            time_context,
            current_tick: 0,
            playback_state: PlaybackState::Stopped,
            cache: project.cache,
            audio_paths: project.audio_paths,
            project_path: Some(path.to_path_buf()),
            name: project.name,
            metronome,
        };

        // Send tracks to engine (already at correct sample rate)
        session.send_tracks_to_engine(sample_rate);

        Ok(session)
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        // Strip samples root from all audio paths before saving
        let stripped_paths: HashMap<String, PathBuf> = self
            .audio_paths
            .iter()
            .map(|(name, p)| (name.clone(), strip_samples_root(p)))
            .collect();

        save_project(
            path,
            self.name.clone(),
            self.tempo(),
            (
                self.time_signature().numerator,
                self.time_signature().denominator,
            ),
            &self.tracks,
            &stripped_paths,
        )?;
        Ok(())
    }

    pub fn save_in_place(&self) -> anyhow::Result<()> {
        let path = self
            .project_path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No project path set"))?;
        self.save(path)
    }

    /// Start playback from the current position.
    ///
    /// Sends a play command to the audio engine via a lock-free queue.
    /// The audio will start playing asynchronously.
    pub fn play(&mut self) {
        let _ = self.engine.commands.push(EngineCommand::Play);
        self.playback_state = PlaybackState::Playing;
    }

    /// Pause playback, maintaining the current position.
    ///
    /// The playhead position is preserved. Call `play()` to resume.
    pub fn pause(&mut self) {
        let _ = self.engine.commands.push(EngineCommand::Pause);
        self.playback_state = PlaybackState::Paused;
    }

    /// Stop playback and reset position to the beginning.
    ///
    /// This pauses the audio engine and seeks to tick 0.
    pub fn stop(&mut self) {
        let _ = self.engine.commands.push(EngineCommand::Pause);
        let _ = self.engine.commands.push(EngineCommand::Seek { sample: 0 });
        self.current_tick = 0;
        self.playback_state = PlaybackState::Stopped;
    }

    /// Seek to a specific tick position.
    ///
    /// The tick is converted to samples based on the current tempo and sent
    /// to the audio engine.
    ///
    /// # Arguments
    ///
    /// * `tick` - The tick position to seek to (480 ticks = 1 quarter note at PPQN=480)
    pub fn seek(&mut self, tick: u64) {
        let sample = self.ticks_to_samples(tick);
        let _ = self.engine.commands.push(EngineCommand::Seek { sample });
        self.current_tick = tick;
    }

    /// Poll the session for position updates and perform garbage collection.
    ///
    /// **This must be called regularly (recommended: 60 Hz / every ~16ms)** to:
    /// 1. Retrieve playback position updates from the audio engine
    /// 2. Free memory from old track data via the basedrop collector
    ///
    /// Returns `Some(tick)` if the playback position changed since the last poll,
    /// `None` otherwise.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use daw_core::Session;
    /// # let mut session = Session::new(vec![], 120.0, (4, 4))?;
    /// // In your main loop at 60 Hz:
    /// if let Some(tick) = session.poll() {
    ///     println!("Playback position: {}", tick);
    /// }
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn poll(&mut self) -> Option<u64> {
        // Free any old track data that the audio thread has dropped
        self.engine.collector.collect();

        let mut position_changed = None;
        while let Ok(status) = self.engine.status.pop() {
            match status {
                EngineStatus::Position(sample) => {
                    let tick = self.samples_to_ticks(sample);
                    self.current_tick = tick;
                    position_changed = Some(tick);
                }
            }
        }
        position_changed
    }

    /// Send updated tracks to the audio engine (lock-free)
    /// Converts tick positions to sample positions
    pub fn update_tracks(&mut self) {
        self.send_tracks_to_engine(self.engine.sample_rate);
    }

    /// When tempo changes, re-send tracks with new sample positions
    pub fn update_tempo(&mut self) {
        self.send_tracks_to_engine(self.engine.sample_rate);
    }

    fn send_tracks_to_engine(&mut self, sample_rate: u32) {
        let mut engine_tracks = self.convert_tracks_for_engine(sample_rate);

        // Add metronome track if enabled
        if self.metronome.enabled {
            if let Some(metronome_track) = self.generate_metronome_track(sample_rate) {
                engine_tracks.push(metronome_track);
            }
        }

        let shared_tracks = Shared::new(&self.engine.handle, engine_tracks);
        let _ = self.engine.tracks.push(shared_tracks);
    }

    /// Generate a metronome track with clicks on each beat
    fn generate_metronome_track(&mut self, sample_rate: u32) -> Option<EngineTrack> {
        // Resample metronome samples if needed (cheap clone if already at target rate)
        let hi_audio = if self.metronome.hi.sample_rate() == sample_rate {
            self.metronome.hi.clone()
        } else {
            self.metronome.hi.resample(sample_rate).ok()?
        };

        let lo_audio = if self.metronome.lo.sample_rate() == sample_rate {
            self.metronome.lo.clone()
        } else {
            self.metronome.lo.resample(sample_rate).ok()?
        };

        // Calculate timeline length based on content
        let max_tick = self.calculate_max_tick();
        // Add some padding (4 bars worth)
        let ticks_per_bar = self.time_context.time_signature.ticks_per_bar();
        let end_tick = max_tick + ticks_per_bar * 4;

        // Generate clicks for each beat
        let beats_per_bar = self.time_context.time_signature.beats_per_bar();
        let mut clips = Vec::new();
        let mut current_tick = 0u64;
        let mut beat_in_bar = 0u32;

        while current_tick < end_tick {
            let audio = if beat_in_bar == 0 {
                hi_audio.clone()
            } else {
                lo_audio.clone()
            };

            clips.push(EngineClip {
                start: self.ticks_to_samples_with_rate(current_tick, sample_rate),
                audio,
                offset: 0,
                length: None,
            });

            current_tick += PPQN;
            beat_in_bar = (beat_in_bar + 1) % beats_per_bar;
        }

        Some(EngineTrack {
            clips,
            volume: self.metronome.volume,
        })
    }

    /// Calculate the maximum tick position across all segments
    fn calculate_max_tick(&self) -> u64 {
        let mut max_tick = 0u64;
        for track in &self.tracks {
            for segment in track.segments() {
                max_tick = max_tick.max(segment.end_tick);
            }
        }
        max_tick
    }

    fn convert_tracks_for_engine(&mut self, sample_rate: u32) -> Vec<EngineTrack> {
        // Build engine tracks from segments, resampling audio if needed
        // Note: Segments already have AudioArc, which makes cloning cheap
        self.tracks
            .iter()
            .filter(|track| track.enabled)
            .map(|track| EngineTrack {
                clips: track
                    .segments()
                    .iter()
                    .filter_map(|segment| {
                        // Resample audio if not at engine sample rate
                        // If already at target rate, this is just a cheap Arc clone
                        let audio = if segment.audio.sample_rate() == sample_rate {
                            segment.audio.clone()
                        } else {
                            segment.audio.resample(sample_rate).ok()?
                        };

                        // Convert duration from ticks to samples
                        let length_samples =
                            self.ticks_to_samples_with_rate(segment.duration_ticks(), sample_rate);

                        Some(EngineClip {
                            start: self.ticks_to_samples_with_rate(segment.start_tick, sample_rate),
                            audio,
                            offset: segment.audio_offset,
                            length: Some(length_samples),
                        })
                    })
                    .collect(),
                volume: track.volume,
            })
            .collect()
    }

    fn ticks_to_samples(&self, ticks: u64) -> u64 {
        self.time_context
            .ticks_to_samples(ticks, self.engine.sample_rate)
    }

    fn ticks_to_samples_with_rate(&self, ticks: u64, sample_rate: u32) -> u64 {
        self.time_context.ticks_to_samples(ticks, sample_rate)
    }

    fn samples_to_ticks(&self, samples: u64) -> u64 {
        self.time_context
            .samples_to_ticks(samples, self.engine.sample_rate)
    }

    pub fn current_tick(&self) -> u64 {
        self.current_tick
    }

    pub fn playback_state(&self) -> PlaybackState {
        self.playback_state
    }

    pub fn is_playing(&self) -> bool {
        self.playback_state.is_playing()
    }

    pub fn time_context(&self) -> &TimeContext {
        &self.time_context
    }

    pub fn tempo(&self) -> f64 {
        self.time_context.tempo
    }

    pub fn time_signature(&self) -> TimeSignature {
        self.time_context.time_signature
    }

    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    /// Set the tempo and update the engine with new sample positions
    pub fn set_tempo(&mut self, tempo: f64) {
        self.time_context.tempo = tempo;
        self.update_tempo();
    }

    /// Set the time signature and update the engine
    pub fn set_time_signature(&mut self, time_signature: TimeSignature) {
        self.time_context.time_signature = time_signature;
        self.update_tempo();
    }

    // Track management methods

    /// Replace all tracks. Track's insert_segment handles overlap resolution internally.
    pub fn set_tracks(&mut self, tracks: Vec<Track>) {
        self.tracks = tracks;
        self.send_tracks_to_engine(self.engine.sample_rate);
    }

    /// Add a segment to a track. Overlaps are resolved automatically by Track.
    pub fn add_segment(&mut self, track_id: TrackId, segment: Segment) {
        if let Some(track) = self.tracks.iter_mut().find(|t| t.id.0 == track_id.0) {
            track.insert_segment(segment);
            self.send_tracks_to_engine(self.engine.sample_rate);
        }
    }

    /// Set the volume for a specific track
    pub fn set_track_volume(&mut self, track_id: u64, volume: f32) {
        if let Some(track) = self.tracks.iter_mut().find(|t| t.id.0 == track_id) {
            track.volume = volume.clamp(0.0, 1.0);
            self.send_tracks_to_engine(self.engine.sample_rate);
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.engine.sample_rate
    }

    pub fn calculate_timeline_width(&self) -> f64 {
        let max_end_tick = self.calculate_max_tick();
        let end_with_padding = max_end_tick + (PPQN * 4);
        let content_width = self.time_context.ticks_to_pixels(end_with_padding);
        let min_width = 1200.0;
        content_width.max(min_width)
    }

    pub fn render_to_file(&self, path: &Path) -> anyhow::Result<()> {
        let sample_rate = 44100;
        let channels = 2;
        let buffer = render_timeline(&self.tracks, self.tempo(), sample_rate, channels);
        write_wav(&buffer, path)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    pub fn project_path(&self) -> Option<&Path> {
        self.project_path.as_deref()
    }

    pub fn set_project_path(&mut self, path: PathBuf) {
        self.project_path = Some(path);
    }

    pub fn audio_paths(&self) -> &HashMap<String, PathBuf> {
        &self.audio_paths
    }

    // Metronome controls

    pub fn metronome_enabled(&self) -> bool {
        self.metronome.enabled
    }

    pub fn set_metronome_enabled(&mut self, enabled: bool) {
        self.metronome.enabled = enabled;
        self.send_tracks_to_engine(self.engine.sample_rate);
    }

    pub fn toggle_metronome(&mut self) {
        self.set_metronome_enabled(!self.metronome.enabled);
    }

    pub fn metronome_volume(&self) -> f32 {
        self.metronome.volume
    }

    pub fn set_metronome_volume(&mut self, volume: f32) {
        self.metronome.volume = volume.clamp(0.0, 1.0);
        if self.metronome.enabled {
            self.send_tracks_to_engine(self.engine.sample_rate);
        }
    }

    // Track enabled/disabled controls

    pub fn set_track_enabled(&mut self, track_id: u64, enabled: bool) {
        if let Some(track) = self.tracks.iter_mut().find(|t| t.id.0 == track_id) {
            track.enabled = enabled;
            self.send_tracks_to_engine(self.engine.sample_rate);
        }
    }

    pub fn toggle_track_enabled(&mut self, track_id: u64) {
        if let Some(track) = self.tracks.iter_mut().find(|t| t.id.0 == track_id) {
            track.enabled = !track.enabled;
            self.send_tracks_to_engine(self.engine.sample_rate);
        }
    }
}
