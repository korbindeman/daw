//! Pure functions for clip operations, designed to be testable without the audio engine.

use daw_transport::{Clip, ClipId};

/// Result of resolving overlaps between a new clip and existing clips.
pub struct OverlapResolution {
    /// Clips to keep (possibly modified)
    pub clips: Vec<Clip>,
    /// The next clip ID to use (incremented if splits occurred)
    pub next_clip_id: u64,
}

/// Resolve overlaps between a new clip and a list of existing clips.
/// The new clip takes priority - existing clips are trimmed, split, or removed.
///
/// # Arguments
/// * `existing_clips` - The current clips on the track
/// * `new_clip` - The new clip being added (not included in output - caller adds it)
/// * `tempo` - Current tempo (needed to calculate clip durations)
/// * `next_clip_id` - The next available clip ID (used for split clips)
///
/// # Returns
/// The modified list of existing clips and the updated next_clip_id
pub fn resolve_overlaps(
    existing_clips: &[Clip],
    new_clip: &Clip,
    tempo: f64,
    mut next_clip_id: u64,
) -> OverlapResolution {
    let new_start = new_clip.start;
    let new_end = new_clip.end_ticks(tempo);

    let mut result_clips: Vec<Clip> = Vec::new();

    for existing in existing_clips {
        let existing_start = existing.start;
        let existing_end = existing.end_ticks(tempo);

        // Check if they overlap: ranges overlap if start_a < end_b && start_b < end_a
        if new_start < existing_end && existing_start < new_end {
            // They overlap - determine how to resolve
            if new_start <= existing_start && new_end >= existing_end {
                // Case 1: New clip completely covers existing - remove it (don't add to result)
                continue;
            } else if new_start > existing_start && new_end < existing_end {
                // Case 2: New clip is in the middle - split existing into two parts

                // Left part: from existing_start to new_start
                let mut left_clip = existing.clone();
                left_clip.length = Some(new_start - existing_start);
                result_clips.push(left_clip);

                // Right part: from new_end to existing_end
                // Need to calculate the offset into the audio for the right part
                let right_offset_ticks = new_end - existing_start;
                let right_offset_samples = ticks_to_samples_for_clip(
                    right_offset_ticks,
                    tempo,
                    existing.audio.sample_rate,
                );

                let mut right_clip = existing.clone();
                right_clip.id = ClipId(next_clip_id);
                next_clip_id += 1;
                right_clip.start = new_end;
                right_clip.offset = existing.offset + right_offset_samples;
                right_clip.length = Some(existing_end - new_end);
                result_clips.push(right_clip);
            } else if new_start <= existing_start {
                // Case 3: New clip covers the start of existing - trim existing's start
                let trim_amount = new_end - existing_start;
                let trim_samples = ticks_to_samples_for_clip(
                    trim_amount,
                    tempo,
                    existing.audio.sample_rate,
                );

                let mut trimmed = existing.clone();
                trimmed.start = new_end;
                trimmed.offset = existing.offset + trim_samples;

                // Update length
                if let Some(len) = existing.length {
                    trimmed.length = Some(len.saturating_sub(trim_amount));
                } else {
                    let full_duration = existing.full_duration_ticks(tempo);
                    trimmed.length = Some(full_duration.saturating_sub(trim_amount));
                }

                // Only add if there's still some length left
                if trimmed.length.map_or(true, |l| l > 0) {
                    result_clips.push(trimmed);
                }
            } else {
                // Case 4: New clip covers the end of existing - trim existing's end
                let new_length = new_start - existing_start;

                // Only add if there's still some length
                if new_length > 0 {
                    let mut trimmed = existing.clone();
                    trimmed.length = Some(new_length);
                    result_clips.push(trimmed);
                }
            }
        } else {
            // No overlap - keep the clip as-is
            result_clips.push(existing.clone());
        }
    }

    OverlapResolution {
        clips: result_clips,
        next_clip_id,
    }
}

/// Convert ticks to samples for a specific sample rate
fn ticks_to_samples_for_clip(ticks: u64, tempo: f64, sample_rate: u32) -> u64 {
    use daw_transport::PPQN;
    let seconds_per_beat = 60.0 / tempo;
    let seconds_per_tick = seconds_per_beat / PPQN as f64;
    let seconds = ticks as f64 * seconds_per_tick;
    (seconds * sample_rate as f64) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use daw_transport::{AudioBuffer, WaveformData};
    use std::sync::Arc;

    /// Create a test clip with a specific start and duration in ticks
    fn make_clip(id: u64, start: u64, duration_ticks: u64, tempo: f64) -> Clip {
        // Calculate how many samples we need for the given duration
        let sample_rate = 44100u32;
        let seconds_per_beat = 60.0 / tempo;
        let seconds_per_tick = seconds_per_beat / 960.0; // PPQN = 960
        let duration_seconds = duration_ticks as f64 * seconds_per_tick;
        let num_samples = (duration_seconds * sample_rate as f64) as usize;

        let audio = Arc::new(AudioBuffer {
            samples: vec![0.0; num_samples * 2], // stereo
            sample_rate,
            channels: 2,
        });
        let waveform = Arc::new(WaveformData {
            peaks: vec![],
            samples_per_bucket: 512,
        });

        Clip {
            id: ClipId(id),
            name: format!("Clip {}", id),
            start,
            audio,
            waveform,
            offset: 0,
            length: None, // Use full audio length
        }
    }

    const TEMPO: f64 = 120.0;
    const PPQN: u64 = 960;

    #[test]
    fn test_no_overlap() {
        // Existing: [0, 960)
        // New: [1920, 2880)
        // No overlap - existing should be unchanged
        let existing = vec![make_clip(1, 0, PPQN, TEMPO)];
        let new_clip = make_clip(2, PPQN * 2, PPQN, TEMPO);

        let result = resolve_overlaps(&existing, &new_clip, TEMPO, 100);

        assert_eq!(result.clips.len(), 1);
        assert_eq!(result.clips[0].start, 0);
        assert_eq!(result.clips[0].id.0, 1);
    }

    #[test]
    fn test_new_completely_covers_existing() {
        // Existing: [960, 1920)
        // New: [0, 2880)
        // Existing should be removed
        let existing = vec![make_clip(1, PPQN, PPQN, TEMPO)];
        let new_clip = make_clip(2, 0, PPQN * 3, TEMPO);

        let result = resolve_overlaps(&existing, &new_clip, TEMPO, 100);

        assert_eq!(result.clips.len(), 0, "Existing clip should be removed");
    }

    #[test]
    fn test_new_covers_start_of_existing() {
        // Existing: [0, 1920) - 2 beats
        // New: [0, 960) - 1 beat
        // Existing should be trimmed to [960, 1920)
        let existing = vec![make_clip(1, 0, PPQN * 2, TEMPO)];
        let new_clip = make_clip(2, 0, PPQN, TEMPO);

        let result = resolve_overlaps(&existing, &new_clip, TEMPO, 100);

        assert_eq!(result.clips.len(), 1);
        assert_eq!(result.clips[0].start, PPQN, "Clip should start at beat 2");
        assert_eq!(result.clips[0].length, Some(PPQN), "Clip should be 1 beat long");
        assert!(result.clips[0].offset > 0, "Offset should be set for trimmed start");
    }

    #[test]
    fn test_new_covers_end_of_existing() {
        // Existing: [0, 1920) - 2 beats
        // New: [960, 1920) - 1 beat at the end
        // Existing should be trimmed to [0, 960)
        let existing = vec![make_clip(1, 0, PPQN * 2, TEMPO)];
        let new_clip = make_clip(2, PPQN, PPQN, TEMPO);

        let result = resolve_overlaps(&existing, &new_clip, TEMPO, 100);

        assert_eq!(result.clips.len(), 1);
        assert_eq!(result.clips[0].start, 0, "Clip should still start at 0");
        assert_eq!(result.clips[0].length, Some(PPQN), "Clip should be 1 beat long");
        assert_eq!(result.clips[0].offset, 0, "Offset should remain 0");
    }

    #[test]
    fn test_new_in_middle_splits_existing() {
        // Existing: [0, 2880) - 3 beats
        // New: [960, 1920) - 1 beat in the middle
        // Existing should be split into [0, 960) and [1920, 2880)
        let existing = vec![make_clip(1, 0, PPQN * 3, TEMPO)];
        let new_clip = make_clip(2, PPQN, PPQN, TEMPO);

        let result = resolve_overlaps(&existing, &new_clip, TEMPO, 100);

        assert_eq!(result.clips.len(), 2, "Should have 2 clips after split");

        // Find left and right parts
        let left = result.clips.iter().find(|c| c.start == 0).expect("Should have left part");
        let right = result.clips.iter().find(|c| c.start == PPQN * 2).expect("Should have right part");

        assert_eq!(left.length, Some(PPQN), "Left part should be 1 beat");
        assert_eq!(left.offset, 0, "Left part offset should be 0");

        assert_eq!(right.length, Some(PPQN), "Right part should be 1 beat");
        assert!(right.offset > 0, "Right part should have offset");
        assert_eq!(right.id.0, 100, "Right part should have new ID");
    }

    #[test]
    fn test_multiple_overlapping_clips() {
        // Existing: [0, 960), [960, 1920), [1920, 2880)
        // New: [480, 2400) - overlaps all three
        // First should be trimmed, second removed, third trimmed
        let existing = vec![
            make_clip(1, 0, PPQN, TEMPO),
            make_clip(2, PPQN, PPQN, TEMPO),
            make_clip(3, PPQN * 2, PPQN, TEMPO),
        ];
        let new_clip = make_clip(4, PPQN / 2, PPQN * 2, TEMPO);

        let result = resolve_overlaps(&existing, &new_clip, TEMPO, 100);

        // Clip 1: [0, 960) overlapped by [480, 2400) -> trimmed to [0, 480)
        // Clip 2: [960, 1920) completely covered by [480, 2400) -> removed
        // Clip 3: [1920, 2880) overlapped by [480, 2400) -> trimmed to [2400, 2880)

        assert_eq!(result.clips.len(), 2, "Should have 2 clips remaining");

        let first = result.clips.iter().find(|c| c.id.0 == 1).expect("Clip 1 should exist");
        assert_eq!(first.start, 0);
        assert_eq!(first.length, Some(PPQN / 2), "First clip trimmed to half beat");

        let third = result.clips.iter().find(|c| c.id.0 == 3).expect("Clip 3 should exist");
        assert_eq!(third.start, PPQN / 2 + PPQN * 2, "Third clip starts after new clip ends");
    }

    #[test]
    fn test_adjacent_clips_no_overlap() {
        // Existing: [0, 960)
        // New: [960, 1920) - starts exactly where existing ends
        // Should not overlap (end is exclusive)
        let existing = vec![make_clip(1, 0, PPQN, TEMPO)];
        let new_clip = make_clip(2, PPQN, PPQN, TEMPO);

        let result = resolve_overlaps(&existing, &new_clip, TEMPO, 100);

        assert_eq!(result.clips.len(), 1);
        assert_eq!(result.clips[0].start, 0);
        assert_eq!(result.clips[0].id.0, 1);
    }

    #[test]
    fn test_next_clip_id_incremented_on_split() {
        // When a clip is split, next_clip_id should be incremented
        let existing = vec![make_clip(1, 0, PPQN * 3, TEMPO)];
        let new_clip = make_clip(2, PPQN, PPQN, TEMPO);

        let result = resolve_overlaps(&existing, &new_clip, TEMPO, 100);

        assert_eq!(result.next_clip_id, 101, "next_clip_id should be incremented");
    }
}
