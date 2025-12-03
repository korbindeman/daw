use daw_core::Track;
use gpui::{Context, IntoElement, Render, Window, div, prelude::*, px};

use crate::theme::{ActiveTheme, to_dark_variant};

pub struct TrackLabels {
    tracks: Vec<Track>,
}

impl TrackLabels {
    pub fn new(tracks: Vec<Track>) -> Self {
        Self { tracks }
    }
}

impl Render for TrackLabels {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();

        div()
            .absolute()
            .right(px(0.))
            .top(px(0.))
            .w(px(150.))
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(20.))
                    .bg(theme.elevated)
                    .border_b_1()
                    .border_l_1()
                    .border_color(theme.border),
            )
            .children(self.tracks.iter().enumerate().map(|(i, track)| {
                let track_color = theme.track_colors[i % theme.track_colors.len()];
                let text_color = to_dark_variant(track_color);
                div()
                    .h(px(80.))
                    .bg(track_color)
                    .border_b_1()
                    .border_color(theme.border)
                    .border_l_1()
                    .px_1()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(text_color)
                            .child(track.name.clone()),
                    )
                    .child(
                        div().text_xs().text_color(text_color.opacity(0.7)).child(
                            track
                                .clips
                                .first()
                                .map(|_| format!("{} clip(s)", track.clips.len()))
                                .unwrap_or_else(|| "No clips".to_string()),
                        ),
                    )
            }))
    }
}
