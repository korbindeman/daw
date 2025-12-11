use crate::theme::{ActiveTheme, to_dark_variant};
use daw_core::{PPQN, Track as TransportTrack, WaveformData};
use gpui::{
    Bounds, Context, EventEmitter, Hsla, Point, Size, Window, canvas, div, fill, prelude::*, px,
};
use std::sync::Arc;

const TRACK_HEIGHT: f32 = 80.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipId(pub String);

#[derive(Debug)]
pub enum TrackEvent {
    ClipClicked(ClipId),
    EmptySpaceClicked(f64), // pixel position clicked
    EmptySpaceRightClicked,
}

impl EventEmitter<TrackEvent> for Track {}

pub struct Track {
    track: TransportTrack,
    pixels_per_beat: f64,
    timeline_width: f64,
    selected_clips: Vec<ClipId>,
}

impl Track {
    pub fn new(
        track: TransportTrack,
        pixels_per_beat: f64,
        _tempo: f64,
        timeline_width: f64,
    ) -> Self {
        Self {
            track,
            pixels_per_beat,
            timeline_width,
            selected_clips: Vec::new(),
        }
    }

    pub fn set_selected_clips(&mut self, selected_clips: Vec<ClipId>) {
        self.selected_clips = selected_clips;
    }
}

impl Render for Track {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let track_index = self.track.id.0 as usize;
        let track_color = theme.track_colors[track_index % theme.track_colors.len()];

        let selected_clips = self.selected_clips.clone();

        // Create clips with bounds tracking for click detection
        let clip_bounds: Vec<(f64, f64)> = self
            .track
            .clips()
            .iter()
            .map(|clip| {
                let start_px = (clip.start_tick as f64 / PPQN as f64) * self.pixels_per_beat;
                let duration_ticks = clip.duration_ticks();
                let width_px = (duration_ticks as f64 / PPQN as f64) * self.pixels_per_beat;
                (start_px, start_px + width_px)
            })
            .collect();

        // Clone for the right-click handler
        let clip_bounds_right = clip_bounds.clone();

        let clips: Vec<_> = self
            .track
            .clips()
            .iter()
            .map(|clip| {
                let start_px = (clip.start_tick as f64 / PPQN as f64) * self.pixels_per_beat;
                let duration_ticks = clip.duration_ticks();
                let width_px = (duration_ticks as f64 / PPQN as f64) * self.pixels_per_beat;
                let clip_id = ClipId(clip.name.clone());
                let is_selected = selected_clips.contains(&clip_id);

                // Create the clip element
                let waveform = clip.waveform.clone();
                let clip_name = clip.name.clone();

                // When selected, flip the colors
                let (final_bg_color, final_waveform_color) = if is_selected {
                    (to_dark_variant(track_color), track_color)
                } else {
                    (track_color, to_dark_variant(track_color))
                };

                div()
                    .absolute()
                    .left(px(start_px as f32))
                    .top(px(4.))
                    .w(px(width_px as f32))
                    .h(px(72.))
                    .bg(final_bg_color)
                    .border_1()
                    .border_color(darken(final_bg_color, 0.2))
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .child(
                        // Clickable label bar at the top
                        div()
                            .w_full()
                            .h(px(16.))
                            .bg(darken(final_bg_color, 0.1))
                            .px_1()
                            .flex()
                            .cursor_grab()
                            .items_center()
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(
                                    move |_track, _event: &gpui::MouseDownEvent, _window, cx| {
                                        cx.emit(TrackEvent::ClipClicked(clip_id.clone()));
                                    },
                                ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(final_waveform_color)
                                    .child(clip_name),
                            ),
                    )
                    .child(
                        // Waveform area - clicks pass through to move the cursor
                        div()
                            .flex_1()
                            .w_full()
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(
                                    move |_track, event: &gpui::MouseDownEvent, _window, cx| {
                                        let x_pos: f32 = event.position.x.into();
                                        cx.emit(TrackEvent::EmptySpaceClicked(x_pos as f64));
                                    },
                                ),
                            )
                            .child(render_waveform(waveform, final_waveform_color)),
                    )
            })
            .collect();

        div()
            .w(px(self.timeline_width as f32))
            .h(px(TRACK_HEIGHT))
            .border_b_2()
            .border_color(theme.border)
            .child(
                div()
                    .w_full()
                    .h_full()
                    .relative()
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(move |_track, event: &gpui::MouseDownEvent, _window, cx| {
                            let x_pos: f32 = event.position.x.into();
                            let x_pos_f64 = x_pos as f64;

                            // Check if click is within any clip bounds
                            let clicked_on_clip = clip_bounds
                                .iter()
                                .any(|(start, end)| x_pos_f64 >= *start && x_pos_f64 <= *end);

                            // Only emit empty space click if we didn't click on a clip
                            if !clicked_on_clip {
                                cx.emit(TrackEvent::EmptySpaceClicked(x_pos_f64));
                            }
                        }),
                    )
                    .on_mouse_down(
                        gpui::MouseButton::Right,
                        cx.listener(move |_track, event: &gpui::MouseDownEvent, _window, cx| {
                            let x_pos: f32 = event.position.x.into();
                            let x_pos_f64 = x_pos as f64;

                            // Check if click is within any clip bounds
                            let clicked_on_clip = clip_bounds_right
                                .iter()
                                .any(|(start, end)| x_pos_f64 >= *start && x_pos_f64 <= *end);

                            // Only emit empty space right click if we didn't click on a clip
                            if !clicked_on_clip {
                                cx.emit(TrackEvent::EmptySpaceRightClicked);
                            }
                        }),
                    )
                    .children(clips),
            )
    }
}

fn render_waveform(waveform: Arc<WaveformData>, color: Hsla) -> impl IntoElement {
    use std::cell::Cell;

    // Cache previous render state to avoid unnecessary repaints
    let last_bounds = Cell::new(None::<Bounds<gpui::Pixels>>);
    let last_color = Cell::new(None::<Hsla>);

    canvas(
        move |bounds, _window, _cx| (bounds, waveform.clone(), color),
        move |_bounds, (bounds_data, waveform, current_color), window, _cx| {
            // Check if anything changed since last paint
            let should_repaint = {
                let prev_bounds = last_bounds.get();
                let prev_color = last_color.get();

                prev_bounds != Some(bounds_data) || prev_color != Some(current_color)
            };

            if !should_repaint {
                return;
            }

            // Update cache
            last_bounds.set(Some(bounds_data));
            last_color.set(Some(current_color));

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

                window.paint_quad(fill(bar_bounds, current_color));
            }
        },
    )
    .size_full()
}

fn darken(color: Hsla, amount: f32) -> Hsla {
    Hsla {
        l: (color.l - amount).max(0.0),
        ..color
    }
}
