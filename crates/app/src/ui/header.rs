use crate::theme::ActiveTheme;
use crate::ui::primitives::{
    Input,
    button::{button, button_active},
};
use daw_core::PPQN;
use gpui::{Context, Entity, EventEmitter, FocusHandle, Focusable, Window, div, prelude::*, px};

const HEADER_HEIGHT: f32 = 50.0;

pub struct Header {
    current_tick: u64,
    pub playing: bool,
    pub metronome_enabled: bool,
    bpm: f64,
    time_sig_numerator: u32,
    time_sig_denominator: u32,
    bpm_input: Entity<Input>,
    time_sig_numerator_input: Entity<Input>,
    time_sig_denominator_input: Entity<Input>,
    focus_handle: FocusHandle,
}

pub enum HeaderEvent {
    Play,
    Pause,
    Stop,
    ToggleMetronome,
}

impl EventEmitter<HeaderEvent> for Header {}

impl Header {
    pub fn new(
        bpm: f64,
        time_sig_numerator: u32,
        time_sig_denominator: u32,
        cx: &mut Context<Self>,
    ) -> Self {
        let bpm_input = cx.new(|cx| {
            Input::new(cx.focus_handle())
                .content(format!("{}", bpm))
                .numeric_only(true)
                .on_change(move |_text, _window, _cx| {
                    // BPM changes will be handled externally
                })
        });

        let time_sig_numerator_input = cx.new(|cx| {
            Input::new(cx.focus_handle())
                .content(format!("{}", time_sig_numerator))
                .numeric_only(true)
                .on_change(move |_text, _window, _cx| {
                    // Time signature changes will be handled externally
                })
        });

        let time_sig_denominator_input = cx.new(|cx| {
            Input::new(cx.focus_handle())
                .content(format!("{}", time_sig_denominator))
                .numeric_only(true)
                .on_change(move |_text, _window, _cx| {
                    // Time signature changes will be handled externally
                })
        });

        Self {
            current_tick: 0,
            playing: false,
            metronome_enabled: false,
            bpm,
            time_sig_numerator,
            time_sig_denominator,
            bpm_input,
            time_sig_numerator_input,
            time_sig_denominator_input,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn update_values(
        &mut self,
        bpm: f64,
        time_sig_numerator: u32,
        time_sig_denominator: u32,
        cx: &mut Context<Self>,
    ) {
        self.bpm = bpm;
        self.time_sig_numerator = time_sig_numerator;
        self.time_sig_denominator = time_sig_denominator;

        self.bpm_input.update(cx, |input, cx| {
            input.set_content(format!("{}", bpm), cx);
        });
        self.time_sig_numerator_input.update(cx, |input, cx| {
            input.set_content(format!("{}", time_sig_numerator), cx);
        });
        self.time_sig_denominator_input.update(cx, |input, cx| {
            input.set_content(format!("{}", time_sig_denominator), cx);
        });
    }

    fn ticks_to_musical_time(&self, ticks: u64, time_sig_numerator: u32) -> (u32, u32, u32) {
        let ticks_per_beat = PPQN;
        let ticks_per_sixteenth = PPQN / 4;
        let ticks_per_bar = ticks_per_beat * time_sig_numerator as u64;

        let bar = (ticks / ticks_per_bar) + 1;
        let beat = ((ticks % ticks_per_bar) / ticks_per_beat) + 1;
        let division = ((ticks % ticks_per_beat) / ticks_per_sixteenth) + 1;

        (bar as u32, beat as u32, division as u32)
    }

    fn format_seconds(&self, ticks: u64, bpm: f64) -> String {
        let seconds_per_beat = 60.0 / bpm;
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

    /// Set tick without triggering a notification (for batched updates)
    pub fn set_tick_silent(&mut self, tick: u64, _cx: &mut Context<Self>) {
        self.current_tick = tick;
    }

    pub fn set_playing(&mut self, playing: bool, cx: &mut Context<Self>) {
        self.playing = playing;
        cx.notify();
    }

    pub fn set_metronome_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.metronome_enabled = enabled;
        cx.notify();
    }
}

impl Focusable for Header {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Header {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (bar, beat, division) =
            self.ticks_to_musical_time(self.current_tick, self.time_sig_numerator);

        div()
            .w_full()
            .h(px(HEADER_HEIGHT))
            .bg(theme.header)
            .border_b_1()
            .border_color(theme.border)
            .flex()
            .gap_2()
            .p_2()
            .items_center()
            .justify_between()
            .text_color(theme.background)
            .child(
                div()
                    .flex_1()
                    .flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .items_center()
                            .child(
                                div()
                                    .w(px(32.))
                                    .h(px(28.))
                                    .bg(theme.background)
                                    .border_1()
                                    .border_color(theme.border)
                                    .text_color(theme.text)
                                    .child(self.time_sig_numerator_input.clone()),
                            )
                            .child(div().text_color(theme.text).child("/"))
                            .child(
                                div()
                                    .w(px(32.))
                                    .h(px(28.))
                                    .bg(theme.background)
                                    .border_1()
                                    .border_color(theme.border)
                                    .text_color(theme.text)
                                    .child(self.time_sig_denominator_input.clone()),
                            ),
                    )
                    .child(
                        div()
                            .w(px(60.))
                            .h(px(28.))
                            .bg(theme.background)
                            .border_1()
                            .border_color(theme.border)
                            .text_color(theme.text)
                            .child(self.bpm_input.clone()),
                    )
                    .child(div().text_color(theme.text).child("BPM"))
                    .child(
                        button_active("metronome-button", self.metronome_enabled, cx)
                            .ml_2()
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(|_, _, _, cx| {
                                    cx.emit(HeaderEvent::ToggleMetronome);
                                }),
                            )
                            .child("M"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(
                        div()
                            .h(px(28.))
                            .px_1()
                            .py(px(2.))
                            .bg(theme.background)
                            .border_1()
                            .border_color(theme.border)
                            .text_color(theme.text)
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .w(px(24.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(format!("{}.", bar)),
                            )
                            .child(
                                div()
                                    .w(px(24.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(format!("{}.", beat)),
                            )
                            .child(
                                div()
                                    .w(px(24.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(format!("{}", division)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                button("play-pause-button", cx)
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
                                    .child(if self.playing { "⏸" } else { "▶" }),
                            )
                            .child(
                                button("stop-button", cx)
                                    .on_mouse_down(
                                        gpui::MouseButton::Left,
                                        cx.listener(|_, _, _, cx| {
                                            cx.emit(HeaderEvent::Stop);
                                        }),
                                    )
                                    .child("⏹"),
                            ),
                    ),
            )
            .child(div().flex_1())
    }
}
