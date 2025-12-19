mod load;
mod save;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub use load::{
    LoadedProject, OfflineClip, ProjectMetadata, load_project, load_project_metadata,
    load_project_with_sample_rate,
};
pub use save::save_project;

/// A reference to an audio sample with explicit path semantics.
///
/// Instead of storing raw `PathBuf`s, we store typed references that make
/// the path resolution explicit and unambiguous.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", content = "path")]
pub enum SampleRef {
    /// Path relative to the dev/workspace root's `samples/` directory.
    /// e.g., "cr78/kick-accent.wav" resolves to `{dev_root}/samples/cr78/kick-accent.wav`
    #[serde(rename = "dev_root")]
    DevRoot(PathBuf),

    /// Path relative to the project file's directory.
    /// e.g., "audio/kick.wav" resolves to `{project_dir}/audio/kick.wav`
    #[serde(rename = "project")]
    ProjectRelative(PathBuf),
}

impl SampleRef {
    /// Get the relative path portion of this reference.
    pub fn path(&self) -> &Path {
        match self {
            SampleRef::DevRoot(p) => p,
            SampleRef::ProjectRelative(p) => p,
        }
    }
}

impl std::fmt::Display for SampleRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SampleRef::DevRoot(p) => write!(f, "dev_root:{}", p.display()),
            SampleRef::ProjectRelative(p) => write!(f, "project:{}", p.display()),
        }
    }
}

/// Context for resolving sample references to absolute paths.
///
/// Contains the various root directories needed to resolve different
/// types of `SampleRef` values.
#[derive(Debug, Clone)]
pub struct PathContext {
    /// Root directory of the project file (parent of .dawproj file).
    pub project_root: PathBuf,

    /// Optional dev/workspace root for resolving DevRoot samples.
    /// When set, DevRoot("cr78/kick.wav") resolves to `{dev_root}/samples/cr78/kick.wav`.
    pub dev_root: Option<PathBuf>,
}

impl PathContext {
    /// Create a new PathContext from a project file path.
    ///
    /// The project_root is set to the parent directory of the project file.
    /// dev_root must be set separately if needed.
    pub fn from_project_path(project_path: &Path) -> Self {
        Self {
            project_root: project_path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_default(),
            dev_root: None,
        }
    }

    /// Set the dev root directory.
    pub fn with_dev_root(mut self, dev_root: PathBuf) -> Self {
        self.dev_root = Some(dev_root);
        self
    }

    /// Resolve a SampleRef to an absolute path.
    ///
    /// Returns `None` if the resolved path doesn't exist or if the required
    /// root directory is not configured.
    pub fn resolve(&self, sample_ref: &SampleRef) -> Option<PathBuf> {
        match sample_ref {
            SampleRef::DevRoot(rel_path) => {
                let dev_root = self.dev_root.as_ref()?;
                let resolved = dev_root.join("samples").join(rel_path);
                if resolved.exists() {
                    Some(resolved)
                } else {
                    None
                }
            }
            SampleRef::ProjectRelative(rel_path) => {
                let resolved = self.project_root.join(rel_path);
                if resolved.exists() {
                    Some(resolved)
                } else {
                    None
                }
            }
        }
    }
}

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
    pub pan: f32,
    pub enabled: bool,
    pub solo: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipData {
    pub start_tick: u64,
    pub end_tick: u64,
    /// Reference to the audio sample for this clip.
    pub sample_ref: SampleRef,
    pub audio_offset: u64,
    pub name: String,
}

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
                            sample_ref: SampleRef::DevRoot(PathBuf::from("audio/kick.wav")),
                            audio_offset: 0,
                            name: "Kick".to_string(),
                        },
                        ClipData {
                            start_tick: 960,
                            end_tick: 1920,
                            sample_ref: SampleRef::DevRoot(PathBuf::from("audio/snare.wav")),
                            audio_offset: 0,
                            name: "Snare".to_string(),
                        },
                    ],
                    volume: 1.0,
                    pan: 0.0,
                    enabled: true,
                    solo: false,
                },
                TrackData {
                    id: 2,
                    name: "Hi-Hats".to_string(),
                    clips: vec![ClipData {
                        start_tick: 480,
                        end_tick: 960,
                        sample_ref: SampleRef::DevRoot(PathBuf::from("audio/hihat.wav")),
                        audio_offset: 0,
                        name: "Hi-Hat".to_string(),
                    }],
                    volume: 0.8,
                    pan: 0.0,
                    enabled: true,
                    solo: false,
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
                sample_ref: SampleRef::DevRoot(PathBuf::from("samples/test.wav")),
                audio_offset: 0,
                name: "Test".to_string(),
            }],
            volume: 0.75,
            pan: 0.0,
            enabled: true,
            solo: false,
        };

        let json = serde_json::to_string(&track).expect("serialize");
        let decoded: TrackData = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded.id, 42);
        assert_eq!(decoded.clips.len(), 1);
        assert_eq!(decoded.clips[0].start_tick, 1920);
        assert_eq!(
            decoded.clips[0].sample_ref,
            SampleRef::DevRoot(PathBuf::from("samples/test.wav"))
        );
    }

    #[test]
    fn test_clip_data_serialization() {
        let clip = ClipData {
            start_tick: 4800,
            end_tick: 5760,
            sample_ref: SampleRef::ProjectRelative(PathBuf::from("audio/local.wav")),
            audio_offset: 0,
            name: "Audio".to_string(),
        };

        let json = serde_json::to_string(&clip).expect("serialize");
        let decoded: ClipData = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded.start_tick, clip.start_tick);
        assert_eq!(decoded.end_tick, clip.end_tick);
        assert_eq!(decoded.sample_ref, clip.sample_ref);
    }

    #[test]
    fn test_sample_ref_serialization() {
        // Test DevRoot serialization
        let dev_root = SampleRef::DevRoot(PathBuf::from("cr78/kick.wav"));
        let json = serde_json::to_string(&dev_root).expect("serialize");
        assert!(json.contains("dev_root"));
        let decoded: SampleRef = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded, dev_root);

        // Test ProjectRelative serialization
        let project_rel = SampleRef::ProjectRelative(PathBuf::from("audio/local.wav"));
        let json = serde_json::to_string(&project_rel).expect("serialize");
        assert!(json.contains("project"));
        let decoded: SampleRef = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded, project_rel);
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
            pan: 0.0,
            enabled: true,
            solo: false,
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

    #[test]
    fn test_path_context_resolution() {
        use tempfile::tempdir;

        let temp = tempdir().unwrap();
        let project_dir = temp.path().join("projects");
        let dev_root = temp.path();

        std::fs::create_dir_all(&project_dir).unwrap();
        std::fs::create_dir_all(dev_root.join("samples/cr78")).unwrap();

        // Create a test audio file
        let test_audio = dev_root.join("samples/cr78/kick.wav");
        std::fs::write(&test_audio, b"fake wav").unwrap();

        let ctx = PathContext {
            project_root: project_dir.clone(),
            dev_root: Some(dev_root.to_path_buf()),
        };

        // Test DevRoot resolution
        let sample_ref = SampleRef::DevRoot(PathBuf::from("cr78/kick.wav"));
        let resolved = ctx.resolve(&sample_ref);
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap(), test_audio);

        // Test missing file returns None
        let missing_ref = SampleRef::DevRoot(PathBuf::from("cr78/missing.wav"));
        assert!(ctx.resolve(&missing_ref).is_none());
    }
}
