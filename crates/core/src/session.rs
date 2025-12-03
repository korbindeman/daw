use std::path::Path;

use crate::time::{TimeContext, TimeSignature};
use daw_engine::AudioEngineHandle;
use daw_project::load_project;
use daw_render::{render_timeline, write_wav};
use daw_transport::{Command, Status, Track, PPQN};

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
}

impl Session {
    pub fn new(tracks: Vec<Track>, tempo: f64, time_signature: impl Into<TimeSignature>) -> anyhow::Result<Self> {
        let time_signature = time_signature.into();
        let engine = daw_engine::start(tracks.clone(), tempo)?;

        Ok(Self {
            engine,
            tracks,
            time_context: TimeContext::new(tempo, time_signature, 100.0),
            current_tick: 0,
            playback_state: PlaybackState::Stopped,
        })
    }

    pub fn from_project(path: &Path) -> anyhow::Result<Self> {
        let project = load_project(path)?;
        Self::new(project.tracks, project.tempo, project.time_signature)
    }

    pub fn play(&mut self) {
        let _ = self.engine.commands.push(Command::Play);
        self.playback_state = PlaybackState::Playing;
    }

    pub fn pause(&mut self) {
        let _ = self.engine.commands.push(Command::Pause);
        self.playback_state = PlaybackState::Paused;
    }

    pub fn stop(&mut self) {
        let _ = self.engine.commands.push(Command::Pause);
        let _ = self.engine.commands.push(Command::Seek { tick: 0 });
        self.current_tick = 0;
        self.playback_state = PlaybackState::Stopped;
    }

    pub fn seek(&mut self, tick: u64) {
        let _ = self.engine.commands.push(Command::Seek { tick });
        self.current_tick = tick;
    }

    pub fn poll(&mut self) -> Option<u64> {
        let mut position_changed = None;
        while let Ok(status) = self.engine.status.pop() {
            match status {
                Status::Position(tick) => {
                    self.current_tick = tick;
                    position_changed = Some(tick);
                }
            }
        }
        position_changed
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
}
