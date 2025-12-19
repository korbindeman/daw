pub mod session;
pub mod time;

pub use session::{Metronome, PlaybackState, Session, SnapMode};
pub use time::{MusicalPosition, TimeContext, TimeSignature};

// Re-export utilities and data types needed by frontends
pub use daw_decode::strip_samples_root;
pub use daw_project::{ClipData, Project, ProjectError, SampleRef, TrackData};
pub use daw_transport::{AudioBuffer, Clip, PPQN, Track, TrackId, WaveformData, samples_to_ticks};

// Note: render_timeline, write_wav, save_project, and decode_file are intentionally NOT re-exported.
// These operations should go through Session methods to maintain proper encapsulation.
