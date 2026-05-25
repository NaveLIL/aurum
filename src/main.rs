mod app;
mod config;
mod types;
mod ui;
mod backend;
mod scanner;
mod action;

use app::App;
use action::Action;
use config::Config;
use std::io;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Load config
    let config = Config::load().unwrap_or_default();
    let mut app = App::new(config);

    // Channel for actions
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    // Initial data fetch
    let tx_load = tx.clone();
    tokio::spawn(async move {
        tx_load.send(Action::SetStatus("Loading installed packages...".to_string())).ok();
        match backend::paru::Paru::get_installed().await {
            Ok(pkgs) => tx_load.send(Action::SetInstalled(pkgs)).ok(),
            Err(e) => tx_load.send(Action::Error(format!("Failed to load installed: {}", e))).ok(),
        };

        tx_load.send(Action::SetStatus("Checking for updates...".to_string())).ok();
        match backend::paru::Paru::get_updates().await {
            Ok(updates) => tx_load.send(Action::SetUpdates(updates)).ok(),
            Err(e) => tx_load.send(Action::Error(format!("Failed to check updates: {}", e))).ok(),
        };

        // Load news
        tx_load.send(Action::SetStatus("Loading Arch news...".to_string())).ok();
        match backend::news::fetch_arch_news().await {
            Ok(news) => tx_load.send(Action::SetNews(news)).ok(),
            Err(e) => tx_load.send(Action::Error(format!("Failed to load news: {}", e))).ok(),
        };

        // Load cache
        tx_load.send(Action::SetStatus("Scanning cache...".to_string())).ok();
        match backend::paru::Paru::get_cache_entries_with_size().await {
            Ok(entries) => tx_load.send(Action::SetCacheEntries(entries)).ok(),
            Err(e) => tx_load.send(Action::Error(format!("Failed to scan cache: {}", e))).ok(),
        };

        tx_load.send(Action::SetStatus("Ready".to_string())).ok();
    });

    // Task to handle input events
    let tick_rate = Duration::from_millis(250);
    let tx_input = tx.clone();
    tokio::spawn(async move {
        let mut last_tick = Instant::now();
        loop {
            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            if crossterm::event::poll(timeout).unwrap_or(false) {
                if let Event::Key(key) = event::read().unwrap() {
                    tx_input.send(Action::Key(key)).ok();
                }
            }
            if last_tick.elapsed() >= tick_rate {
                tx_input.send(Action::Tick).ok();
                last_tick = Instant::now();
            }
        }
    });

    // Main loop
    loop {
        terminal.draw(|f| ui::draw(f, &mut app))?;

        if let Some(action) = rx.recv().await {
            match action {
                Action::Quit => {
                    app.running = false;
                }
                Action::Tick => app.tick(),
                Action::SetInstalled(pkgs) => {
                    app.installed_packages = pkgs;
                }
                Action::SetUpdates(updates) => {
                    app.updates = updates;
                    app.last_checked = Some(chrono::Local::now().format("%H:%M:%S").to_string());
                }
                Action::SetStatus(msg) => {
                    app.status_message = Some(msg);
                }
                Action::Error(msg) => {
                    app.is_loading = false;
                    app.status_message = Some(format!("❌ {}", msg));
                }
                Action::SetSearchResults(pkgs) => {
                    app.is_loading = false;
                    app.search_results = pkgs;
                }
                Action::SetNews(news) => {
                    app.news_items = news;
                }
                Action::SetCacheEntries(entries) => {
                    app.cache_entries = entries;
                }
                Action::SetPackageInfo(pkg) => {
                    app.selected_package = Some(pkg);
                    app.is_loading = false;
                    app.route = app::Route::PackageDetails;
                }
                Action::ScanPackage(pkg_name) => {
                    let tx_scan = tx.clone();
                    app.status_message = Some(format!("Scanning {}...", pkg_name));
                    app.is_loading = true;
                    let config = app.config.clone();

                    tokio::spawn(async move {
                        match backend::paru::Paru::get_pkgbuild(&pkg_name).await {
                            Ok(content) => {
                                let scanner = scanner::Scanner::new(&config);
                                let result = scanner.scan(&pkg_name, &content);
                                tx_scan.send(Action::SetScanResult(result)).ok()
                            }
                            Err(e) => tx_scan.send(Action::Error(format!("Scan failed: {}", e))).ok(),
                        }
                    });
                }
                Action::SetScanResult(result) => {
                    app.is_loading = false;
                    app.scan_results.push(result);
                    app.route = app::Route::Scanner;
                    app.tab_index = 6;
                    app.status_message = Some("✅ Scan complete.".to_string());
                }
                // Package installation — suspend TUI and run paru
                Action::InstallPackages(pkg_names) => {
                    if pkg_names.is_empty() {
                        continue;
                    }
                    // Restore terminal before running paru
                    disable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        LeaveAlternateScreen,
                        DisableMouseCapture
                    )?;
                    terminal.show_cursor()?;

                    let pkgs_str = pkg_names.join(" ");
                    println!("\n>>> paru -S {}\n", pkgs_str);
                    let status = std::process::Command::new("paru")
                        .arg("-S")
                        .args(&pkg_names)
                        .status();

                    match status {
                        Ok(s) if s.success() => {
                            println!("\n✅ Installation complete. Press Enter to return...");
                        }
                        Ok(s) => {
                            println!("\n⚠ paru exited with code: {}. Press Enter to return...", s.code().unwrap_or(-1));
                        }
                        Err(e) => {
                            println!("\n❌ Failed to run paru: {}. Press Enter to return...", e);
                        }
                    }

                    // Wait for Enter
                    let _ = std::io::stdin().read_line(&mut String::new());

                    // Re-enter TUI
                    enable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        EnterAlternateScreen,
                        EnableMouseCapture
                    )?;

                    app.selected_packages.clear();

                    // Refresh data
                    let tx_refresh = tx.clone();
                    tokio::spawn(async move {
                        tx_refresh.send(Action::SetStatus("Refreshing...".to_string())).ok();
                        if let Ok(pkgs) = backend::paru::Paru::get_installed().await {
                            tx_refresh.send(Action::SetInstalled(pkgs)).ok();
                        }
                        if let Ok(updates) = backend::paru::Paru::get_updates().await {
                            tx_refresh.send(Action::SetUpdates(updates)).ok();
                        }
                        tx_refresh.send(Action::SetStatus("Ready".to_string())).ok();
                    });
                }
                Action::ToggleSelect(name) => {
                    if app.selected_packages.contains(&name) {
                        app.selected_packages.remove(&name);
                    } else {
                        app.selected_packages.insert(name);
                    }
                }
                Action::UpdateSingle(pkg_name) => {
                    tx.send(Action::InstallPackages(vec![pkg_name])).ok();
                }
                Action::UpdateAll => {
                    disable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        LeaveAlternateScreen,
                        DisableMouseCapture
                    )?;
                    terminal.show_cursor()?;

                    println!("\n>>> paru -Sua\n");
                    let status = std::process::Command::new("paru")
                        .arg("-Sua")
                        .status();

                    match status {
                        Ok(s) if s.success() => {
                            println!("\n✅ Update complete. Press Enter to return...");
                        }
                        Ok(s) => {
                            println!("\n⚠ paru exited with code: {}. Press Enter to return...",
                                s.code().unwrap_or(-1));
                        }
                        Err(e) => {
                            println!("\n❌ Failed to run paru: {}. Press Enter to return...", e);
                        }
                    }

                    let _ = std::io::stdin().read_line(&mut String::new());

                    enable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        EnterAlternateScreen,
                        EnableMouseCapture
                    )?;

                    let tx_refresh = tx.clone();
                    tokio::spawn(async move {
                        tx_refresh.send(Action::SetStatus("Refreshing...".to_string())).ok();
                        if let Ok(pkgs) = backend::paru::Paru::get_installed().await {
                            tx_refresh.send(Action::SetInstalled(pkgs)).ok();
                        }
                        if let Ok(updates) = backend::paru::Paru::get_updates().await {
                            tx_refresh.send(Action::SetUpdates(updates)).ok();
                        }
                        tx_refresh.send(Action::SetStatus("Ready".to_string())).ok();
                    });
                }
                Action::CleanCache(name) => {
                    app.status_message = Some(format!("Deleting cache for {}...", name));
                    app.is_loading = true;
                    let tx_clean = tx.clone();
                    let name_clone = name.clone();
                    tokio::spawn(async move {
                        match backend::paru::Paru::clean_cache(name_clone).await {
                            Ok(_) => {
                                tx_clean.send(Action::CleanCacheSuccess(name)).ok();
                            }
                            Err(e) => {
                                tx_clean.send(Action::Error(format!("Failed to clean cache: {}", e))).ok();
                            }
                        }
                    });
                }
                Action::CleanCacheSuccess(name) => {
                    app.is_loading = false;
                    app.cache_entries.retain(|c| c.name != name);
                    app.status_message = Some(format!("✅ Cache for '{}' deleted.", name));
                }
                Action::CleanAllCache => {
                    app.status_message = Some("Cleaning all cache...".to_string());
                    app.is_loading = true;
                    let tx_clean = tx.clone();
                    tokio::spawn(async move {
                        match backend::paru::Paru::clean_all_cache().await {
                            Ok(_) => {
                                tx_clean.send(Action::CleanAllCacheSuccess).ok();
                            }
                            Err(e) => {
                                tx_clean.send(Action::Error(format!("Failed to clean cache: {}", e))).ok();
                            }
                        }
                    });
                }
                Action::CleanAllCacheSuccess => {
                    app.is_loading = false;
                    app.cache_entries.clear();
                    app.status_message = Some("✅ All cache cleaned.".to_string());
                }
                Action::ShowConfirm(msg, action) => {
                    app.confirm_dialog = Some((msg, action));
                }
                Action::ConfirmYes => {
                    if let Some((_, action)) = app.confirm_dialog.take() {
                        tx.send(*action).ok();
                    }
                }
                Action::ConfirmNo => {
                    app.confirm_dialog = None;
                }
                // Key handling
                Action::Key(key) => {
                    // If confirm dialog is open, only handle y/n
                    if app.confirm_dialog.is_some() {
                        match key.code {
                            KeyCode::Char('y') | KeyCode::Char('Y') => {
                                tx.send(Action::ConfirmYes).ok();
                            }
                            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                                tx.send(Action::ConfirmNo).ok();
                            }
                            _ => {}
                        }
                    } else {
                        match app.input_mode {
                            app::InputMode::Normal => match key.code {
                                KeyCode::Char('q') => {
                                    app.running = false;
                                }
                                KeyCode::Char('j') | KeyCode::Down => {
                                    app.select_next();
                                }
                                KeyCode::Char('k') | KeyCode::Up => {
                                    app.select_previous();
                                }
                                KeyCode::Tab => app.next_tab(),
                                KeyCode::BackTab => app.previous_tab(),
                                KeyCode::Char('/') => {
                                    app.input_mode = app::InputMode::Editing;
                                    app.route = app::Route::Search;
                                    app.tab_index = 3;
                                }
                                // Spacebar to toggle selection
                                KeyCode::Char(' ') => {
                                    let pkg_name = match app.route {
                                        app::Route::Updates => {
                                            app.list_state.selected()
                                                .and_then(|i| app.updates.get(i).map(|u| u.name.clone()))
                                        }
                                        app::Route::Search => {
                                            app.list_state.selected()
                                                .and_then(|i| app.search_results.get(i).map(|p| p.name.clone()))
                                        }
                                        _ => None,
                                    };
                                    if let Some(name) = pkg_name {
                                        tx.send(Action::ToggleSelect(name)).ok();
                                    }
                                }
                                // Security scan on selected package
                                KeyCode::Char('s') => {
                                    let pkg_name = match app.route {
                                        app::Route::Updates => {
                                            app.list_state.selected()
                                                .and_then(|i| app.updates.get(i).map(|u| u.name.clone()))
                                        }
                                        app::Route::Search => {
                                            app.list_state.selected()
                                                .and_then(|i| app.search_results.get(i).map(|p| p.name.clone()))
                                        }
                                        app::Route::Installed => {
                                            app.list_state.selected()
                                                .and_then(|i| app.installed_packages.get(i).map(|p| p.name.clone()))
                                        }
                                        _ => None,
                                    };
                                    if let Some(name) = pkg_name {
                                        tx.send(Action::ScanPackage(name)).ok();
                                    }
                                }
                                // Install selected package(s) (Enter in Updates/Search)
                                KeyCode::Enter => {
                                    if !app.selected_packages.is_empty() {
                                        let selected: Vec<String> = app.selected_packages.iter().cloned().collect();
                                        let count = selected.len();
                                        tx.send(Action::ShowConfirm(
                                            format!("Install {} selected packages?", count),
                                            Box::new(Action::InstallPackages(selected)),
                                        )).ok();
                                    } else {
                                        match app.route {
                                            app::Route::Updates => {
                                                if let Some(i) = app.list_state.selected() {
                                                    if let Some(u) = app.updates.get(i) {
                                                        let name = u.name.clone();
                                                        tx.send(Action::ShowConfirm(
                                                            format!("Install update for '{}'?", name),
                                                            Box::new(Action::InstallPackages(vec![name])),
                                                        )).ok();
                                                    }
                                                }
                                            }
                                            app::Route::Search => {
                                                if let Some(i) = app.list_state.selected() {
                                                    if let Some(p) = app.search_results.get(i) {
                                                        let name = p.name.clone();
                                                        tx.send(Action::ShowConfirm(
                                                            format!("Install '{}'?", name),
                                                            Box::new(Action::InstallPackages(vec![name])),
                                                        )).ok();
                                                    }
                                                }
                                            }
                                            app::Route::Installed => {
                                                // Show details
                                                if let Some(i) = app.list_state.selected() {
                                                    if let Some(p) = app.installed_packages.get(i) {
                                                        let name = p.name.clone();
                                                        let tx_info = tx.clone();
                                                        app.is_loading = true;
                                                        app.status_message = Some(format!("Loading info for {}...", name));
                                                        tokio::spawn(async move {
                                                            match backend::paru::Paru::get_info(&name).await {
                                                                Ok(Some(pkg)) => tx_info.send(Action::SetPackageInfo(pkg)).ok(),
                                                                Ok(None) => tx_info.send(Action::Error(format!("Package '{}' not found", name))).ok(),
                                                                Err(e) => tx_info.send(Action::Error(format!("Failed: {}", e))).ok(),
                                                            }
                                                        });
                                                    }
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                // Update selected
                                KeyCode::Char('u') => {
                                    if app.route == app::Route::Updates {
                                        if let Some(i) = app.list_state.selected() {
                                            if let Some(u) = app.updates.get(i) {
                                                let name = u.name.clone();
                                                tx.send(Action::ShowConfirm(
                                                    format!("Update '{}'?", name),
                                                    Box::new(Action::UpdateSingle(name)),
                                                )).ok();
                                            }
                                        }
                                    }
                                }
                                // Update all
                                KeyCode::Char('U') => {
                                    if app.route == app::Route::Updates && !app.updates.is_empty() {
                                        tx.send(Action::ShowConfirm(
                                            format!("Update all {} packages?", app.updates.len()),
                                            Box::new(Action::UpdateAll),
                                        )).ok();
                                    }
                                }
                                // Cache: delete selected
                                KeyCode::Char('d') => {
                                    if app.route == app::Route::Cache {
                                        if let Some(i) = app.list_state.selected() {
                                            if let Some(c) = app.cache_entries.get(i) {
                                                let name = c.name.clone();
                                                tx.send(Action::ShowConfirm(
                                                    format!("Delete cache for '{}'?", name),
                                                    Box::new(Action::CleanCache(name)),
                                                )).ok();
                                            }
                                        }
                                    }
                                }
                                // Cache: delete all
                                KeyCode::Char('D') => {
                                    if app.route == app::Route::Cache && !app.cache_entries.is_empty() {
                                        tx.send(Action::ShowConfirm(
                                            "Delete ALL cache entries?".to_string(),
                                            Box::new(Action::CleanAllCache),
                                        )).ok();
                                    }
                                }
                                _ => {}
                            },
                            app::InputMode::Editing => match key.code {
                                KeyCode::Esc => {
                                    app.input_mode = app::InputMode::Normal;
                                }
                                KeyCode::Enter => {
                                    app.input_mode = app::InputMode::Normal;
                                    let query = app.search_input.value().to_string();
                                    if query.is_empty() {
                                        continue;
                                    }
                                    let tx_search = tx.clone();
                                    app.is_loading = true;
                                    app.search_results.clear();

                                    tokio::spawn(async move {
                                        let aur = backend::aur::AurClient::new();
                                        match aur.search(&query).await {
                                            Ok(pkgs) => tx_search.send(Action::SetSearchResults(pkgs)).ok(),
                                            Err(e) => tx_search.send(Action::Error(format!("Search failed: {}", e))).ok(),
                                        };
                                    });
                                }
                                _ => {
                                    use tui_input::backend::crossterm::EventHandler;
                                    app.search_input.handle_event(&crossterm::event::Event::Key(key));
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        if !app.running {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
