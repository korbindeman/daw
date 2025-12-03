use gpui::{Menu, MenuItem, actions};

actions!(daw, [OpenProject, RenderProject]);

pub fn app_menus() -> Vec<Menu> {
    vec![
        Menu {
            name: "Cedar".into(),
            items: vec![MenuItem::action("Quit", crate::Quit)],
        },
        Menu {
            name: "File".into(),
            items: vec![
                MenuItem::action("Open Project...", OpenProject),
                MenuItem::action("Render...", RenderProject),
            ],
        },
    ]
}
