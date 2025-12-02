use daw_transport::{Clip, PPQN, Track as TransportTrack, WaveformData};
use gpui::{
    Bounds, Context, ElementId, Point, Size, Window, black, canvas, div, fill, prelude::*, px,
    rgb, rgba,
};
use std::sync::Arc;

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
    let waveform = clip.waveform.clone();

    div()
        .absolute()
        .left(px(start_px as f32))
        .top(px(4.))
        .w(px(width_px as f32))
        .h(px(72.))
        .bg(rgb(0x8B9DC3))
        .border_1()
        .border_color(black())
        .overflow_hidden()
        .child(render_waveform(waveform))
}

fn render_waveform(waveform: Arc<WaveformData>) -> impl IntoElement {
    canvas(
        move |bounds, _window, _cx| (bounds, waveform.clone()),
        move |_bounds, (bounds_data, waveform), window, _cx| {
            let height: f32 = bounds_data.size.height.into();
            let width: f32 = bounds_data.size.width.into();
            let origin_x: f32 = bounds_data.origin.x.into();
            let origin_y: f32 = bounds_data.origin.y.into();
            let center_y = height / 2.0;

            let num_buckets = waveform.peaks.len();
            if num_buckets == 0 {
                return;
            }

            let pixels_per_bucket = width / num_buckets as f32;

            for (i, (min_val, max_val)) in waveform.peaks.iter().enumerate() {
                let x = origin_x + i as f32 * pixels_per_bucket;
                let bar_width = pixels_per_bucket.max(1.0);

                let top = center_y - (*max_val * center_y);
                let bottom = center_y - (*min_val * center_y);
                let bar_height = (bottom - top).max(1.0);

                let bar_bounds = Bounds {
                    origin: Point {
                        x: px(x),
                        y: px(origin_y + top),
                    },
                    size: Size {
                        width: px(bar_width),
                        height: px(bar_height),
                    },
                };

                window.paint_quad(fill(bar_bounds, rgba(0x3D5A80FF)));
            }
        },
    )
    .size_full()
}
