use std::{path::Path, sync::Arc};

use daw_transport::{Clip, ClipId, Track, TrackId};

const PPQN: u64 = 960;

/// Basic four-on-the-floor beat with kick, snare, and hihats
pub fn four_on_the_floor() -> Vec<Track> {
    let kick = Arc::new(daw_decode::decode_file(Path::new("samples/cr78/kick.wav")).unwrap());
    let snare = Arc::new(daw_decode::decode_file(Path::new("samples/cr78/snare.wav")).unwrap());
    let hihat = Arc::new(daw_decode::decode_file(Path::new("samples/cr78/hihat.wav")).unwrap());

    vec![
        // Kick on every beat
        Track {
            id: TrackId(0),
            clips: vec![
                Clip {
                    id: ClipId(0),
                    start: 0,
                    audio: kick.clone(),
                },
                Clip {
                    id: ClipId(1),
                    start: PPQN,
                    audio: kick.clone(),
                },
                Clip {
                    id: ClipId(2),
                    start: PPQN * 2,
                    audio: kick.clone(),
                },
                Clip {
                    id: ClipId(3),
                    start: PPQN * 3,
                    audio: kick.clone(),
                },
            ],
        },
        // Snare on beats 2 and 4
        Track {
            id: TrackId(1),
            clips: vec![
                Clip {
                    id: ClipId(4),
                    start: PPQN,
                    audio: snare.clone(),
                },
                Clip {
                    id: ClipId(5),
                    start: PPQN * 3,
                    audio: snare.clone(),
                },
            ],
        },
        // Hihats on eighth notes
        Track {
            id: TrackId(2),
            clips: vec![
                Clip {
                    id: ClipId(6),
                    start: 0,
                    audio: hihat.clone(),
                },
                Clip {
                    id: ClipId(7),
                    start: PPQN / 2,
                    audio: hihat.clone(),
                },
                Clip {
                    id: ClipId(8),
                    start: PPQN,
                    audio: hihat.clone(),
                },
                Clip {
                    id: ClipId(9),
                    start: PPQN + PPQN / 2,
                    audio: hihat.clone(),
                },
                Clip {
                    id: ClipId(10),
                    start: PPQN * 2,
                    audio: hihat.clone(),
                },
                Clip {
                    id: ClipId(11),
                    start: PPQN * 2 + PPQN / 2,
                    audio: hihat.clone(),
                },
                Clip {
                    id: ClipId(12),
                    start: PPQN * 3,
                    audio: hihat.clone(),
                },
                Clip {
                    id: ClipId(13),
                    start: PPQN * 3 + PPQN / 2,
                    audio: hihat.clone(),
                },
            ],
        },
    ]
}

/// Hip-hop style beat with syncopated kick
pub fn hip_hop() -> Vec<Track> {
    let kick =
        Arc::new(daw_decode::decode_file(Path::new("samples/cr78/kick-accent.wav")).unwrap());
    let snare =
        Arc::new(daw_decode::decode_file(Path::new("samples/cr78/snare-accent.wav")).unwrap());
    let hihat = Arc::new(daw_decode::decode_file(Path::new("samples/cr78/hihat.wav")).unwrap());

    vec![
        // Kick on 1, and syncopated before beat 3
        Track {
            id: TrackId(0),
            clips: vec![
                Clip {
                    id: ClipId(0),
                    start: 0,
                    audio: kick.clone(),
                },
                Clip {
                    id: ClipId(1),
                    start: PPQN * 2 - PPQN / 4,
                    audio: kick.clone(),
                },
            ],
        },
        // Snare on beats 2 and 4
        Track {
            id: TrackId(1),
            clips: vec![
                Clip {
                    id: ClipId(2),
                    start: PPQN,
                    audio: snare.clone(),
                },
                Clip {
                    id: ClipId(3),
                    start: PPQN * 3,
                    audio: snare.clone(),
                },
            ],
        },
        // Hihats on eighth notes
        Track {
            id: TrackId(2),
            clips: vec![
                Clip {
                    id: ClipId(4),
                    start: 0,
                    audio: hihat.clone(),
                },
                Clip {
                    id: ClipId(5),
                    start: PPQN / 2,
                    audio: hihat.clone(),
                },
                Clip {
                    id: ClipId(6),
                    start: PPQN,
                    audio: hihat.clone(),
                },
                Clip {
                    id: ClipId(7),
                    start: PPQN + PPQN / 2,
                    audio: hihat.clone(),
                },
                Clip {
                    id: ClipId(8),
                    start: PPQN * 2,
                    audio: hihat.clone(),
                },
                Clip {
                    id: ClipId(9),
                    start: PPQN * 2 + PPQN / 2,
                    audio: hihat.clone(),
                },
                Clip {
                    id: ClipId(10),
                    start: PPQN * 3,
                    audio: hihat.clone(),
                },
                Clip {
                    id: ClipId(11),
                    start: PPQN * 3 + PPQN / 2,
                    audio: hihat.clone(),
                },
            ],
        },
    ]
}

/// Bossa nova inspired beat with rim and percussion
pub fn bossa() -> Vec<Track> {
    let kick = Arc::new(daw_decode::decode_file(Path::new("samples/cr78/kick.wav")).unwrap());
    let rim = Arc::new(daw_decode::decode_file(Path::new("samples/cr78/rim.wav")).unwrap());
    let conga = Arc::new(daw_decode::decode_file(Path::new("samples/cr78/conga-l.wav")).unwrap());

    vec![
        // Kick on 1 and 3
        Track {
            id: TrackId(0),
            clips: vec![
                Clip {
                    id: ClipId(0),
                    start: 0,
                    audio: kick.clone(),
                },
                Clip {
                    id: ClipId(1),
                    start: PPQN * 2,
                    audio: kick.clone(),
                },
            ],
        },
        // Rim click pattern
        Track {
            id: TrackId(1),
            clips: vec![
                Clip {
                    id: ClipId(2),
                    start: PPQN / 2,
                    audio: rim.clone(),
                },
                Clip {
                    id: ClipId(3),
                    start: PPQN + PPQN / 2,
                    audio: rim.clone(),
                },
                Clip {
                    id: ClipId(4),
                    start: PPQN * 2 + PPQN / 2,
                    audio: rim.clone(),
                },
                Clip {
                    id: ClipId(5),
                    start: PPQN * 3,
                    audio: rim.clone(),
                },
            ],
        },
        // Conga accents
        Track {
            id: TrackId(2),
            clips: vec![
                Clip {
                    id: ClipId(6),
                    start: PPQN,
                    audio: conga.clone(),
                },
                Clip {
                    id: ClipId(7),
                    start: PPQN * 3 + PPQN / 2,
                    audio: conga.clone(),
                },
            ],
        },
    ]
}
