use daw_core::Track;
use gpui::{Context, EventEmitter, IntoElement, Render, Window, div, prelude::*, px};

use crate::theme::{ActiveTheme, to_dark_variant};

const TRACK_LABEL_WIDTH: f32 = 150.0;
const TRACK_HEIGHT: f32 = 80.0;
const RULER_HEIGHT: f32 = 20.0;

pub struct TrackLabels {
    tracks: Vec<Track>,
}

pub enum TrackLabelsEvent {
    ToggleEnabled(u64),
}

impl EventEmitter<TrackLabelsEvent> for TrackLabels {}

impl TrackLabels {
    pub fn new(tracks: Vec<Track>) -> Self {
        Self { tracks }
    }

    pub fn set_tracks(&mut self, tracks: Vec<Track>) {
        self.tracks = tracks;
    }
}

impl Render for TrackLabels {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();

        div()
            .absolute()
            .right(px(0.))
            .top(px(0.))
            .w(px(TRACK_LABEL_WIDTH))
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(RULER_HEIGHT))
                    .bg(theme.elevated)
                    .border_b_1()
                    .border_l_1()
                    .border_color(theme.border),
            )
            .children(self.tracks.iter().enumerate().map(|(i, track)| {
                let track_color = theme.track_colors[i % theme.track_colors.len()];
                let text_color = to_dark_variant(track_color);
                let track_id = track.id.0;
                let enabled = track.enabled;

                // Dim the background color when disabled
                let bg_color = if enabled {
                    track_color
                } else {
                    track_color.opacity(0.3)
                };

                div()
                    .h(px(TRACK_HEIGHT))
                    .bg(bg_color)
                    .border_b_2()
                    .border_color(theme.border)
                    .border_l_1()
                    .px_1()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                // Enable/disable toggle button
                                div()
                                    .id(("track-toggle", i))
                                    .w(px(16.))
                                    .h(px(16.))
                                    .rounded(px(2.))
                                    .border_1()
                                    .border_color(text_color.opacity(0.5))
                                    .bg(if enabled {
                                        text_color.opacity(0.8)
                                    } else {
                                        gpui::transparent_black()
                                    })
                                    .on_mouse_down(
                                        gpui::MouseButton::Left,
                                        cx.listener(move |_this, _event, _window, cx| {
                                            cx.emit(TrackLabelsEvent::ToggleEnabled(track_id));
                                        }),
                                    ),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(if enabled {
                                        text_color
                                    } else {
                                        text_color.opacity(0.5)
                                    })
                                    .child(track.name.clone()),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(text_color.opacity(if enabled { 0.7 } else { 0.3 }))
                            .child(
                                track
                                    .segments()
                                    .first()
                                    .map(|_| format!("{} segment(s)", track.segments().len()))
                                    .unwrap_or_else(|| "No segments".to_string()),
                            ),
                    )
            }))
    }
}
