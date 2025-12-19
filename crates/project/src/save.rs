use crate::{ClipData, Project, ProjectError, SampleRef, TrackData};
use daw_transport::Track;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

pub fn save_project(
    path: &Path,
    name: String,
    tempo: f64,
    time_signature: (u32, u32),
    tracks: &[Track],
    sample_refs: &HashMap<String, SampleRef>,
) -> Result<(), ProjectError> {
    let project = Project {
        name,
        tempo,
        time_signature,
        tracks: tracks
            .iter()
            .map(|track| TrackData {
                id: track.id.0,
                name: track.name.clone(),
                clips: track
                    .clips()
                    .iter()
                    .filter_map(|clip| {
                        // Only save clips that have a sample reference
                        sample_refs.get(&clip.name).map(|sample_ref| ClipData {
                            name: clip.name.clone(),
                            start_tick: clip.start_tick,
                            end_tick: clip.end_tick,
                            audio_offset: clip.audio_offset,
                            sample_ref: sample_ref.clone(),
                        })
                    })
                    .collect(),
                volume: track.volume,
                pan: track.pan,
                enabled: track.enabled,
                solo: track.solo,
            })
            .collect(),
    };

    let file = File::create(path)?;
    let writer = BufWriter::new(file);
    serde_json::to_writer_pretty(writer, &project)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use daw_transport::{AudioArc, Clip, Track, TrackId, WaveformData};
    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::tempdir;

    fn create_test_track() -> (Track, HashMap<String, SampleRef>) {
        let audio = AudioArc::new(vec![0.0; 1000], 44100, 2);
        let waveform = Arc::new(WaveformData::from_audio_arc(&audio, 512));

        let mut track = Track::new(TrackId(1), "Test Track".to_string());
        track.volume = 0.9;
        track.insert_clip(Clip {
            start_tick: 0,
            end_tick: 960,
            audio: audio.clone(),
            waveform: waveform.clone(),
            audio_offset: 0,
            name: "Kick".to_string(),
        });
        track.insert_clip(Clip {
            start_tick: 960,
            end_tick: 1920,
            audio: audio.clone(),
            waveform: waveform.clone(),
            audio_offset: 0,
            name: "Snare".to_string(),
        });

        let mut sample_refs = HashMap::new();
        sample_refs.insert(
            "Kick".to_string(),
            SampleRef::DevRoot(PathBuf::from("drums/kick.wav")),
        );
        sample_refs.insert(
            "Snare".to_string(),
            SampleRef::DevRoot(PathBuf::from("drums/snare.wav")),
        );

        (track, sample_refs)
    }

    #[test]
    fn test_save_project_creates_file() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("test.dawproj");

        let (track, sample_refs) = create_test_track();

        save_project(
            &path,
            "Test Project".to_string(),
            120.0,
            (4, 4),
            &[track],
            &sample_refs,
        )
        .expect("save");

        assert!(path.exists());
    }

    #[test]
    fn test_save_project_content_is_valid_json() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("test.dawproj");

        let (track, sample_refs) = create_test_track();

        save_project(
            &path,
            "My Song".to_string(),
            140.0,
            (3, 4),
            &[track],
            &sample_refs,
        )
        .expect("save");

        let file = std::fs::File::open(&path).expect("open");
        let reader = std::io::BufReader::new(file);
        let loaded: crate::Project = serde_json::from_reader(reader).expect("decode");

        assert_eq!(loaded.name, "My Song");
        assert_eq!(loaded.tempo, 140.0);
        assert_eq!(loaded.time_signature, (3, 4));
        assert_eq!(loaded.tracks.len(), 1);
        assert_eq!(loaded.tracks[0].clips.len(), 2);
        // Verify sample refs are saved correctly
        assert_eq!(
            loaded.tracks[0].clips[0].sample_ref,
            SampleRef::DevRoot(PathBuf::from("drums/kick.wav"))
        );
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
        let loaded: crate::Project = serde_json::from_reader(reader).expect("decode");

        assert!(loaded.tracks.is_empty());
    }

    #[test]
    fn test_save_project_clips_without_sample_ref_are_skipped() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("missing_ref.dawproj");

        let audio = AudioArc::new(vec![0.0; 100], 44100, 2);
        let waveform = Arc::new(WaveformData::from_audio_arc(&audio, 512));

        let mut track = Track::new(TrackId(1), "Track".to_string());
        track.insert_clip(Clip {
            start_tick: 0,
            end_tick: 960,
            audio,
            waveform,
            audio_offset: 0,
            name: "Clip Without Ref".to_string(),
        });

        // Save with empty sample_refs - clip should be skipped
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
        let loaded: crate::Project = serde_json::from_reader(reader).expect("decode");

        // Track exists but has no clips (the clip was skipped)
        assert_eq!(loaded.tracks.len(), 1);
        assert!(loaded.tracks[0].clips.is_empty());
    }
}
