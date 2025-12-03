use crate::theme::ActiveTheme;
use daw_core::PPQN;
use gpui::{Context, Window, div, prelude::*, px};

pub struct Playhead {
    current_tick: u64,
    pixels_per_beat: f64,
}

impl Playhead {
    pub fn new(current_tick: u64, pixels_per_beat: f64) -> Self {
        Self {
            current_tick,
            pixels_per_beat,
        }
    }

    pub fn set_tick(&mut self, tick: u64) {
        self.current_tick = tick;
    }
}

impl Render for Playhead {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let x_pos = (self.current_tick as f64 / PPQN as f64) * self.pixels_per_beat;

        div()
            .absolute()
            .left(px(x_pos as f32))
            .top(px(0.))
            .bottom(px(0.))
            .w(px(2.))
            .bg(theme.accent)
    }
}
