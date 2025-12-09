use std::collections::HashMap;
use std::path::{Path, PathBuf};

use basedrop::Shared;

use crate::time::{TimeContext, TimeSignature};
use daw_decode::{AudioCache, decode_audio_arc_direct, strip_samples_root};
use daw_engine::{AudioEngineHandle, EngineClip, EngineCommand, EngineStatus, EngineTrack};
use daw_project::{load_project, save_project};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

impl PlaybackState {
    pub fn is_playing(&self) -> bool {
        matches!(self, PlaybackState::Playing)
    }
}

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
    pub fn new(
        tracks: Vec<Track>,
        tempo: f64,
        time_signature: impl Into<TimeSignature>,
    ) -> anyhow::Result<Self> {
        Self::new_with_audio_paths(tracks, tempo, time_signature, HashMap::new())
    }

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

    pub fn from_project(path: &Path) -> anyhow::Result<Self> {
        let project = load_project(path)?;
        let mut session = Self::new_with_audio_paths(
            project.tracks,
            project.tempo,
            project.time_signature,
            project.audio_paths,
        )?;
        session.project_path = Some(path.to_path_buf());
        session.name = project.name;
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

    pub fn play(&mut self) {
        let _ = self.engine.commands.push(EngineCommand::Play);
        self.playback_state = PlaybackState::Playing;
    }

    pub fn pause(&mut self) {
        let _ = self.engine.commands.push(EngineCommand::Pause);
        self.playback_state = PlaybackState::Paused;
    }

    pub fn stop(&mut self) {
        let _ = self.engine.commands.push(EngineCommand::Pause);
        let _ = self.engine.commands.push(EngineCommand::Seek { sample: 0 });
        self.current_tick = 0;
        self.playback_state = PlaybackState::Stopped;
    }

    pub fn seek(&mut self, tick: u64) {
        let sample = self.ticks_to_samples(tick);
        let _ = self.engine.commands.push(EngineCommand::Seek { sample });
        self.current_tick = tick;
    }

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

    pub fn time_context_mut(&mut self) -> &mut TimeContext {
        &mut self.time_context
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

    pub fn audio_paths_mut(&mut self) -> &mut HashMap<String, PathBuf> {
        &mut self.audio_paths
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
