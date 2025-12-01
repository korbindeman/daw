use std::{path::Path, time::Duration};

use std::sync::Arc;

use daw_engine::start;
use daw_transport::{Clip, ClipId, Command, Track, TrackId};

fn main() {
    let bongo_buffer =
        Arc::new(daw_decode::decode_file(Path::new("samples/cr78/bongo-l.wav")).unwrap());
    let guiro_buffer =
        Arc::new(daw_decode::decode_file(Path::new("samples/cr78/guiro-short.wav")).unwrap());
    let tracks = vec![
        Track {
            id: TrackId(0),
            clips: vec![
                Clip {
                    id: ClipId(0),
                    start: 0,
                    audio: bongo_buffer.clone(),
                },
                Clip {
                    id: ClipId(1),
                    start: 960 * 3, // beat 3
                    audio: bongo_buffer.clone(),
                },
            ],
        },
        Track {
            id: TrackId(1),
            clips: vec![Clip {
                id: ClipId(2),
                start: 960 * 2, // beat 2
                audio: guiro_buffer.clone(),
            }],
        },
    ];
    let mut handle = start(tracks, 120.0).unwrap();

    handle.commands.push(Command::Play).unwrap();

    loop {
        while let Ok(status) = handle.status.pop() {
            println!("position: {:?}", status);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
