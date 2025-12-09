use crate::theme::ActiveTheme;
use gpui::{Context, Window, div, prelude::*, px};
use std::collections::BTreeMap;

const SIDEBAR_WIDTH: f32 = 200.0;

pub struct Sidebar {
    directories: BTreeMap<String, Vec<String>>,
}

impl Sidebar {
    pub fn new() -> Self {
        let mut directories = BTreeMap::new();

        if let Ok(entries) = std::fs::read_dir("samples") {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_dir() {
                        if let Some(dir_name) = entry.file_name().to_str() {
                            let mut samples = Vec::new();
                            let dir_path = format!("samples/{}", dir_name);

                            if let Ok(sample_entries) = std::fs::read_dir(&dir_path) {
                                for sample_entry in sample_entries.flatten() {
                                    if let Some(name) = sample_entry.file_name().to_str() {
                                        if name.ends_with(".wav") {
                                            samples.push(name.to_string());
                                        }
                                    }
                                }
                            }

                            samples.sort();
                            directories.insert(dir_name.to_string(), samples);
                        }
                    }
                }
            }
        }

        Self { directories }
    }
}

impl Render for Sidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .id("sidebar")
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .overflow_y_scroll()
            .bg(theme.surface)
            .border_r_1()
            .border_color(theme.border)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_2()
                    .children(self.directories.iter().map(|(dir_name, samples)| {
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_color(theme.text)
                                    .text_size(px(11.))
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .child(dir_name.clone()),
                            )
                            .children(samples.iter().map(|sample| {
                                div()
                                    .text_color(theme.text)
                                    .text_size(px(10.))
                                    .pl_2()
                                    .child(sample.clone())
                            }))
                    })),
            )
    }
}
