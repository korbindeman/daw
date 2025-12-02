use daw_transport::{Clip, Track as TransportTrack};
use gpui::{Context, Window, black, div, prelude::*, px, rgb};

pub struct Track {
    track: TransportTrack,
}

impl Track {
    pub fn new(track: TransportTrack) -> Self {
        Self { track }
    }
}

impl Render for Track {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w_full()
            .h(px(80.))
            .border_b_1()
            .border_color(black())
            .flex()
            .flex_col()
            .p_2()
            .gap_2()
            .child(div().text_sm().child(format!("Track {}", self.track.id.0)))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .children(self.track.clips.iter().map(|clip| render_clip(clip))),
            )
    }
}

fn render_clip(clip: &Clip) -> impl IntoElement {
    div()
        .px_2()
        .py_1()
        .bg(rgb(0x8B9DC3))
        .border_1()
        .border_color(black())
        .text_xs()
        .child(format!("Clip {} @ {}", clip.id.0, clip.start))
}
