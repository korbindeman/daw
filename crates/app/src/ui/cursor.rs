use crate::theme::ActiveTheme;
use daw_core::PPQN;
use gpui::{Context, Window, div, prelude::*, px};

pub struct Cursor {
    current_tick: Option<u64>,
    pixels_per_beat: f64,
}

impl Cursor {
    pub fn new(current_tick: Option<u64>, pixels_per_beat: f64) -> Self {
        Self {
            current_tick,
            pixels_per_beat,
        }
    }

    pub fn set_tick(&mut self, tick: Option<u64>) {
        self.current_tick = tick;
    }
}

impl Render for Cursor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        if let Some(tick) = self.current_tick {
            let x_pos = (tick as f64 / PPQN as f64) * self.pixels_per_beat;

            div()
                .absolute()
                .left(px(x_pos as f32))
                .top(px(0.))
                .bottom(px(0.))
                .w(px(1.))
                .bg(theme.text_muted)
                .opacity(0.8)
        } else {
            // Don't render if cursor is not set
            div().hidden()
        }
    }
}
