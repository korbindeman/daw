use crate::theme::ActiveTheme;
use gpui::{Context, Div, ElementId, InteractiveElement, Stateful, Styled, div, px};

/// Creates a button with standard styling
pub fn button<V>(id: impl Into<ElementId>, cx: &mut Context<V>) -> Stateful<Div> {
    let theme = cx.theme();
    div()
        .h(px(28.))
        .px_2()
        .bg(theme.element_active) // Darker default background
        .border_2() // Thicker border
        .border_color(theme.border)
        .rounded(px(6.)) // Rounded corners
        .text_color(theme.text)
        .flex()
        .items_center()
        .justify_center()
        .hover(|s| s.bg(theme.element_hover))
        .id(id.into())
}

/// Creates a button with an active state
pub fn button_active<V>(
    id: impl Into<ElementId>,
    active: bool,
    cx: &mut Context<V>,
) -> Stateful<Div> {
    let theme = cx.theme();

    // Darker, richer accent color for active state
    let active_bg = gpui::hsla(0.02, 0.72, 0.45, 1.0); // Darker, richer version of accent
    let active_border = gpui::hsla(0.02, 0.80, 0.35, 1.0); // Even darker border in same hue

    div()
        .h(px(28.))
        .px_2()
        .bg(if active {
            active_bg
        } else {
            theme.element_active // Darker default background
        })
        .border_2() // Thicker border
        .border_color(if active {
            active_border // Darker border in same hue
        } else {
            theme.border
        })
        .rounded(px(6.)) // Rounded corners
        .text_color(theme.text)
        .flex()
        .items_center()
        .justify_center()
        .hover(|s| {
            s.bg(if active {
                active_bg
            } else {
                theme.element_hover
            })
        })
        .id(id.into())
}
