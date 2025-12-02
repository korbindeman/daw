use daw_transport::PPQN;
use gpui::{Context, EventEmitter, Window, black, div, prelude::*, px};

pub struct Header {
    current_tick: u64,
    time_signature: (u32, u32),
}

pub enum HeaderEvent {
    Play,
    Stop,
}

impl EventEmitter<HeaderEvent> for Header {}

impl Header {
    pub fn new(current_tick: u64, time_signature: (u32, u32)) -> Self {
        Self {
            current_tick,
            time_signature,
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
}

impl Render for Header {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let musical_time = self.format_musical_time(self.current_tick);

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
                    .id("play-button")
                    .px_4()
                    .py_2()
                    .border_1()
                    .border_color(black())
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|_, _, _, cx| {
                            cx.emit(HeaderEvent::Play);
                        }),
                    )
                    .child("Play"),
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
            .child(div().ml_auto().child(musical_time))
    }
}
