use crate::{ClipData, Project, ProjectError, TrackData};
use daw_transport::Track;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

pub fn save_project(
    path: &Path,
    name: String,
    tempo: f64,
    time_signature: (u32, u32),
    tracks: &[Track],
    audio_paths: &std::collections::HashMap<u64, std::path::PathBuf>,
) -> Result<(), ProjectError> {
    let project = Project {
        name,
        tempo,
        time_signature,
        tracks: tracks
            .iter()
            .map(|track| TrackData {
                id: track.id.0,
                clips: track
                    .clips
                    .iter()
                    .map(|clip| ClipData {
                        id: clip.id.0,
                        start: clip.start,
                        audio_path: audio_paths
                            .get(&clip.id.0)
                            .cloned()
                            .unwrap_or_default(),
                    })
                    .collect(),
            })
            .collect(),
    };

    let file = File::create(path)?;
    let writer = BufWriter::new(file);
    rmp_serde::encode::write(&mut { writer }, &project)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use daw_transport::{AudioBuffer, Clip, ClipId, Track, TrackId, WaveformData};
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::tempdir;

    fn create_test_track() -> (Track, HashMap<u64, std::path::PathBuf>) {
        let audio = Arc::new(AudioBuffer {
            samples: vec![0.0; 1000],
            sample_rate: 44100,
            channels: 2,
        });
        let waveform = Arc::new(WaveformData::from_audio_buffer(&audio, 512));

        let track = Track {
            id: TrackId(1),
            clips: vec![
                Clip {
                    id: ClipId(100),
                    start: 0,
                    audio: audio.clone(),
                    waveform: waveform.clone(),
                },
                Clip {
                    id: ClipId(101),
                    start: 960,
                    audio: audio.clone(),
                    waveform: waveform.clone(),
                },
            ],
        };

        let mut audio_paths = HashMap::new();
        audio_paths.insert(100, PathBuf::from("audio/kick.wav"));
        audio_paths.insert(101, PathBuf::from("audio/snare.wav"));

        (track, audio_paths)
    }

    #[test]
    fn test_save_project_creates_file() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("test.dawproj");

        let (track, audio_paths) = create_test_track();

        save_project(
            &path,
            "Test Project".to_string(),
            120.0,
            (4, 4),
            &[track],
            &audio_paths,
        )
        .expect("save");

        assert!(path.exists());
    }

    #[test]
    fn test_save_project_content_is_valid_msgpack() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("test.dawproj");

        let (track, audio_paths) = create_test_track();

        save_project(
            &path,
            "My Song".to_string(),
            140.0,
            (3, 4),
            &[track],
            &audio_paths,
        )
        .expect("save");

        let file = std::fs::File::open(&path).expect("open");
        let reader = std::io::BufReader::new(file);
        let loaded: crate::Project = rmp_serde::decode::from_read(reader).expect("decode");

        assert_eq!(loaded.name, "My Song");
        assert_eq!(loaded.tempo, 140.0);
        assert_eq!(loaded.time_signature, (3, 4));
        assert_eq!(loaded.tracks.len(), 1);
        assert_eq!(loaded.tracks[0].clips.len(), 2);
    }

    #[test]
    fn test_save_empty_project() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("empty.dawproj");

        save_project(
            &path,
            "Empty".to_string(),
            120.0,
            (4, 4),
            &[],
            &HashMap::new(),
        )
        .expect("save");

        let file = std::fs::File::open(&path).expect("open");
        let reader = std::io::BufReader::new(file);
        let loaded: crate::Project = rmp_serde::decode::from_read(reader).expect("decode");

        assert!(loaded.tracks.is_empty());
    }

    #[test]
    fn test_save_project_with_missing_audio_path() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("missing_path.dawproj");

        let audio = Arc::new(AudioBuffer {
            samples: vec![0.0; 100],
            sample_rate: 44100,
            channels: 2,
        });
        let waveform = Arc::new(WaveformData::from_audio_buffer(&audio, 512));

        let track = Track {
            id: TrackId(1),
            clips: vec![Clip {
                id: ClipId(999),
                start: 0,
                audio,
                waveform,
            }],
        };

        save_project(
            &path,
            "Test".to_string(),
            120.0,
            (4, 4),
            &[track],
            &HashMap::new(),
        )
        .expect("save");

        let file = std::fs::File::open(&path).expect("open");
        let reader = std::io::BufReader::new(file);
        let loaded: crate::Project = rmp_serde::decode::from_read(reader).expect("decode");

        assert_eq!(loaded.tracks[0].clips[0].audio_path, PathBuf::new());
    }
}
