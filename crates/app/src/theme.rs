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
            // Backgrounds - Gruvbox-inspired warm yellows/browns
            background: hsla(0.10, 0.15, 0.16, 1.0), // warm dark brown (gruvbox bg)
            surface: hsla(0.10, 0.12, 0.22, 1.0),    // lighter warm brown for sidebar/ruler
            elevated: hsla(0.12, 0.08, 0.66, 1.0),   // warm beige/tan - timeline background
            header: hsla(0.10, 0.10, 0.30, 1.0),     // warm medium brown for header

            // Borders
            border: hsla(0.10, 0.08, 0.28, 0.5), // subtle warm brown border with transparency
            border_focused: hsla(0.12, 0.45, 0.58, 1.0), // gruvbox yellow accent

            // Text
            text: hsla(0.12, 0.12, 0.92, 1.0), // warm off-white (gruvbox fg)
            text_muted: hsla(0.10, 0.08, 0.52, 1.0), // warm muted tan

            // Interactive elements
            element: hsla(0.10, 0.12, 0.40, 1.0), // warm brown for buttons
            element_hover: hsla(0.10, 0.12, 0.46, 1.0),
            element_active: hsla(0.10, 0.12, 0.34, 1.0),

            // Accent - warm orange/red for playhead (gruvbox red)
            accent: hsla(0.02, 0.72, 0.55, 1.0),

            // Waveform color is derived in track.rs - Gruvbox palette
            track_colors: vec![
                hsla(0.92, 0.38, 0.68, 1.0), // Gruvbox purple
                hsla(0.58, 0.42, 0.65, 1.0), // Gruvbox blue
                hsla(0.08, 0.68, 0.70, 1.0), // Gruvbox orange
                hsla(0.12, 0.65, 0.72, 1.0), // Gruvbox yellow
                hsla(0.35, 0.48, 0.62, 1.0), // Gruvbox green
                hsla(0.48, 0.52, 0.66, 1.0), // Gruvbox aqua
                hsla(0.02, 0.65, 0.64, 1.0), // Gruvbox red
                hsla(0.85, 0.40, 0.70, 1.0), // Gruvbox magenta
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
