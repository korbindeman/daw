use gpui::KeyBinding;

use crate::app_menus::OpenProject;
use crate::ui::primitives::input;
use crate::{PlayPause, Quit};

pub fn keybindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("space", PlayPause, None),
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-o", OpenProject, None),
        // Input keybindings
        KeyBinding::new("backspace", input::Backspace, Some("Input")),
        KeyBinding::new("delete", input::Delete, Some("Input")),
        KeyBinding::new("left", input::Left, Some("Input")),
        KeyBinding::new("right", input::Right, Some("Input")),
        KeyBinding::new("shift-left", input::SelectLeft, Some("Input")),
        KeyBinding::new("shift-right", input::SelectRight, Some("Input")),
        KeyBinding::new("cmd-a", input::SelectAll, Some("Input")),
        KeyBinding::new("cmd-v", input::Paste, Some("Input")),
        KeyBinding::new("cmd-c", input::Copy, Some("Input")),
        KeyBinding::new("cmd-x", input::Cut, Some("Input")),
        KeyBinding::new("home", input::Home, Some("Input")),
        KeyBinding::new("end", input::End, Some("Input")),
        KeyBinding::new("ctrl-cmd-space", input::ShowCharacterPalette, Some("Input")),
    ]
}
