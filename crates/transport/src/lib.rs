use std::sync::Arc;

/// Pulses Per Quarter Note - defines timing resolution
pub const PPQN: u64 = 960;

#[derive(Debug, Clone)]
pub struct AudioBuffer {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Debug)]
pub enum Command {
    Play,
    Pause,
    Seek { tick: u64 },
}

#[derive(Debug)]
pub enum Status {
    Position(u64),
}

#[derive(Debug, Clone)]
pub struct Clip {
    pub id: ClipId,
    pub start: u64, // tick position on timeline
    pub audio: Arc<AudioBuffer>,
}

#[derive(Debug, Clone)]
pub struct ClipId(pub u64);

#[derive(Debug, Clone)]
pub struct Track {
    pub id: TrackId,
    pub clips: Vec<Clip>,
}

#[derive(Debug, Clone)]
pub struct TrackId(pub u64);
