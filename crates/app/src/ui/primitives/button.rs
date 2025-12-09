use crate::theme::ActiveTheme;
use gpui::{Context, Div, ElementId, InteractiveElement, Stateful, Styled, div, px};

/// Creates a button with standard styling
pub fn button<V>(id: impl Into<ElementId>, cx: &mut Context<V>) -> Stateful<Div> {
    let theme = cx.theme();
    div()
        .h(px(28.))
        .px_2()
        .bg(theme.element)
        .border_1()
        .border_color(theme.border)
        .text_color(theme.text)
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
    div()
        .h(px(28.))
        .px_2()
        .bg(if active {
            theme.element_active
        } else {
            theme.element
        })
        .border_1()
        .border_color(theme.border)
        .text_color(theme.text)
        .hover(|s| s.bg(theme.element_hover))
        .id(id.into())
}
