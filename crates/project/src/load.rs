use crate::{Project, ProjectError};
use daw_transport::{Clip, Track, TrackId, WaveformData};
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
    /// Mapping from clip name to audio file path
    pub audio_paths: HashMap<String, std::path::PathBuf>,
    /// Audio cache used during loading (contains decoded and resampled audio)
    pub cache: daw_decode::AudioCache,
}

#[derive(Debug, Clone)]
pub struct ProjectMetadata {
    pub name: String,
    pub tempo: f64,
    pub time_signature: (u32, u32),
    pub track_count: usize,
    pub segment_count: usize,
}

fn load_project_data(path: &Path) -> Result<Project, ProjectError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    // Try JSON first, fall back to MessagePack
    serde_json::from_reader(reader).or_else(|_| {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        rmp_serde::decode::from_read(reader).map_err(ProjectError::from)
    })
}

pub fn load_project_metadata(path: &Path) -> Result<ProjectMetadata, ProjectError> {
    let project = load_project_data(path)?;

    let clip_count: usize = project.tracks.iter().map(|t| t.clips.len()).sum();

    Ok(ProjectMetadata {
        name: project.name,
        tempo: project.tempo,
        time_signature: project.time_signature,
        track_count: project.tracks.len(),
        segment_count: clip_count,
    })
}

pub fn load_project(path: &Path) -> Result<LoadedProject, ProjectError> {
    load_project_with_sample_rate(path, None)
}

pub fn load_project_with_sample_rate(
    path: &Path,
    target_sample_rate: Option<u32>,
) -> Result<LoadedProject, ProjectError> {
    let project = load_project_data(path)?;

    let mut cache = daw_decode::AudioCache::new();
    let mut tracks = Vec::new();
    let mut audio_paths = HashMap::new();

    for track_data in &project.tracks {
        let mut track = Track::new(TrackId(track_data.id), track_data.name.clone());
        track.volume = track_data.volume;
        track.enabled = track_data.enabled;

        for clip_data in &track_data.clips {
            let audio = cache
                .get_or_load(&clip_data.audio_path, target_sample_rate)
                .map_err(|e| ProjectError::AudioDecode {
                    path: clip_data.audio_path.clone(),
                    source: e,
                })?;

            audio_paths.insert(clip_data.name.clone(), clip_data.audio_path.clone());

            let waveform = WaveformData::from_audio_arc(&audio, 512);

            track.insert_clip(Clip {
                start_tick: clip_data.start_tick,
                end_tick: clip_data.end_tick,
                audio,
                waveform: Arc::new(waveform),
                audio_offset: clip_data.audio_offset,
                name: clip_data.name.clone(),
            });
        }

        tracks.push(track);
    }

    Ok(LoadedProject {
        name: project.name,
        tempo: project.tempo,
        time_signature: project.time_signature,
        tracks,
        audio_paths,
        cache,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Project, SegmentData, TrackData, save_project};
    use daw_transport::{AudioArc, Clip, Track, TrackId, WaveformData};
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
        std::fs::write(&path, b"not valid json or msgpack").expect("write");

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

        let audio = AudioArc::new(vec![0.0; 100], 44100, 2);
        let waveform = Arc::new(WaveformData::from_audio_arc(&audio, 512));

        let mut original_track = Track::new(TrackId(1), "Roundtrip Track".to_string());
        original_track.volume = 0.85;
        original_track.insert_clip(Clip {
            start_tick: 960,
            end_tick: 1920,
            audio,
            waveform,
            audio_offset: 0,
            name: "Test Clip".to_string(),
        });

        let mut audio_paths = HashMap::new();
        audio_paths.insert("Test Clip".to_string(), audio_path.clone());

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
        assert_eq!(loaded.tracks[0].clips().len(), 1);
        assert_eq!(loaded.tracks[0].clips()[0].start_tick, 960);
        assert_eq!(loaded.tracks[0].clips()[0].end_tick, 1920);
    }

    #[test]
    fn test_load_project_with_audio_path() {
        let dir = tempdir().expect("tempdir");
        let project_path = dir.path().join("test.dawproj");
        let audio_path = dir.path().join("sample.wav");

        write_test_wav(&audio_path);

        let project = Project {
            name: "Audio Path Test".to_string(),
            tempo: 120.0,
            time_signature: (4, 4),
            tracks: vec![TrackData {
                id: 1,
                name: "Sample Track".to_string(),
                clips: vec![SegmentData {
                    start_tick: 0,
                    end_tick: 960,
                    audio_path: audio_path.clone(),
                    audio_offset: 0,
                    name: "Sample Clip".to_string(),
                }],
                volume: 1.0,
                enabled: true,
            }],
        };

        let file = std::fs::File::create(&project_path).expect("create");
        let writer = std::io::BufWriter::new(file);
        serde_json::to_writer(writer, &project).expect("encode");

        let loaded = load_project(&project_path).expect("load");

        assert_eq!(loaded.tracks[0].clips()[0].start_tick, 0);
        assert_eq!(loaded.audio_paths.get("Sample Clip").unwrap(), &audio_path);
    }

    #[test]
    fn test_load_legacy_msgpack_project() {
        let dir = tempdir().expect("tempdir");
        let project_path = dir.path().join("legacy.dawproj");
        let audio_path = dir.path().join("sample.wav");

        write_test_wav(&audio_path);

        let project = Project {
            name: "Legacy MsgPack Test".to_string(),
            tempo: 120.0,
            time_signature: (4, 4),
            tracks: vec![TrackData {
                id: 1,
                name: "Sample Track".to_string(),
                clips: vec![SegmentData {
                    start_tick: 0,
                    end_tick: 960,
                    audio_path: audio_path.clone(),
                    audio_offset: 0,
                    name: "Sample Clip".to_string(),
                }],
                volume: 1.0,
                enabled: true,
            }],
        };

        // Write as MessagePack (legacy format)
        let file = std::fs::File::create(&project_path).expect("create");
        let writer = std::io::BufWriter::new(file);
        rmp_serde::encode::write(&mut { writer }, &project).expect("encode");

        let loaded = load_project(&project_path).expect("load");

        assert_eq!(loaded.name, "Legacy MsgPack Test");
        assert_eq!(loaded.tracks[0].clips()[0].start_tick, 0);
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
                clips: vec![SegmentData {
                    start_tick: 0,
                    end_tick: 960,
                    audio_path: PathBuf::from("nonexistent.wav"),
                    audio_offset: 0,
                    name: "Missing Clip".to_string(),
                }],
                volume: 1.0,
                enabled: true,
            }],
        };

        let file = std::fs::File::create(&project_path).expect("create");
        let writer = std::io::BufWriter::new(file);
        serde_json::to_writer(writer, &project).expect("encode");

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
        serde_json::to_writer(writer, &project).expect("encode");

        let loaded = load_project(&project_path).expect("load");

        assert_eq!(loaded.name, "Empty");
        assert_eq!(loaded.tempo, 90.0);
        assert_eq!(loaded.time_signature, (6, 8));
        assert!(loaded.tracks.is_empty());
        assert!(loaded.audio_paths.is_empty());
    }
}
