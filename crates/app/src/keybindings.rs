use gpui::KeyBinding;

use crate::{PlayPause, Quit};

pub fn keybindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("space", PlayPause, None),
        KeyBinding::new("cmd-q", Quit, None),
    ]
}
