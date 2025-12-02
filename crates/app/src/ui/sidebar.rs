use crate::theme::ActiveTheme;
use gpui::{div, prelude::*, px, Context, Window};

pub struct Sidebar {
    samples: Vec<String>,
}

impl Sidebar {
    pub fn new() -> Self {
        let mut samples = Vec::new();
        if let Ok(entries) = std::fs::read_dir("samples/cr78") {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.ends_with(".wav") {
                        samples.push(name.to_string());
                    }
                }
            }
        }
        samples.sort();
        Self { samples }
    }
}

impl Render for Sidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .h_full()
            .w(px(200.))
            .bg(theme.surface)
            .border_r_1()
            .border_color(theme.border)
            .flex()
            .flex_col()
            .gap_2()
            .p_2()
            .children(self.samples.iter().map(|sample| {
                div().text_color(theme.text).child(sample.clone())
            }))
    }
}
