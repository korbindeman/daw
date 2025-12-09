pub mod session;
pub mod time;

pub use session::{Metronome, PlaybackState, Session};
pub use time::{MusicalPosition, TimeContext, TimeSignature};

// Re-export utilities and data types needed by frontends
pub use daw_decode::strip_samples_root;
pub use daw_project::{ClipData, Project, ProjectError, TrackData};
pub use daw_transport::{
    AudioBuffer, PPQN, Segment, Track, TrackId, WaveformData, samples_to_ticks,
};

// Note: render_timeline, write_wav, save_project, and decode_file are intentionally NOT re-exported.
// These operations should go through Session methods to maintain proper encapsulation.
