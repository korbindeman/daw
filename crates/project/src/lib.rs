mod load;
mod save;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub use load::{load_project, load_project_metadata, load_project_with_sample_rate, ProjectMetadata};
pub use save::save_project;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub tempo: f64,
    pub time_signature: (u32, u32),
    pub tracks: Vec<TrackData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackData {
    pub id: u64,
    pub name: String,
    pub segments: Vec<SegmentData>,
    pub volume: f32,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentData {
    pub start_tick: u64,
    pub end_tick: u64,
    pub audio_path: PathBuf,
    pub audio_offset: u64,
    pub name: String,
}

// Keep ClipData as an alias for backwards compatibility with old project files
pub type ClipData = SegmentData;

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialize(#[from] serde_json::Error),

    #[error("Deserialization error: {0}")]
    Deserialize(#[from] rmp_serde::decode::Error),

    #[error("Failed to decode audio file '{path}': {source}")]
    AudioDecode {
        path: PathBuf,
        source: anyhow::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_project() -> Project {
        Project {
            name: "Test Project".to_string(),
            tempo: 120.0,
            time_signature: (4, 4),
            tracks: vec![
                TrackData {
                    id: 1,
                    name: "Drums".to_string(),
                    segments: vec![
                        SegmentData {
                            start_tick: 0,
                            end_tick: 960,
                            audio_path: PathBuf::from("audio/kick.wav"),
                            audio_offset: 0,
                            name: "Kick".to_string(),
                        },
                        SegmentData {
                            start_tick: 960,
                            end_tick: 1920,
                            audio_path: PathBuf::from("audio/snare.wav"),
                            audio_offset: 0,
                            name: "Snare".to_string(),
                        },
                    ],
                    volume: 1.0,
                    enabled: true,
                },
                TrackData {
                    id: 2,
                    name: "Hi-Hats".to_string(),
                    segments: vec![SegmentData {
                        start_tick: 480,
                        end_tick: 960,
                        audio_path: PathBuf::from("audio/hihat.wav"),
                        audio_offset: 0,
                        name: "Hi-Hat".to_string(),
                    }],
                    volume: 0.8,
                    enabled: true,
                },
            ],
        }
    }

    #[test]
    fn test_project_serialization_roundtrip() {
        let project = sample_project();

        let json = serde_json::to_string(&project).expect("serialize");
        let decoded: Project = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded.name, project.name);
        assert_eq!(decoded.tempo, project.tempo);
        assert_eq!(decoded.time_signature, project.time_signature);
        assert_eq!(decoded.tracks.len(), project.tracks.len());
    }

    #[test]
    fn test_track_data_serialization() {
        let track = TrackData {
            id: 42,
            name: "Test Track".to_string(),
            segments: vec![SegmentData {
                start_tick: 1920,
                end_tick: 2880,
                audio_path: PathBuf::from("samples/test.wav"),
                audio_offset: 0,
                name: "Test".to_string(),
            }],
            volume: 0.75,
            enabled: true,
        };

        let json = serde_json::to_string(&track).expect("serialize");
        let decoded: TrackData = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded.id, 42);
        assert_eq!(decoded.segments.len(), 1);
        assert_eq!(decoded.segments[0].start_tick, 1920);
        assert_eq!(
            decoded.segments[0].audio_path,
            PathBuf::from("samples/test.wav")
        );
    }

    #[test]
    fn test_segment_data_serialization() {
        let segment = SegmentData {
            start_tick: 4800,
            end_tick: 5760,
            audio_path: PathBuf::from("/absolute/path/to/audio.wav"),
            audio_offset: 0,
            name: "Audio".to_string(),
        };

        let json = serde_json::to_string(&segment).expect("serialize");
        let decoded: SegmentData = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded.start_tick, segment.start_tick);
        assert_eq!(decoded.end_tick, segment.end_tick);
        assert_eq!(decoded.audio_path, segment.audio_path);
    }

    #[test]
    fn test_empty_project() {
        let project = Project {
            name: "Empty".to_string(),
            tempo: 140.0,
            time_signature: (3, 4),
            tracks: vec![],
        };

        let json = serde_json::to_string(&project).expect("serialize");
        let decoded: Project = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded.name, "Empty");
        assert_eq!(decoded.tempo, 140.0);
        assert_eq!(decoded.time_signature, (3, 4));
        assert!(decoded.tracks.is_empty());
    }

    #[test]
    fn test_track_with_no_segments() {
        let track = TrackData {
            id: 5,
            name: "Empty Track".to_string(),
            segments: vec![],
            volume: 1.0,
            enabled: true,
        };

        let json = serde_json::to_string(&track).expect("serialize");
        let decoded: TrackData = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded.id, 5);
        assert!(decoded.segments.is_empty());
    }

    #[test]
    fn test_project_clone() {
        let project = sample_project();
        let cloned = project.clone();

        assert_eq!(cloned.name, project.name);
        assert_eq!(cloned.tempo, project.tempo);
        assert_eq!(cloned.tracks.len(), project.tracks.len());
    }

    #[test]
    fn test_project_debug() {
        let project = sample_project();
        let debug_str = format!("{:?}", project);

        assert!(debug_str.contains("Test Project"));
        assert!(debug_str.contains("120"));
    }
}
