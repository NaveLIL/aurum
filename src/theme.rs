use ratatui::style::Color;

#[derive(Debug, Clone)]
pub struct ThemeColors {
    pub name: &'static str,
    pub header_fg: Color,
    pub border_fg: Color,
    pub accent_fg: Color,
    pub success_fg: Color,
    pub warning_fg: Color,
    pub error_fg: Color,
    pub text_fg: Color,
    pub highlight_bg: Color,
    pub highlight_fg: Color,
}

pub fn get_theme(name: &str) -> ThemeColors {
    match name.to_lowercase().as_str() {
        "nord" => ThemeColors {
            name: "Nord",
            header_fg: Color::Rgb(143, 188, 187),   // Nord 8 (Aurora)
            border_fg: Color::Rgb(76, 86, 106),     // Nord 3 (Polar Night)
            accent_fg: Color::Rgb(136, 192, 208),   // Nord 9 (Frost)
            success_fg: Color::Rgb(163, 190, 140),  // Nord 14 (Green)
            warning_fg: Color::Rgb(235, 203, 139),  // Nord 13 (Yellow)
            error_fg: Color::Rgb(191, 97, 106),     // Nord 11 (Red)
            text_fg: Color::Rgb(229, 233, 240),     // Nord 5 (Snow Storm)
            highlight_bg: Color::Rgb(59, 66, 82),   // Nord 2 (Selected Background)
            highlight_fg: Color::Rgb(143, 188, 187),
        },
        "gruvbox" => ThemeColors {
            name: "Gruvbox",
            header_fg: Color::Rgb(250, 189, 47),    // Gruvbox Yellow
            border_fg: Color::Rgb(102, 92, 84),     // Gruvbox Gray/Brown
            accent_fg: Color::Rgb(142, 192, 124),   // Gruvbox Aqua
            success_fg: Color::Rgb(184, 187, 38),   // Gruvbox Green
            warning_fg: Color::Rgb(254, 128, 25),   // Gruvbox Orange
            error_fg: Color::Rgb(251, 73, 52),      // Gruvbox Red
            text_fg: Color::Rgb(235, 219, 178),     // Gruvbox Light Foreground
            highlight_bg: Color::Rgb(60, 56, 54),   // Gruvbox Dark Selection
            highlight_fg: Color::Rgb(250, 189, 47),
        },
        "dracula" => ThemeColors {
            name: "Dracula",
            header_fg: Color::Rgb(189, 147, 249),   // Dracula Purple
            border_fg: Color::Rgb(98, 114, 164),    // Dracula Comment Gray
            accent_fg: Color::Rgb(255, 121, 198),   // Dracula Pink
            success_fg: Color::Rgb(80, 250, 123),   // Dracula Green
            warning_fg: Color::Rgb(255, 184, 108),  // Dracula Orange
            error_fg: Color::Rgb(255, 85, 85),      // Dracula Red
            text_fg: Color::Rgb(248, 248, 242),     // Dracula Foreground
            highlight_bg: Color::Rgb(68, 71, 90),    // Dracula Selection
            highlight_fg: Color::Rgb(80, 250, 123),
        },
        "cyberpunk" | "neon" => ThemeColors {
            name: "Cyberpunk",
            header_fg: Color::Rgb(255, 0, 127),     // Neon Pink
            border_fg: Color::Rgb(0, 240, 255),      // Neon Cyan
            accent_fg: Color::Rgb(255, 230, 0),     // Neon Gold/Yellow
            success_fg: Color::Rgb(57, 255, 20),    // Neon Green
            warning_fg: Color::Rgb(255, 110, 0),    // Neon Orange
            error_fg: Color::Rgb(255, 42, 109),     // Bright Red
            text_fg: Color::Rgb(255, 255, 255),     // White
            highlight_bg: Color::Rgb(40, 0, 80),     // Deep Purple Selected
            highlight_fg: Color::Rgb(0, 240, 255),
        },
        _ => ThemeColors {
            name: "Default",
            header_fg: Color::Rgb(255, 220, 80),    // Warm Yellow
            border_fg: Color::Rgb(60, 60, 80),      // Dark Blueish Grey
            accent_fg: Color::Rgb(80, 180, 255),    // Light Blue
            success_fg: Color::Rgb(100, 220, 100),  // Soft Green
            warning_fg: Color::Rgb(255, 200, 100),  // Soft Orange
            error_fg: Color::Rgb(255, 100, 100),    // Soft Red
            text_fg: Color::Rgb(200, 200, 220),     // Light Gray
            highlight_bg: Color::Rgb(50, 50, 70),    // Dark Selected Background
            highlight_fg: Color::Rgb(255, 220, 80),
        },
    }
}
