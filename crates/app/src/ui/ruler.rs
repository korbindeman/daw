use crate::theme::ActiveTheme;
use gpui::{Context, Window, div, prelude::*, px};

const RULER_HEIGHT: f32 = 20.0;

pub struct TimelineRuler {
    pixels_per_beat: f64,
    time_signature: (u32, u32),
    timeline_width: f64,
}

impl TimelineRuler {
    pub fn new(pixels_per_beat: f64, time_signature: (u32, u32), timeline_width: f64) -> Self {
        Self {
            pixels_per_beat,
            time_signature,
            timeline_width,
        }
    }
}

impl Render for TimelineRuler {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let beats_per_bar = self.time_signature.0;
        let total_beats = (self.timeline_width / self.pixels_per_beat).ceil() as u32;

        let mut markers = vec![];

        for beat in 0..=total_beats {
            let x_pos = beat as f64 * self.pixels_per_beat;
            let is_bar_start = beat % beats_per_bar == 0;
            let bar_number = beat / beats_per_bar + 1;

            if is_bar_start {
                markers.push(
                    div()
                        .absolute()
                        .left(px(x_pos as f32))
                        .top(px(0.))
                        .h_full()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.text)
                                .child(format!("{}", bar_number)),
                        )
                        .child(div().w(px(1.)).h_full().bg(theme.text_muted)),
                );
            } else {
                markers.push(
                    div()
                        .absolute()
                        .left(px(x_pos as f32))
                        .top(px(12.))
                        .h(px(8.))
                        .child(div().w(px(1.)).h_full().bg(theme.text_muted)),
                );
            }
        }

        div()
            .w(px(self.timeline_width as f32))
            .h(px(RULER_HEIGHT))
            .bg(theme.surface)
            .border_b_1()
            .border_color(theme.border)
            .relative()
            .children(markers)
    }
}
