mod beats;
mod ui;

use daw_engine as engine;
use daw_transport::Command;
use daw_transport::Status;
use gpui::{
    App, Application, Context, Entity, Timer, Window, WindowOptions, black, div, prelude::*, px,
    rgb,
};
use std::time::Duration;
use ui::{Header, HeaderEvent, Playhead, Sidebar, TimelineRuler, Track};

struct Daw {
    tracks: Vec<daw_transport::Track>,
    engine_handle: engine::AudioEngineHandle,
    current_tick: u64,
    time_signature: (u32, u32), // (numerator, denominator) e.g., (4, 4)
    tempo: f64,
    pixels_per_beat: f64,
    header_handle: Entity<Header>,
    playhead_handle: Entity<Playhead>,
}

impl Daw {
    fn new(cx: &mut Context<Self>) -> Self {
        let time_signature = (4, 4);
        let tracks = beats::four_on_the_floor();
        let bpm = 120.0;
        let pixels_per_beat = 100.0;
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

        // Create playhead
        let playhead = cx.new(|_| Playhead::new(0, pixels_per_beat));

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
            tempo: bpm,
            pixels_per_beat,
            header_handle: header,
            playhead_handle: playhead,
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
                    self.playhead_handle.update(cx, |playhead, cx| {
                        playhead.set_tick(tick);
                        cx.notify();
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
        // Calculate timeline width based on furthest clip end
        let timeline_width = self.calculate_timeline_width();

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
                        div()
                            .flex_1()
                            .relative()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .pr(px(150.))
                                    .child(cx.new(|_| {
                                        TimelineRuler::new(
                                            self.pixels_per_beat,
                                            self.time_signature,
                                            timeline_width,
                                        )
                                    }))
                                    .child(
                                        div()
                                            .flex_1()
                                            .relative()
                                            .child(self.playhead_handle.clone())
                                            .children(self.tracks.iter().map(|track| {
                                                cx.new(|_| {
                                                    Track::new(
                                                        track.clone(),
                                                        self.pixels_per_beat,
                                                        self.tempo,
                                                        timeline_width,
                                                    )
                                                })
                                            })),
                                    ),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .right(px(0.))
                                    .top(px(0.))
                                    .w(px(150.))
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .h(px(20.))
                                            .bg(rgb(0xE8E8E8))
                                            .border_b_1()
                                            .border_color(rgb(0x000000)),
                                    )
                                    .children(self.tracks.iter().map(|track| {
                                        div()
                                            .h(px(80.))
                                            .border_b_1()
                                            .border_color(black())
                                            .border_l_1()
                                            .p_2()
                                            .flex()
                                            .items_center()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .child(format!("Track {}", track.id.0)),
                                            )
                                    })),
                            ),
                    ),
            )
    }
}

impl Daw {
    fn calculate_timeline_width(&self) -> f64 {
        use daw_transport::PPQN;

        let mut max_end_tick = 0u64;
        for track in &self.tracks {
            for clip in &track.clips {
                let duration_ticks = clip.duration_ticks(self.tempo);
                let end_tick = clip.start + duration_ticks;
                max_end_tick = max_end_tick.max(end_tick);
            }
        }

        // Add some padding at the end
        let end_with_padding = max_end_tick + (PPQN * 4); // 4 beats padding
        let content_width = (end_with_padding as f64 / PPQN as f64) * self.pixels_per_beat;

        // Ensure minimum width to fill the container
        let min_width = 1200.0; // Minimum timeline width
        content_width.max(min_width)
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let _window = cx
            .open_window(WindowOptions::default(), |_, cx| cx.new(|cx| Daw::new(cx)))
            .unwrap();
    });
}
