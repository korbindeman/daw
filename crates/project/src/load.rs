use crate::{Project, ProjectError};
use daw_transport::{Clip, ClipId, Track, TrackId, WaveformData};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug)]
pub struct LoadedProject {
    pub name: String,
    pub tempo: f64,
    pub time_signature: (u32, u32),
    pub tracks: Vec<Track>,
    pub audio_paths: HashMap<u64, std::path::PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ProjectMetadata {
    pub name: String,
    pub tempo: f64,
    pub time_signature: (u32, u32),
    pub track_count: usize,
    pub clip_count: usize,
}

pub fn load_project_metadata(path: &Path) -> Result<ProjectMetadata, ProjectError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let project: Project = rmp_serde::decode::from_read(reader)?;

    let clip_count: usize = project.tracks.iter().map(|t| t.clips.len()).sum();

    Ok(ProjectMetadata {
        name: project.name,
        tempo: project.tempo,
        time_signature: project.time_signature,
        track_count: project.tracks.len(),
        clip_count,
    })
}

pub fn load_project(path: &Path) -> Result<LoadedProject, ProjectError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let project: Project = rmp_serde::decode::from_read(reader)?;

    let mut tracks = Vec::new();
    let mut audio_paths = HashMap::new();

    for track_data in &project.tracks {
        let mut clips = Vec::new();

        for clip_data in &track_data.clips {
            let audio_buffer = daw_decode::decode_file(&clip_data.audio_path).map_err(|e| {
                ProjectError::AudioDecode {
                    path: clip_data.audio_path.clone(),
                    source: e,
                }
            })?;

            audio_paths.insert(clip_data.id, clip_data.audio_path.clone());

            let waveform = WaveformData::from_audio_buffer(&audio_buffer, 512);

            clips.push(Clip {
                id: ClipId(clip_data.id),
                name: clip_data.name.clone(),
                start: clip_data.start,
                audio: Arc::new(audio_buffer),
                waveform: Arc::new(waveform),
            });
        }

        let track = Track {
            id: TrackId(track_data.id),
            name: track_data.name.clone(),
            clips,
            volume: track_data.volume,
        };
        tracks.push(track);
    }

    Ok(LoadedProject {
        name: project.name,
        tempo: project.tempo,
        time_signature: project.time_signature,
        tracks,
        audio_paths,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClipData, Project, TrackData, save_project};
    use daw_transport::{AudioBuffer, Clip, ClipId, Track, TrackId, WaveformData};
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::tempdir;

    fn write_test_wav(path: &Path) {
        use std::io::Write;
        let mut file = std::fs::File::create(path).expect("create wav");
        let header: [u8; 44] = [
            0x52, 0x49, 0x46, 0x46, // "RIFF"
            0x24, 0x00, 0x00, 0x00, // file size - 8
            0x57, 0x41, 0x56, 0x45, // "WAVE"
            0x66, 0x6d, 0x74, 0x20, // "fmt "
            0x10, 0x00, 0x00, 0x00, // format chunk size
            0x01, 0x00, // PCM format
            0x01, 0x00, // 1 channel
            0x44, 0xac, 0x00, 0x00, // 44100 sample rate
            0x88, 0x58, 0x01, 0x00, // byte rate
            0x02, 0x00, // block align
            0x10, 0x00, // bits per sample
            0x64, 0x61, 0x74, 0x61, // "data"
            0x00, 0x00, 0x00, 0x00, // data size (0 samples)
        ];
        file.write_all(&header).expect("write wav header");
    }

    #[test]
    fn test_load_project_file_not_found() {
        let result = load_project(Path::new("/nonexistent/project.dawproj"));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProjectError::Io(_)));
    }

    #[test]
    fn test_load_project_invalid_format() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("invalid.dawproj");
        std::fs::write(&path, b"not valid msgpack").expect("write");

        let result = load_project(&path);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProjectError::Deserialize(_)));
    }

    #[test]
    fn test_load_project_roundtrip() {
        let dir = tempdir().expect("tempdir");
        let project_path = dir.path().join("test.dawproj");
        let audio_path = dir.path().join("audio.wav");

        write_test_wav(&audio_path);

        let audio = Arc::new(AudioBuffer {
            samples: vec![0.0; 100],
            sample_rate: 44100,
            channels: 2,
        });
        let waveform = Arc::new(WaveformData::from_audio_buffer(&audio, 512));

        let original_track = Track {
            id: TrackId(1),
            name: "Roundtrip Track".to_string(),
            clips: vec![Clip {
                id: ClipId(100),
                name: "Test Clip".to_string(),
                start: 960,
                audio,
                waveform,
            }],
            volume: 0.85,
        };

        let mut audio_paths = HashMap::new();
        audio_paths.insert(100, audio_path.clone());

        save_project(
            &project_path,
            "Roundtrip Test".to_string(),
            128.0,
            (4, 4),
            &[original_track],
            &audio_paths,
        )
        .expect("save");

        let loaded = load_project(&project_path).expect("load");

        assert_eq!(loaded.name, "Roundtrip Test");
        assert_eq!(loaded.tempo, 128.0);
        assert_eq!(loaded.time_signature, (4, 4));
        assert_eq!(loaded.tracks.len(), 1);
        assert_eq!(loaded.tracks[0].id.0, 1);
        assert_eq!(loaded.tracks[0].clips.len(), 1);
        assert_eq!(loaded.tracks[0].clips[0].id.0, 100);
        assert_eq!(loaded.tracks[0].clips[0].start, 960);
    }

    #[test]
    fn test_load_project_relative_audio_path() {
        let dir = tempdir().expect("tempdir");
        let project_path = dir.path().join("test.dawproj");
        let audio_dir = dir.path().join("audio");
        std::fs::create_dir(&audio_dir).expect("create audio dir");
        let audio_path = audio_dir.join("sample.wav");

        write_test_wav(&audio_path);

        let project = Project {
            name: "Relative Path Test".to_string(),
            tempo: 120.0,
            time_signature: (4, 4),
            tracks: vec![TrackData {
                id: 1,
                name: "Sample Track".to_string(),
                clips: vec![ClipData {
                    id: 100,
                    start: 0,
                    audio_path: PathBuf::from("audio/sample.wav"),
                    name: "Sample Clip".to_string(),
                }],
                volume: 1.0,
            }],
        };

        let file = std::fs::File::create(&project_path).expect("create");
        let writer = std::io::BufWriter::new(file);
        rmp_serde::encode::write(&mut { writer }, &project).expect("encode");

        let loaded = load_project(&project_path).expect("load");

        assert_eq!(loaded.tracks[0].clips[0].id.0, 100);
        assert_eq!(
            loaded.audio_paths.get(&100).unwrap(),
            &PathBuf::from("audio/sample.wav")
        );
    }

    #[test]
    fn test_load_project_missing_audio_file() {
        let dir = tempdir().expect("tempdir");
        let project_path = dir.path().join("test.dawproj");

        let project = Project {
            name: "Missing Audio".to_string(),
            tempo: 120.0,
            time_signature: (4, 4),
            tracks: vec![TrackData {
                id: 1,
                name: "Missing Track".to_string(),
                clips: vec![ClipData {
                    id: 100,
                    start: 0,
                    audio_path: PathBuf::from("nonexistent.wav"),
                    name: "Missing Clip".to_string(),
                }],
                volume: 1.0,
            }],
        };

        let file = std::fs::File::create(&project_path).expect("create");
        let writer = std::io::BufWriter::new(file);
        rmp_serde::encode::write(&mut { writer }, &project).expect("encode");

        let result = load_project(&project_path);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ProjectError::AudioDecode { .. }
        ));
    }

    #[test]
    fn test_load_empty_project() {
        let dir = tempdir().expect("tempdir");
        let project_path = dir.path().join("empty.dawproj");

        let project = Project {
            name: "Empty".to_string(),
            tempo: 90.0,
            time_signature: (6, 8),
            tracks: vec![],
        };

        let file = std::fs::File::create(&project_path).expect("create");
        let writer = std::io::BufWriter::new(file);
        rmp_serde::encode::write(&mut { writer }, &project).expect("encode");

        let loaded = load_project(&project_path).expect("load");

        assert_eq!(loaded.name, "Empty");
        assert_eq!(loaded.tempo, 90.0);
        assert_eq!(loaded.time_signature, (6, 8));
        assert!(loaded.tracks.is_empty());
        assert!(loaded.audio_paths.is_empty());
    }
}
