mod load;
mod save;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub use load::{ProjectMetadata, load_project, load_project_metadata};
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipData {
    pub id: u64,
    pub start: u64,
    pub audio_path: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialize(#[from] rmp_serde::encode::Error),

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
                            id: 100,
                            start: 0,
                            audio_path: PathBuf::from("audio/kick.wav"),
                        },
                        ClipData {
                            id: 101,
                            start: 960,
                            audio_path: PathBuf::from("audio/snare.wav"),
                        },
                    ],
                },
                TrackData {
                    id: 2,
                    name: "Hi-Hats".to_string(),
                    clips: vec![ClipData {
                        id: 200,
                        start: 480,
                        audio_path: PathBuf::from("audio/hihat.wav"),
                    }],
                },
            ],
        }
    }

    #[test]
    fn test_project_serialization_roundtrip() {
        let project = sample_project();

        let bytes = rmp_serde::encode::to_vec(&project).expect("serialize");
        let decoded: Project = rmp_serde::decode::from_slice(&bytes).expect("deserialize");

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
                id: 1,
                start: 1920,
                audio_path: PathBuf::from("samples/test.wav"),
            }],
        };

        let bytes = rmp_serde::encode::to_vec(&track).expect("serialize");
        let decoded: TrackData = rmp_serde::decode::from_slice(&bytes).expect("deserialize");

        assert_eq!(decoded.id, 42);
        assert_eq!(decoded.clips.len(), 1);
        assert_eq!(decoded.clips[0].id, 1);
        assert_eq!(decoded.clips[0].start, 1920);
        assert_eq!(
            decoded.clips[0].audio_path,
            PathBuf::from("samples/test.wav")
        );
    }

    #[test]
    fn test_clip_data_serialization() {
        let clip = ClipData {
            id: 99,
            start: 4800,
            audio_path: PathBuf::from("/absolute/path/to/audio.wav"),
        };

        let bytes = rmp_serde::encode::to_vec(&clip).expect("serialize");
        let decoded: ClipData = rmp_serde::decode::from_slice(&bytes).expect("deserialize");

        assert_eq!(decoded.id, clip.id);
        assert_eq!(decoded.start, clip.start);
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

        let bytes = rmp_serde::encode::to_vec(&project).expect("serialize");
        let decoded: Project = rmp_serde::decode::from_slice(&bytes).expect("deserialize");

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
        };

        let bytes = rmp_serde::encode::to_vec(&track).expect("serialize");
        let decoded: TrackData = rmp_serde::decode::from_slice(&bytes).expect("deserialize");

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
