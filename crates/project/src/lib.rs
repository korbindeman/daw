mod load;
mod save;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub use load::{
    ProjectMetadata, load_project, load_project_metadata, load_project_with_sample_rate,
};
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
    pub clips: Vec<ClipData>,
    pub volume: f32,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipData {
    pub start_tick: u64,
    pub end_tick: u64,
    pub audio_path: PathBuf,
    pub audio_offset: u64,
    pub name: String,
}

// Keep SegmentData as an alias for backwards compatibility with old project files
pub type SegmentData = ClipData;

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
                    clips: vec![
                        ClipData {
                            start_tick: 0,
                            end_tick: 960,
                            audio_path: PathBuf::from("audio/kick.wav"),
                            audio_offset: 0,
                            name: "Kick".to_string(),
                        },
                        ClipData {
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
                    clips: vec![ClipData {
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
            clips: vec![ClipData {
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
        assert_eq!(decoded.clips.len(), 1);
        assert_eq!(decoded.clips[0].start_tick, 1920);
        assert_eq!(
            decoded.clips[0].audio_path,
            PathBuf::from("samples/test.wav")
        );
    }

    #[test]
    fn test_clip_data_serialization() {
        let clip = ClipData {
            start_tick: 4800,
            end_tick: 5760,
            audio_path: PathBuf::from("/absolute/path/to/audio.wav"),
            audio_offset: 0,
            name: "Audio".to_string(),
        };

        let json = serde_json::to_string(&clip).expect("serialize");
        let decoded: ClipData = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded.start_tick, clip.start_tick);
        assert_eq!(decoded.end_tick, clip.end_tick);
        assert_eq!(decoded.audio_path, clip.audio_path);
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
    fn test_track_with_no_clips() {
        let track = TrackData {
            id: 5,
            name: "Empty Track".to_string(),
            clips: vec![],
            volume: 1.0,
            enabled: true,
        };

        let json = serde_json::to_string(&track).expect("serialize");
        let decoded: TrackData = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded.id, 5);
        assert!(decoded.clips.is_empty());
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
