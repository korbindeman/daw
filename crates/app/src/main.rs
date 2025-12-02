mod ui;

use std::{path::Path, sync::Arc};

use daw_engine as engine;
use daw_transport::{Clip, ClipId, Command, Track as TransportTrack, TrackId};
use gpui::{App, Application, Context, Entity, Window, WindowOptions, div, prelude::*, rgb};
use ui::{Header, HeaderEvent, Sidebar, Track};

struct Daw {
    tracks: Vec<TransportTrack>,
    engine_handle: engine::AudioEngineHandle,
    current_tick: u64,
    time_signature: (u32, u32), // (numerator, denominator) e.g., (4, 4)
    header_handle: Entity<Header>,
}

impl Daw {
    fn new(cx: &mut Context<Self>) -> Self {
        let time_signature = (4, 4);

        let kick_buffer =
            Arc::new(daw_decode::decode_file(Path::new("samples/cr78/kick.wav")).unwrap());
        let hihat_buffer =
            Arc::new(daw_decode::decode_file(Path::new("samples/cr78/hihat.wav")).unwrap());

        let tracks = vec![
            TransportTrack {
                id: TrackId(0),
                clips: vec![
                    Clip {
                        id: ClipId(0),
                        start: 0,
                        audio: kick_buffer.clone(),
                    },
                    Clip {
                        id: ClipId(1),
                        start: 960 * 3, // beat 3
                        audio: kick_buffer.clone(),
                    },
                ],
            },
            TransportTrack {
                id: TrackId(1),
                clips: vec![Clip {
                    id: ClipId(2),
                    start: 0, // beat 2
                    audio: hihat_buffer.clone(),
                }],
            },
        ];

        let engine_handle = engine::start(tracks.clone(), 120.0).unwrap();

        // Create header and subscribe to its events
        let header = cx.new(|_| Header::new(0, time_signature));
        cx.subscribe(
            &header,
            |this, _header, event: &HeaderEvent, _cx| match event {
                HeaderEvent::Play => this.play(),
                HeaderEvent::Stop => this.stop(),
            },
        )
        .detach();

        Self {
            tracks,
            engine_handle,
            current_tick: 0,
            time_signature,
            header_handle: header,
        }
    }

    fn play(&mut self) {
        let _ = self.engine_handle.commands.push(Command::Play);
    }

    fn _pause(&mut self) {
        let _ = self.engine_handle.commands.push(Command::Pause);
    }

    fn stop(&mut self) {
        let _ = self.engine_handle.commands.push(Command::Pause);
        let _ = self.engine_handle.commands.push(Command::Seek { tick: 0 });
    }
}

impl Render for Daw {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(0xD3D0D1))
            .flex()
            .flex_col()
            .child(self.header_handle.clone())
            .child(
                div()
                    .flex()
                    .flex_1()
                    .child(cx.new(|_| Sidebar::new()))
                    .child(
                        div().flex_1().flex().flex_col().children(
                            self.tracks
                                .iter()
                                .map(|track| cx.new(|_| Track::new(track.clone()))),
                        ),
                    ),
            )
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let _window = cx
            .open_window(WindowOptions::default(), |_, cx| cx.new(|cx| Daw::new(cx)))
            .unwrap();
    });
}
