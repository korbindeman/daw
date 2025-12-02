mod beats;
mod ui;

use daw_engine as engine;
use daw_transport::Command;
use daw_transport::Status;
use gpui::{App, Application, Context, Entity, Timer, Window, WindowOptions, div, prelude::*, rgb};
use std::time::Duration;
use ui::{Header, HeaderEvent, Sidebar, Track};

struct Daw {
    tracks: Vec<daw_transport::Track>,
    engine_handle: engine::AudioEngineHandle,
    current_tick: u64,
    time_signature: (u32, u32), // (numerator, denominator) e.g., (4, 4)
    header_handle: Entity<Header>,
}

impl Daw {
    fn new(cx: &mut Context<Self>) -> Self {
        let time_signature = (4, 4);
        let tracks = beats::four_on_the_floor();
        let bpm = 120.0;
        let engine_handle = engine::start(tracks.clone(), bpm).unwrap();

        // Create header and subscribe to its events
        let header = cx.new(|_| Header::new(0, time_signature, bpm));
        cx.subscribe(
            &header,
            |this, header, event: &HeaderEvent, cx| match event {
                HeaderEvent::Play => this.play(&header, cx),
                HeaderEvent::Pause => this.pause(&header, cx),
                HeaderEvent::Stop => this.stop(&header, cx),
            },
        )
        .detach();

        // Start a timer to poll playback position and update the header
        // TODO: I don't really know how this works
        cx.spawn(
            async |this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                loop {
                    Timer::after(Duration::from_millis(16)).await;
                    let result = cx.update(|cx| {
                        this.update(cx, |daw, cx| {
                            daw.poll_status(cx);
                        })
                    });
                    if result.is_err() {
                        break;
                    }
                }
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

    fn poll_status(&mut self, cx: &mut Context<Self>) {
        while let Ok(status) = self.engine_handle.status.pop() {
            match status {
                Status::Position(tick) => {
                    self.current_tick = tick;
                    self.header_handle.update(cx, |header, cx| {
                        header.set_tick(tick, cx);
                    });
                }
            }
        }
    }

    fn play(&mut self, header: &Entity<Header>, cx: &mut Context<Self>) {
        let _ = self.engine_handle.commands.push(Command::Play);
        header.update(cx, |header, cx| header.set_playing(true, cx));
    }

    fn pause(&mut self, header: &Entity<Header>, cx: &mut Context<Self>) {
        let _ = self.engine_handle.commands.push(Command::Pause);
        header.update(cx, |header, cx| header.set_playing(false, cx));
    }

    fn stop(&mut self, header: &Entity<Header>, cx: &mut Context<Self>) {
        let _ = self.engine_handle.commands.push(Command::Pause);
        let _ = self.engine_handle.commands.push(Command::Seek { tick: 0 });
        header.update(cx, |header, cx| header.set_playing(false, cx));
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
