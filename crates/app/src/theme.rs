use gpui::{App, Global, Hsla, hsla};

/// Simple theme with semantic colors for the DAW
#[derive(Clone)]
pub struct Theme {
    // Backgrounds
    pub background: Hsla,
    pub surface: Hsla,
    pub elevated: Hsla,
    pub header: Hsla,

    // Borders
    pub border: Hsla,
    pub border_focused: Hsla,

    // Text
    pub text: Hsla,
    pub text_muted: Hsla,

    // Interactive elements
    pub element: Hsla,
    pub element_hover: Hsla,
    pub element_active: Hsla,

    // Accent / playhead
    pub accent: Hsla,

    // Track colors (for distinguishing tracks)
    pub track_colors: Vec<Hsla>,
}

impl Theme {
    /// Dark theme (default)
    pub fn dark() -> Self {
        Self {
            // Backgrounds
            background: hsla(0.12, 0.08, 0.18, 1.0), // warm dark gray
            surface: hsla(0.0, 0.0, 0.28, 1.0),      // lighter for sidebar/ruler
            elevated: hsla(0.0, 0.0, 0.71, 1.0),     // #B6B6B6 - timeline background
            header: hsla(0.0, 0.0, 0.58, 1.0),       // darker gray for header

            // Borders
            border: hsla(0.12, 0.05, 0.30, 1.0), // warm gray border
            border_focused: hsla(0.58, 0.6, 0.5, 1.0), // blue-ish

            // Text
            text: hsla(0.0, 0.0, 0.95, 1.0), // bright white for buttons
            text_muted: hsla(0.12, 0.04, 0.55, 1.0), // warm gray

            // Interactive elements
            element: hsla(0.58, 0.02, 0.45, 1.0), // slightly darker steel for buttons
            element_hover: hsla(0.58, 0.02, 0.50, 1.0),
            element_active: hsla(0.58, 0.02, 0.40, 1.0),

            // Accent - red for playhead
            accent: hsla(0.0, 0.75, 0.50, 1.0),

            // Waveform color is derived in track.rs
            track_colors: vec![
                hsla(0.85, 0.31, 0.67, 1.0), // Pink #C592C0
                hsla(0.62, 0.28, 0.65, 1.0), // Blue #8E9CBF
                hsla(0.08, 0.60, 0.72, 1.0), // Peach
                hsla(0.14, 0.60, 0.75, 1.0), // Yellow
                hsla(0.35, 0.50, 0.65, 1.0), // Green
                hsla(0.50, 0.50, 0.68, 1.0), // Cyan
                hsla(0.75, 0.50, 0.70, 1.0), // Purple
                hsla(0.85, 0.50, 0.70, 1.0), // Magenta
            ],
        }
    }
}

impl Global for Theme {}

/// Extension trait to access the theme from App context
pub trait ActiveTheme {
    fn theme(&self) -> &Theme;
}

impl ActiveTheme for App {
    fn theme(&self) -> &Theme {
        self.global::<Theme>()
    }
}

/// Initialize the theme as a global
pub fn init(cx: &mut App) {
    cx.set_global(Theme::dark());
}

/// Convert a track color to its dark variant for high contrast
pub fn to_dark_variant(color: Hsla) -> Hsla {
    Hsla {
        h: color.h,
        s: color.s * 0.6,
        l: 0.22,
        a: color.a,
    }
}
