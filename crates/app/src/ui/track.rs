use daw_transport::{Clip, PPQN, Track as TransportTrack};
use gpui::{Context, ElementId, Window, black, div, prelude::*, px, rgb};

pub struct Track {
    track: TransportTrack,
    pixels_per_beat: f64,
    tempo: f64,
    timeline_width: f64,
}

impl Track {
    pub fn new(
        track: TransportTrack,
        pixels_per_beat: f64,
        tempo: f64,
        timeline_width: f64,
    ) -> Self {
        Self {
            track,
            pixels_per_beat,
            tempo,
            timeline_width,
        }
    }
}

impl Render for Track {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let clips = self.track.clips.iter().map(|clip| {
            let start_px = (clip.start as f64 / PPQN as f64) * self.pixels_per_beat;
            let duration_ticks = clip.duration_ticks(self.tempo);
            let width_px = (duration_ticks as f64 / PPQN as f64) * self.pixels_per_beat;

            render_clip(clip, start_px, width_px)
        });

        div()
            .id(ElementId::Name(
                format!("track-{}-scroll", self.track.id.0).into(),
            ))
            .w_full()
            .h(px(80.))
            .border_b_1()
            .border_color(black())
            .overflow_x_scroll()
            .child(
                div()
                    .w(px(self.timeline_width as f32))
                    .h_full()
                    .relative()
                    .children(clips),
            )
    }
}

fn render_clip(clip: &Clip, start_px: f64, width_px: f64) -> impl IntoElement {
    div()
        .absolute()
        .left(px(start_px as f32))
        .top(px(4.))
        .w(px(width_px as f32))
        .h(px(72.))
        .bg(rgb(0x8B9DC3))
        .border_1()
        .border_color(black())
        .flex()
        .items_center()
        .justify_center()
        .child(div().text_xs().child(format!("Clip {}", clip.id.0)))
}
