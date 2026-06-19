use tui_input::Input;
use crate::types::{Package, Update, ScanResult, NewsItem, CacheEntry, DiskStats, SystemInfo, CpuMemStats, FailedService};
use crate::config::Config;
use crate::action::Action;
use std::collections::HashSet;
use crate::backend::flatpak::{FlatpakApp, FlatpakSearchApp};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchSource {
    #[default]
    Aur,
    Flatpak,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InstalledSource {
    #[default]
    System,
    Flatpak,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum Route {
    #[default]
    Dashboard,
    Updates,
    Installed,
    Search,
    Store,
    News,
    Cache,
    Scanner,
    PackageDetails,
    DiffViewer,
    Settings,
    Systemd,
}


#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum SettingsField {
    CheckInterval,
    MaxCacheSize,
    AurUrl,
    AutoCleanCache,
    AutoCleanInterval,
    RiskyPattern(usize),
    AddRiskyPattern,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PkgbuildViewMode {
    Full,
    Diff,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,
    Editing,
}


pub struct App {
    pub running: bool,
    pub config: Config,
    pub route: Route,
    pub input_mode: InputMode,
    pub search_input: Input,

    // Data
    pub installed_packages: Vec<Package>,
    pub installed_packages_set: HashSet<String>,
    pub updates: Vec<Update>,
    pub search_results: Vec<Package>,
    pub selected_package: Option<Package>,
    pub scan_results: Vec<ScanResult>,
    pub news_items: Vec<NewsItem>,
    pub cache_entries: Vec<CacheEntry>,
    pub orphans: Vec<Package>,
    pub selected_packages: std::collections::HashSet<String>,
    pub pkgbuild_name: String,
    pub pkgbuild_view_mode: PkgbuildViewMode,
    pub pkgbuild_raw_lines: Vec<ratatui::text::Line<'static>>,
    pub pkgbuild_diff_lines: Vec<ratatui::text::Line<'static>>,
    pub pkgbuild_scroll: usize,


    // Flatpak
    pub flatpak_available: bool,
    pub search_source: SearchSource,
    pub installed_source: InstalledSource,
    pub flatpak_search_results: Vec<FlatpakSearchApp>,
    pub installed_flatpaks: Vec<FlatpakApp>,

    // UI State
    pub list_state: ratatui::widgets::ListState,
    pub tab_index: usize,
    pub orphans_list_state: ratatui::widgets::ListState,
    pub cache_active_pane: usize, // 0 = cache, 1 = orphans

    // Store State
    pub store_category_index: usize,
    pub store_app_index: usize,
    pub store_active_pane: usize,

    // Status
    pub status_message: Option<String>,
    pub is_loading: bool,
    pub last_checked: Option<String>,

    // UI polish
    pub spinner_frame: usize,
    pub confirm_dialog: Option<(String, Box<Action>)>,

    // Help & Disk stats
    pub show_help: bool,
    pub disk_stats: DiskStats,
    pub system_info: SystemInfo,
    pub cpu_mem_stats: CpuMemStats,
    // Settings State
    pub settings_selected_index: usize,
    pub settings_field_edit: Option<SettingsField>,
    pub settings_input: Input,

    // Systemd Inspector State
    pub failed_services: Vec<FailedService>,
    pub systemd_list_state: ratatui::widgets::ListState,
    pub systemd_selected_logs: Vec<String>,
    pub systemd_logs_loading: bool,
}


const TAB_COUNT: usize = 9;

impl App {
    pub fn new(config: Config) -> Self {
        Self {
            running: true,
            config,
            route: Route::Dashboard,
            input_mode: InputMode::Normal,
            search_input: Input::default(),
            installed_packages: Vec::new(),
            installed_packages_set: HashSet::new(),
            updates: Vec::new(),
            search_results: Vec::new(),
            selected_package: None,
            scan_results: Vec::new(),
            news_items: Vec::new(),
            cache_entries: Vec::new(),
            orphans: Vec::new(),
            selected_packages: std::collections::HashSet::new(),
            pkgbuild_name: String::new(),
            pkgbuild_view_mode: PkgbuildViewMode::Diff,
            pkgbuild_raw_lines: Vec::new(),
            pkgbuild_diff_lines: Vec::new(),
            pkgbuild_scroll: 0,

            flatpak_available: false,
            search_source: SearchSource::default(),
            installed_source: InstalledSource::default(),
            flatpak_search_results: Vec::new(),
            installed_flatpaks: Vec::new(),
            list_state: ratatui::widgets::ListState::default(),
            tab_index: 0,
            orphans_list_state: ratatui::widgets::ListState::default(),
            cache_active_pane: 0,
            store_category_index: 0,
            store_app_index: 0,
            store_active_pane: 0,
            status_message: None,
            is_loading: false,
            last_checked: None,
            spinner_frame: 0,
            confirm_dialog: None,
            show_help: false,
            disk_stats: DiskStats::default(),
            system_info: SystemInfo::default(),
            cpu_mem_stats: CpuMemStats::default(),
            settings_selected_index: 0,
            settings_field_edit: None,
            settings_input: Input::default(),
            failed_services: Vec::new(),
            systemd_list_state: ratatui::widgets::ListState::default(),
            systemd_selected_logs: Vec::new(),
            systemd_logs_loading: false,
        }
    }

    pub fn tick(&mut self) {
        self.spinner_frame = (self.spinner_frame + 1) % 10;
    }

    pub fn tab_route(index: usize) -> Route {
        match index {
            0 => Route::Dashboard,
            1 => Route::Updates,
            2 => Route::Installed,
            3 => Route::Search,
            4 => Route::Store,
            5 => Route::News,
            6 => Route::Cache,
            7 => Route::Scanner,
            8 => Route::Settings,
            _ => Route::Dashboard,
        }
    }

    pub fn theme(&self) -> crate::theme::ThemeColors {
        crate::theme::get_theme(&self.config.theme)
    }

    pub fn next_tab(&mut self) {
        self.tab_index = (self.tab_index + 1) % TAB_COUNT;
        self.route = Self::tab_route(self.tab_index);
        self.list_state.select(None);
    }

    pub fn previous_tab(&mut self) {
        if self.tab_index > 0 {
            self.tab_index -= 1;
        } else {
            self.tab_index = TAB_COUNT - 1;
        }
        self.route = Self::tab_route(self.tab_index);
        self.list_state.select(None);
    }

    /// Get the currently active list length for navigation
    pub fn current_list_len(&self) -> usize {
        match self.route {
            Route::Updates => self.updates.len(),
            Route::Installed => {
                match self.installed_source {
                    InstalledSource::System => self.installed_packages.len(),
                    InstalledSource::Flatpak => self.installed_flatpaks.len(),
                }
            }
            Route::Search => {
                match self.search_source {
                    SearchSource::Aur => self.search_results.len(),
                    SearchSource::Flatpak => self.flatpak_search_results.len(),
                }
            }
            Route::News => self.news_items.len(),
            Route::Cache => self.cache_entries.len(),
            Route::Scanner => self.scan_results.last().map_or(0, |r| r.vulnerabilities.len()),
            _ => 0,
        }
    }

    pub fn select_next(&mut self) {
        let len = self.current_list_len();
        if len == 0 { return; }
        let i = match self.list_state.selected() {
            Some(i) => if i >= len - 1 { 0 } else { i + 1 },
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn select_previous(&mut self) {
        let len = self.current_list_len();
        if len == 0 { return; }
        let i = match self.list_state.selected() {
            Some(i) => if i == 0 { len - 1 } else { i - 1 },
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn spinner_char(&self) -> char {
        const FRAMES: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        FRAMES[self.spinner_frame % FRAMES.len()]
    }
}
