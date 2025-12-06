use crate::theme::ActiveTheme;
use crate::ui::primitives::Input;
use daw_core::PPQN;
use gpui::{Context, Entity, EventEmitter, FocusHandle, Focusable, Window, div, prelude::*, px};

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
        let theme = cx.theme();
        let (bar, beat, division) =
            self.ticks_to_musical_time(self.current_tick, self.time_sig_numerator);
        let time_seconds = self.format_seconds(self.current_tick, self.bpm);

        div()
            .w_full()
            .h(px(50.))
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
                            .bg(theme.background)
                            .border_1()
                            .border_color(theme.border)
                            .text_color(theme.text)
                            .child(self.bpm_input.clone()),
                    )
                    .child(div().text_color(theme.text).child("BPM"))
                    .child(
                        div()
                            .id("metronome-button")
                            .px_2()
                            .py_1()
                            .ml_2()
                            .bg(if self.metronome_enabled {
                                theme.element_active
                            } else {
                                theme.element
                            })
                            .border_1()
                            .border_color(theme.border)
                            .text_color(theme.text)
                            .hover(|s| s.bg(theme.element_hover))
                            .active(|s| s.bg(theme.element_active))
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(|_, _, _, cx| {
                                    cx.emit(HeaderEvent::ToggleMetronome);
                                }),
                            )
                            .child(if self.metronome_enabled { "M" } else { "M" }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .flex()
                                    .gap_1()
                                    .child(
                                        div()
                                            .w(px(32.))
                                            .h(px(20.))
                                            .px_1()
                                            .bg(theme.background)
                                            .border_1()
                                            .border_color(theme.border)
                                            .text_color(theme.text)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(format!("{}", bar)),
                                    )
                                    .child(
                                        div()
                                            .w(px(32.))
                                            .h(px(20.))
                                            .px_1()
                                            .bg(theme.background)
                                            .border_1()
                                            .border_color(theme.border)
                                            .text_color(theme.text)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(format!("{}", beat)),
                                    )
                                    .child(
                                        div()
                                            .w(px(32.))
                                            .h(px(20.))
                                            .px_1()
                                            .bg(theme.background)
                                            .border_1()
                                            .border_color(theme.border)
                                            .text_color(theme.text)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(format!("{}", division)),
                                    ),
                            )
                            .child(
                                div()
                                    .text_color(theme.text)
                                    .text_size(px(10.))
                                    .child(time_seconds),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                div()
                                    .id("play-pause-button")
                                    .px_4()
                                    .py_2()
                                    .bg(theme.element)
                                    .border_1()
                                    .border_color(theme.border)
                                    .text_color(theme.text)
                                    .hover(|s| s.bg(theme.element_hover))
                                    .active(|s| s.bg(theme.element_active))
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
                                div()
                                    .id("stop-button")
                                    .px_4()
                                    .py_2()
                                    .bg(theme.element)
                                    .border_1()
                                    .border_color(theme.border)
                                    .text_color(theme.text)
                                    .hover(|s| s.bg(theme.element_hover))
                                    .active(|s| s.bg(theme.element_active))
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
