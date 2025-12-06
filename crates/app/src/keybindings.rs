use gpui::KeyBinding;

use crate::app_menus::{OpenProject, RenderProject, SaveProject, SaveProjectAs};
use crate::{PlayPause, Quit};

pub fn keybindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("space", PlayPause, None),
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-o", OpenProject, None),
        KeyBinding::new("cmd-s", SaveProject, None),
        KeyBinding::new("cmd-shift-s", SaveProjectAs, None),
        KeyBinding::new("cmd-r", RenderProject, None),
    ]
}
