use daw_transport::PPQN;
use gpui::{Context, EventEmitter, Window, black, div, prelude::*, px};

pub struct Header {
    current_tick: u64,
    time_signature: (u32, u32),
    bpm: f64,
    pub playing: bool,
}

pub enum HeaderEvent {
    Play,
    Pause,
    Stop,
}

impl EventEmitter<HeaderEvent> for Header {}

impl Header {
    pub fn new(current_tick: u64, time_signature: (u32, u32), bpm: f64) -> Self {
        Self {
            current_tick,
            time_signature,
            bpm,
            playing: false,
        }
    }

    fn ticks_to_musical_time(&self, ticks: u64) -> (u32, u32, u32) {
        let (numerator, _denominator) = self.time_signature;

        let ticks_per_beat = PPQN;
        let ticks_per_sixteenth = PPQN / 4;
        let ticks_per_bar = ticks_per_beat * numerator as u64;

        let bar = (ticks / ticks_per_bar) + 1;
        let beat = ((ticks % ticks_per_bar) / ticks_per_beat) + 1;
        let division = ((ticks % ticks_per_beat) / ticks_per_sixteenth) + 1;

        (bar as u32, beat as u32, division as u32)
    }

    fn format_musical_time(&self, ticks: u64) -> String {
        let (bar, beat, division) = self.ticks_to_musical_time(ticks);
        format!("{}.{}.{}", bar, beat, division)
    }

    fn format_seconds(&self, ticks: u64) -> String {
        let seconds_per_beat = 60.0 / self.bpm;
        let seconds_per_tick = seconds_per_beat / PPQN as f64;
        let total_seconds = ticks as f64 * seconds_per_tick;
        let minutes = (total_seconds / 60.0) as u32;
        let seconds = total_seconds % 60.0;
        format!("{}:{:05.2}", minutes, seconds)
    }

    pub fn set_tick(&mut self, tick: u64, cx: &mut Context<Self>) {
        self.current_tick = tick;
        cx.notify();
    }

    pub fn set_playing(&mut self, playing: bool, cx: &mut Context<Self>) {
        self.playing = playing;
        cx.notify();
    }
}

impl Render for Header {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let musical_time = self.format_musical_time(self.current_tick);
        let time_seconds = self.format_seconds(self.current_tick);

        div()
            .w_full()
            .h(px(50.))
            .border_b_1()
            .border_color(black())
            .flex()
            .gap_2()
            .p_2()
            .items_center()
            .child(
                div()
                    .id("play-pause-button")
                    .px_4()
                    .py_2()
                    .border_1()
                    .border_color(black())
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            if this.playing {
                                cx.emit(HeaderEvent::Pause);
                            } else {
                                cx.emit(HeaderEvent::Play);
                            }
                        }),
                    )
                    .child(if self.playing { "Pause" } else { "Play" }),
            )
            .child(
                div()
                    .id("stop-button")
                    .px_4()
                    .py_2()
                    .border_1()
                    .border_color(black())
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|_, _, _, cx| {
                            cx.emit(HeaderEvent::Stop);
                        }),
                    )
                    .child("Stop"),
            )
            .child(div().ml_auto().child(format!("{} BPM", self.bpm)))
            .child(div().ml_4().child(musical_time))
            .child(div().ml_4().child(time_seconds))
    }
}
