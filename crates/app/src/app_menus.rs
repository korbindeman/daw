use gpui::{Menu, MenuItem, actions};

actions!(
    daw,
    [OpenProject, SaveProject, SaveProjectAs, RenderProject]
);

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
                MenuItem::separator(),
                MenuItem::action("Save", SaveProject),
                MenuItem::action("Save As...", SaveProjectAs),
                MenuItem::separator(),
                MenuItem::action("Render...", RenderProject),
            ],
        },
    ]
}
