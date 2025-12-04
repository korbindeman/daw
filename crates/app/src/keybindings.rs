use gpui::KeyBinding;

use crate::app_menus::OpenProject;
use crate::{PlayPause, Quit};

pub fn keybindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("space", PlayPause, None),
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-o", OpenProject, None),
    ]
}
