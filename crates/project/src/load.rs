use crate::{PathContext, Project, ProjectError, SampleRef};
use daw_transport::{Clip, Track, TrackId, WaveformData};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;

/// Information about a clip whose audio file could not be loaded.
#[derive(Debug, Clone)]
pub struct OfflineClip {
    /// The track this clip belongs to
    pub track_id: TrackId,
    /// The original sample reference from the project file
    pub sample_ref: SampleRef,
    /// The clip's position on the timeline
    pub start_tick: u64,
    pub end_tick: u64,
    /// The clip's name
    pub name: String,
    /// Error message describing why the audio couldn't be loaded
    pub error: String,
}

#[derive(Debug)]
pub struct LoadedProject {
    pub name: String,
    pub tempo: f64,
    pub time_signature: (u32, u32),
    pub tracks: Vec<Track>,
    /// Mapping from clip name to sample reference
    pub sample_refs: HashMap<String, SampleRef>,
    /// Audio cache used during loading (contains decoded and resampled audio)
    pub cache: daw_decode::AudioCache,
    /// Clips that couldn't be loaded due to missing or invalid audio files
    pub offline_clips: Vec<OfflineClip>,
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

pub fn load_project(path: &Path, ctx: &PathContext) -> Result<LoadedProject, ProjectError> {
    load_project_with_sample_rate(path, None, ctx)
}

pub fn load_project_with_sample_rate(
    path: &Path,
    target_sample_rate: Option<u32>,
    ctx: &PathContext,
) -> Result<LoadedProject, ProjectError> {
    let project = load_project_data(path)?;

    let mut cache = daw_decode::AudioCache::new();
    let mut tracks = Vec::new();
    let mut sample_refs = HashMap::new();
    let mut offline_clips = Vec::new();

    for track_data in &project.tracks {
        let mut track = Track::new(TrackId(track_data.id), track_data.name.clone());
        track.volume = track_data.volume;
        track.pan = track_data.pan;
        track.enabled = track_data.enabled;
        track.solo = track_data.solo;

        for clip_data in &track_data.clips {
            // Try to resolve the sample reference to an absolute path
            let resolved_path = ctx.resolve(&clip_data.sample_ref);

            match resolved_path {
                Some(abs_path) => {
                    // Try to load the audio
                    match cache.get_or_load_direct(&abs_path, target_sample_rate) {
                        Ok(audio) => {
                            sample_refs
                                .insert(clip_data.name.clone(), clip_data.sample_ref.clone());

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
                        Err(e) => {
                            // Audio file exists but couldn't be decoded
                            offline_clips.push(OfflineClip {
                                track_id: TrackId(track_data.id),
                                sample_ref: clip_data.sample_ref.clone(),
                                start_tick: clip_data.start_tick,
                                end_tick: clip_data.end_tick,
                                name: clip_data.name.clone(),
                                error: format!("Failed to decode: {}", e),
                            });
                        }
                    }
                }
                None => {
                    // Sample reference couldn't be resolved (file not found)
                    offline_clips.push(OfflineClip {
                        track_id: TrackId(track_data.id),
                        sample_ref: clip_data.sample_ref.clone(),
                        start_tick: clip_data.start_tick,
                        end_tick: clip_data.end_tick,
                        name: clip_data.name.clone(),
                        error: format!(
                            "Sample not found: {:?}",
                            clip_data.sample_ref.path().display()
                        ),
                    });
                }
            }
        }

        tracks.push(track);
    }

    Ok(LoadedProject {
        name: project.name,
        tempo: project.tempo,
        time_signature: project.time_signature,
        tracks,
        sample_refs,
        cache,
        offline_clips,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClipData, Project, TrackData};
    use std::path::PathBuf;
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
        let ctx = PathContext {
            project_root: PathBuf::from("/nonexistent"),
            dev_root: None,
        };
        let result = load_project(Path::new("/nonexistent/project.dawproj"), &ctx);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProjectError::Io(_)));
    }

    #[test]
    fn test_load_project_invalid_format() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("invalid.dawproj");
        std::fs::write(&path, b"not valid json or msgpack").expect("write");

        let ctx = PathContext::from_project_path(&path);
        let result = load_project(&path, &ctx);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProjectError::Deserialize(_)));
    }

    #[test]
    fn test_load_project_with_project_relative_sample() {
        let dir = tempdir().expect("tempdir");
        let project_path = dir.path().join("test.dawproj");
        let audio_path = dir.path().join("sample.wav");

        write_test_wav(&audio_path);

        let project = Project {
            name: "Project Relative Test".to_string(),
            tempo: 120.0,
            time_signature: (4, 4),
            tracks: vec![TrackData {
                id: 1,
                name: "Sample Track".to_string(),
                clips: vec![ClipData {
                    start_tick: 0,
                    end_tick: 960,
                    sample_ref: SampleRef::ProjectRelative(PathBuf::from("sample.wav")),
                    audio_offset: 0,
                    name: "Sample Clip".to_string(),
                }],
                volume: 1.0,
                pan: 0.0,
                enabled: true,
                solo: false,
            }],
        };

        let file = std::fs::File::create(&project_path).expect("create");
        let writer = std::io::BufWriter::new(file);
        serde_json::to_writer(writer, &project).expect("encode");

        let ctx = PathContext::from_project_path(&project_path);
        let loaded = load_project(&project_path, &ctx).expect("load");

        assert_eq!(loaded.tracks[0].clips()[0].start_tick, 0);
        assert!(loaded.offline_clips.is_empty());
    }

    #[test]
    fn test_load_project_with_dev_root_sample() {
        let dir = tempdir().expect("tempdir");
        let project_dir = dir.path().join("projects");
        let samples_dir = dir.path().join("samples").join("drums");
        std::fs::create_dir_all(&project_dir).expect("create project dir");
        std::fs::create_dir_all(&samples_dir).expect("create samples dir");

        let project_path = project_dir.join("test.dawproj");
        let audio_path = samples_dir.join("kick.wav");

        write_test_wav(&audio_path);

        let project = Project {
            name: "Dev Root Test".to_string(),
            tempo: 120.0,
            time_signature: (4, 4),
            tracks: vec![TrackData {
                id: 1,
                name: "Drums".to_string(),
                clips: vec![ClipData {
                    start_tick: 0,
                    end_tick: 960,
                    sample_ref: SampleRef::DevRoot(PathBuf::from("drums/kick.wav")),
                    audio_offset: 0,
                    name: "Kick".to_string(),
                }],
                volume: 1.0,
                pan: 0.0,
                enabled: true,
                solo: false,
            }],
        };

        let file = std::fs::File::create(&project_path).expect("create");
        let writer = std::io::BufWriter::new(file);
        serde_json::to_writer(writer, &project).expect("encode");

        let ctx = PathContext::from_project_path(&project_path)
            .with_dev_root(dir.path().to_path_buf());
        let loaded = load_project(&project_path, &ctx).expect("load");

        assert_eq!(loaded.tracks[0].clips()[0].start_tick, 0);
        assert!(loaded.offline_clips.is_empty());
    }

    #[test]
    fn test_load_project_missing_audio_creates_offline_clip() {
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
                    start_tick: 0,
                    end_tick: 960,
                    sample_ref: SampleRef::ProjectRelative(PathBuf::from("nonexistent.wav")),
                    audio_offset: 0,
                    name: "Missing Clip".to_string(),
                }],
                volume: 1.0,
                pan: 0.0,
                enabled: true,
                solo: false,
            }],
        };

        let file = std::fs::File::create(&project_path).expect("create");
        let writer = std::io::BufWriter::new(file);
        serde_json::to_writer(writer, &project).expect("encode");

        let ctx = PathContext::from_project_path(&project_path);
        let loaded = load_project(&project_path, &ctx).expect("load");

        // Project should load successfully
        assert_eq!(loaded.name, "Missing Audio");
        // Track should exist but have no clips (the clip is offline)
        assert_eq!(loaded.tracks.len(), 1);
        assert!(loaded.tracks[0].clips().is_empty());
        // Offline clip should be recorded
        assert_eq!(loaded.offline_clips.len(), 1);
        assert_eq!(loaded.offline_clips[0].name, "Missing Clip");
        assert!(loaded.offline_clips[0].error.contains("Sample not found"));
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

        let ctx = PathContext::from_project_path(&project_path);
        let loaded = load_project(&project_path, &ctx).expect("load");

        assert_eq!(loaded.name, "Empty");
        assert_eq!(loaded.tempo, 90.0);
        assert_eq!(loaded.time_signature, (6, 8));
        assert!(loaded.tracks.is_empty());
        assert!(loaded.sample_refs.is_empty());
        assert!(loaded.offline_clips.is_empty());
    }
}
