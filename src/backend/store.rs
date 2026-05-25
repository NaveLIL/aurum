#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreApp {
    pub name: &'static str,
    pub description: &'static str,
    pub category: &'static str,
}

pub const CATEGORIES: &[&str] = &[
    "🌐 Internet",
    "🛠 Development",
    "🎬 Multimedia",
    "🎮 Games",
    "⚙ System",
];

pub const STORE_APPS: &[StoreApp] = &[
    // Internet
    StoreApp {
        name: "google-chrome",
        description: "Popular web browser from Google",
        category: "🌐 Internet",
    },
    StoreApp {
        name: "brave-bin",
        description: "Privacy-focused web browser blocking ads and trackers by default",
        category: "🌐 Internet",
    },
    StoreApp {
        name: "telegram-desktop",
        description: "Official Telegram Desktop client",
        category: "🌐 Internet",
    },
    StoreApp {
        name: "discord",
        description: "All-in-one voice and text chat for gamers",
        category: "🌐 Internet",
    },
    StoreApp {
        name: "zoom",
        description: "Video conferencing and web conferencing service",
        category: "🌐 Internet",
    },

    // Development
    StoreApp {
        name: "visual-studio-code-bin",
        description: "Code editing. Redefined. (Binary release)",
        category: "🛠 Development",
    },
    StoreApp {
        name: "sublime-text-4",
        description: "Sophisticated text editor for code, markup and prose",
        category: "🛠 Development",
    },
    StoreApp {
        name: "neovim",
        description: "Vim-fork focused on extensibility and usability",
        category: "🛠 Development",
    },
    StoreApp {
        name: "jetbrains-toolbox",
        description: "JetBrains Tools Manager",
        category: "🛠 Development",
    },
    StoreApp {
        name: "postman-bin",
        description: "API platform for building and using APIs",
        category: "🛠 Development",
    },

    // Multimedia
    StoreApp {
        name: "spotify",
        description: "Proprietary music streaming service client",
        category: "🎬 Multimedia",
    },
    StoreApp {
        name: "vlc",
        description: "Multi-platform technologies and media player",
        category: "🎬 Multimedia",
    },
    StoreApp {
        name: "gimp",
        description: "GNU Image Manipulation Program",
        category: "🎬 Multimedia",
    },
    StoreApp {
        name: "obs-studio",
        description: "Free and open source software for video recording and live streaming",
        category: "🎬 Multimedia",
    },
    StoreApp {
        name: "blender",
        description: "Fully integrated 3D creation suite",
        category: "🎬 Multimedia",
    },

    // Games
    StoreApp {
        name: "steam-installer",
        description: "Steam digital distribution platform client installer",
        category: "🎮 Games",
    },
    StoreApp {
        name: "lutris",
        description: "Open source gaming platform for Linux",
        category: "🎮 Games",
    },
    StoreApp {
        name: "heroic-games-launcher-bin",
        description: "An open source Epic Games Store, GOG, and Prime Gaming launcher",
        category: "🎮 Games",
    },
    StoreApp {
        name: "protonup-qt",
        description: "Install and manage Proton-GE, Luxtorpeda & more",
        category: "🎮 Games",
    },
    StoreApp {
        name: "bottles",
        description: "Run Windows software and games on Linux in bottles",
        category: "🎮 Games",
    },

    // System
    StoreApp {
        name: "btop",
        description: "Modern and colorful command line resource monitor",
        category: "⚙ System",
    },
    StoreApp {
        name: "kitty",
        description: "A modern, hackable, featureful, OpenGL based terminal emulator",
        category: "⚙ System",
    },
    StoreApp {
        name: "alacritty",
        description: "A cross-platform, GPU-accelerated terminal emulator",
        category: "⚙ System",
    },
    StoreApp {
        name: "timeshift",
        description: "System restore utility for Linux",
        category: "⚙ System",
    },
    StoreApp {
        name: "fastfetch",
        description: "Like neofetch, but much faster and written in C",
        category: "⚙ System",
    },
];

pub fn get_categories() -> Vec<&'static str> {
    CATEGORIES.to_vec()
}

pub fn get_apps_by_category(category: &str) -> Vec<StoreApp> {
    STORE_APPS
        .iter()
        .filter(|app| app.category == category)
        .cloned()
        .collect()
}
