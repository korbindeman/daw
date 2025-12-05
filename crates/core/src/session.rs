use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use basedrop::Shared;
use rayon::prelude::*;

use crate::time::{TimeContext, TimeSignature};
use daw_decode::strip_samples_root;
use daw_engine::{AudioEngineHandle, EngineClip, EngineCommand, EngineStatus, EngineTrack};
use daw_project::{load_project, save_project};
use daw_render::{render_timeline, write_wav};
use daw_transport::{resample_audio, AudioBuffer, PPQN, Track};

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

/// Cache key for resampled audio: (pointer to original audio, target sample rate)
type ResampleCacheKey = (usize, u32);

pub struct Session {
    engine: AudioEngineHandle,
    tracks: Vec<Track>,
    time_context: TimeContext,
    current_tick: u64,
    playback_state: PlaybackState,
    /// Cache of resampled audio buffers, keyed by (original audio pointer, target sample rate)
    resample_cache: HashMap<ResampleCacheKey, Arc<AudioBuffer>>,
    /// Mapping from clip ID to audio file path (relative to samples root)
    audio_paths: HashMap<u64, PathBuf>,
    /// Path to the project file (if loaded from or saved to a file)
    project_path: Option<PathBuf>,
    /// Project name
    name: String,
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
        audio_paths: HashMap<u64, PathBuf>,
    ) -> anyhow::Result<Self> {
        let time_context = TimeContext::new(tempo, time_signature.into(), 100.0);

        // Convert tracks to engine format (ticks -> samples)
        // Use a placeholder sample rate; we'll get the real one after engine starts
        let engine = daw_engine::start(vec![])?;
        let sample_rate = engine.sample_rate;

        let mut session = Self {
            engine,
            tracks,
            time_context,
            current_tick: 0,
            playback_state: PlaybackState::Stopped,
            resample_cache: HashMap::new(),
            audio_paths,
            project_path: None,
            name: "Untitled".to_string(),
        };

        // Now send the real tracks with correct sample rate conversion
        session.send_tracks_to_engine(sample_rate);

        Ok(session)
    }

    pub fn from_project(path: &Path) -> anyhow::Result<Self> {
        let project = load_project(path)?;
        let mut session =
            Self::new_with_audio_paths(project.tracks, project.tempo, project.time_signature, project.audio_paths)?;
        session.project_path = Some(path.to_path_buf());
        session.name = project.name;
        Ok(session)
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        // Strip samples root from all audio paths before saving
        let stripped_paths: HashMap<u64, PathBuf> = self
            .audio_paths
            .iter()
            .map(|(id, p)| (*id, strip_samples_root(p)))
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
        let engine_tracks = self.convert_tracks_for_engine(sample_rate);
        let shared_tracks = Shared::new(&self.engine.handle, engine_tracks);
        let _ = self.engine.tracks.push(shared_tracks);
    }

    fn convert_tracks_for_engine(&mut self, sample_rate: u32) -> Vec<EngineTrack> {
        // Collect unique audio buffers by pointer address (avoids needing Hash on AudioBuffer)
        let mut unique_audios: HashMap<usize, Arc<AudioBuffer>> = HashMap::new();
        for track in &self.tracks {
            for clip in &track.clips {
                let ptr = Arc::as_ptr(&clip.audio) as usize;
                unique_audios
                    .entry(ptr)
                    .or_insert_with(|| clip.audio.clone());
            }
        }

        // Filter to only those not already in cache
        let to_resample: Vec<Arc<AudioBuffer>> = unique_audios
            .into_values()
            .filter(|audio| {
                let key = (Arc::as_ptr(audio) as usize, sample_rate);
                !self.resample_cache.contains_key(&key)
            })
            .collect();

        // Resample in parallel
        let resampled: Vec<(ResampleCacheKey, Arc<AudioBuffer>)> = to_resample
            .par_iter()
            .filter_map(|audio| {
                let key = (Arc::as_ptr(audio) as usize, sample_rate);
                if audio.sample_rate == sample_rate {
                    // No resampling needed, use original
                    Some((key, audio.clone()))
                } else {
                    resample_audio(audio, sample_rate)
                        .ok()
                        .map(|resampled| (key, Arc::new(resampled)))
                }
            })
            .collect();

        // Insert into cache
        for (key, audio) in resampled {
            self.resample_cache.insert(key, audio);
        }

        // Build engine tracks using cached resampled audio
        self.tracks
            .iter()
            .map(|track| EngineTrack {
                clips: track
                    .clips
                    .iter()
                    .filter_map(|clip| {
                        let key = (Arc::as_ptr(&clip.audio) as usize, sample_rate);
                        let resampled_audio = self.resample_cache.get(&key)?;
                        Some(EngineClip {
                            start: self.ticks_to_samples_with_rate(clip.start, sample_rate),
                            audio: resampled_audio.clone(),
                        })
                    })
                    .collect(),
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

    pub fn tracks_mut(&mut self) -> &mut Vec<Track> {
        &mut self.tracks
    }

    pub fn sample_rate(&self) -> u32 {
        self.engine.sample_rate
    }

    pub fn calculate_timeline_width(&self) -> f64 {
        let mut max_end_tick = 0u64;
        for track in &self.tracks {
            for clip in &track.clips {
                let duration_ticks = clip.duration_ticks(self.tempo());
                let end_tick = clip.start + duration_ticks;
                max_end_tick = max_end_tick.max(end_tick);
            }
        }

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

    pub fn audio_paths(&self) -> &HashMap<u64, PathBuf> {
        &self.audio_paths
    }

    pub fn audio_paths_mut(&mut self) -> &mut HashMap<u64, PathBuf> {
        &mut self.audio_paths
    }
}
