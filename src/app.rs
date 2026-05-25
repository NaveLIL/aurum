use tui_input::Input;
use crate::types::{Package, Update, ScanResult, NewsItem, CacheEntry};
use crate::config::Config;
use crate::action::Action;

#[derive(Debug, Clone, PartialEq, Default)]
pub enum Route {
    #[default]
    Dashboard,
    Updates,
    Installed,
    Search,
    News,
    Cache,
    Scanner,
    PackageDetails,
    DiffViewer,
    Help,
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
    pub updates: Vec<Update>,
    pub search_results: Vec<Package>,
    pub selected_package: Option<Package>,
    pub scan_results: Vec<ScanResult>,
    pub news_items: Vec<NewsItem>,
    pub cache_entries: Vec<CacheEntry>,
    pub selected_packages: std::collections::HashSet<String>,

    // UI State
    pub list_state: ratatui::widgets::ListState,
    pub table_state: ratatui::widgets::TableState,
    pub tab_index: usize,
    pub cache_list_state: ratatui::widgets::ListState,
    pub news_list_state: ratatui::widgets::ListState,

    // Status
    pub status_message: Option<String>,
    pub is_loading: bool,
    pub last_checked: Option<String>,

    // UI polish
    pub spinner_frame: usize,
    pub confirm_dialog: Option<(String, Box<Action>)>,
}

const TAB_COUNT: usize = 7;

impl App {
    pub fn new(config: Config) -> Self {
        Self {
            running: true,
            config,
            route: Route::Dashboard,
            input_mode: InputMode::Normal,
            search_input: Input::default(),
            installed_packages: Vec::new(),
            updates: Vec::new(),
            search_results: Vec::new(),
            selected_package: None,
            scan_results: Vec::new(),
            news_items: Vec::new(),
            cache_entries: Vec::new(),
            selected_packages: std::collections::HashSet::new(),
            list_state: ratatui::widgets::ListState::default(),
            table_state: ratatui::widgets::TableState::default(),
            tab_index: 0,
            cache_list_state: ratatui::widgets::ListState::default(),
            news_list_state: ratatui::widgets::ListState::default(),
            status_message: None,
            is_loading: false,
            last_checked: None,
            spinner_frame: 0,
            confirm_dialog: None,
        }
    }

    pub fn tick(&mut self) {
        self.spinner_frame = (self.spinner_frame + 1) % 10;
    }

    pub fn quit(&mut self) {
        self.running = false;
    }

    pub fn tab_route(index: usize) -> Route {
        match index {
            0 => Route::Dashboard,
            1 => Route::Updates,
            2 => Route::Installed,
            3 => Route::Search,
            4 => Route::News,
            5 => Route::Cache,
            6 => Route::Scanner,
            _ => Route::Dashboard,
        }
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
            Route::Installed => self.installed_packages.len(),
            Route::Search => self.search_results.len(),
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
