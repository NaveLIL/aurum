mod app;
mod config;
mod types;
mod ui;
mod backend;
mod scanner;
mod action;
mod theme;

fn translate_cyrillic(c: char) -> char {
    match c {
        // Lowercase
        'й' => 'q', 'ц' => 'w', 'у' => 'e', 'к' => 'r', 'е' => 't', 'н' => 'y', 'г' => 'u', 'ш' => 'i', 'щ' => 'o', 'з' => 'p', 'х' => '[', 'ъ' => ']',
        'ф' => 'a', 'ы' => 's', 'в' => 'd', 'а' => 'f', 'п' => 'g', 'р' => 'h', 'о' => 'j', 'л' => 'k', 'д' => 'l', 'ж' => ';', 'э' => '\'',
        'я' => 'z', 'ч' => 'x', 'с' => 'c', 'м' => 'v', 'и' => 'b', 'т' => 'n', 'ь' => 'm', 'б' => ',', 'ю' => '.',
        // Uppercase
        'Й' => 'Q', 'Ц' => 'W', 'У' => 'E', 'К' => 'R', 'Е' => 'T', 'Н' => 'Y', 'Г' => 'U', 'Ш' => 'I', 'Щ' => 'O', 'З' => 'P', 'Х' => '{', 'Ъ' => '}',
        'Ф' => 'A', 'Ы' => 'S', 'В' => 'D', 'А' => 'F', 'П' => 'G', 'Р' => 'H', 'О' => 'J', 'Л' => 'K', 'Д' => 'L', 'Ж' => ':', 'Э' => '"',
        'Я' => 'Z', 'Ч' => 'X', 'С' => 'C', 'М' => 'V', 'И' => 'B', 'Т' => 'N', 'Ь' => 'M', 'Б' => '<', 'Ю' => '>',
        _ => c,
    }
}

fn command_exists(command: &str) -> bool {
    std::process::Command::new("sh")
        .args(["-lc", &format!("command -v {} >/dev/null 2>&1", command)])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

static INTERNET_CACHE: std::sync::Mutex<Option<(std::time::Instant, bool)>> = std::sync::Mutex::new(None);

async fn check_internet_connection() -> bool {
    if let Ok(guard) = INTERNET_CACHE.lock() {
        if let Some((last_check, val)) = *guard {
            if last_check.elapsed() < std::time::Duration::from_secs(30) {
                return val;
            }
        }
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let is_online = client.get("http://clients3.google.com/generate_204")
        .send()
        .await
        .is_ok();

    if let Ok(mut guard) = INTERNET_CACHE.lock() {
        *guard = Some((std::time::Instant::now(), is_online));
    }
    is_online
}

fn spawn_refresh(tx: tokio::sync::mpsc::UnboundedSender<Action>, flatpak_available: bool) {
    tokio::spawn(async move {
        tx.send(Action::SetStatus("Checking network...".to_string())).ok();

        let is_online = check_internet_connection().await;

        let tx6 = tx.clone();
        let t6 = tokio::spawn(async move {
            if let Ok(mut info) = backend::paru::Paru::get_system_info().await {
                info.is_online = is_online;
                tx6.send(Action::SetSystemInfo(info)).ok();
            }
        });

        let tx1 = tx.clone();
        let t1 = tokio::spawn(async move {
            if let Ok(pkgs) = backend::paru::Paru::get_installed().await {
                tx1.send(Action::SetInstalled(pkgs)).ok();
            }
        });

        let tx3 = tx.clone();
        let t3 = tokio::spawn(async move {
            if let Ok(orphans) = backend::paru::Paru::get_orphans().await {
                tx3.send(Action::SetOrphans(orphans)).ok();
            }
        });

        let tx5 = tx.clone();
        let t5 = tokio::spawn(async move {
            if let Ok(stats) = backend::paru::Paru::get_disk_stats().await {
                tx5.send(Action::SetDiskStats(stats)).ok();
            }
        });

        let tx_cache = tx.clone();
        let t_cache = tokio::spawn(async move {
            if let Ok(entries) = backend::paru::Paru::get_cache_entries_with_size().await {
                tx_cache.send(Action::SetCacheEntries(entries)).ok();
            }
        });

        if !is_online {
            let _ = tokio::join!(t1, t3, t5, t6, t_cache);
            tx.send(Action::SetStatus("⚠️ Offline Mode (Network connection failed)".to_string())).ok();
            return;
        }

        tx.send(Action::SetStatus("Refreshing...".to_string())).ok();

        let tx2 = tx.clone();
        let t2 = tokio::spawn(async move {
            let mut all_updates = Vec::new();
            if let Ok(Ok(mut updates)) = tokio::time::timeout(std::time::Duration::from_secs(10), backend::paru::Paru::get_updates()).await {
                all_updates.append(&mut updates);
            }
            if flatpak_available {
                if let Ok(Ok(mut flatpak_updates)) = tokio::time::timeout(std::time::Duration::from_secs(10), backend::flatpak::Flatpak::get_updates()).await {
                    all_updates.append(&mut flatpak_updates);
                }
            }
            tx2.send(Action::SetUpdates(all_updates)).ok();
        });

        let tx4 = tx.clone();
        let t4 = tokio::spawn(async move {
            if flatpak_available {
                if let Ok(Ok(flatpaks)) = tokio::time::timeout(std::time::Duration::from_secs(10), backend::flatpak::Flatpak::get_installed()).await {
                    tx4.send(Action::SetFlatpakInstalled(flatpaks)).ok();
                }
            }
        });

        let _ = tokio::join!(t1, t2, t3, t4, t5, t6, t_cache);
        tx.send(Action::SetStatus("Ready".to_string())).ok();
    });
}

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
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if crate::types::TEAM_SIG != "EREZ Dev" {
        panic!("Critical Integrity Failure: Developer signature corrupted.");
    }
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        return handle_cli(args).await;
    }

    // Setup panic hook to restore terminal
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        crossterm::terminal::disable_raw_mode().ok();
        crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::event::DisableMouseCapture
        ).ok();
        original_hook(panic_info);
    }));

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Load config
    let mut config = Config::load().unwrap_or_default();

    // Run auto clean cache if enabled and interval has elapsed
    if config.auto_clean_cache {
        let now = chrono::Utc::now().timestamp() as u64;
        let threshold = config.auto_clean_interval_days * 24 * 60 * 60;
        if now >= config.last_cleanup_timestamp + threshold {
            config.last_cleanup_timestamp = now;
            config.save().ok();
            tokio::spawn(async move {
                let _ = backend::paru::Paru::clean_all_cache().await;
            });
        }
    }

    let mut app = App::new(config);

    // Channel for actions
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let input_active = Arc::new(AtomicBool::new(true));
    let input_paused = Arc::new(AtomicBool::new(false));

    // Initial data fetch
    let tx_load = tx.clone();
    tokio::spawn(async move {
        tx_load.send(Action::SetStatus("Checking network...".to_string())).ok();

        let is_online = check_internet_connection().await;

        let tx8 = tx_load.clone();
        let t8 = tokio::spawn(async move {
            let failed_count = match backend::systemd::get_failed_services().await {
                Ok(list) => list.len() as u32,
                Err(_) => 0,
            };
            match backend::paru::Paru::get_system_info().await {
                Ok(mut info) => {
                    info.is_online = is_online;
                    info.failed_services_count = failed_count;
                    tx8.send(Action::SetSystemInfo(info)).ok();
                }
                Err(e) => { tx8.send(Action::Error(format!("Failed to load system info: {}", e))).ok(); }
            }
        });

        let tx1 = tx_load.clone();
        let t1 = tokio::spawn(async move {
            match backend::paru::Paru::get_installed().await {
                Ok(pkgs) => { tx1.send(Action::SetInstalled(pkgs)).ok(); }
                Err(e) => { tx1.send(Action::Error(format!("Failed to load installed: {}", e))).ok(); }
            }
        });

        let tx4 = tx_load.clone();
        let t4 = tokio::spawn(async move {
            match backend::paru::Paru::get_cache_entries_with_size().await {
                Ok(entries) => { tx4.send(Action::SetCacheEntries(entries)).ok(); }
                Err(e) => { tx4.send(Action::Error(format!("Failed to scan cache: {}", e))).ok(); }
            }
        });

        let tx5 = tx_load.clone();
        let t5 = tokio::spawn(async move {
            match backend::paru::Paru::get_orphans().await {
                Ok(orphans) => { tx5.send(Action::SetOrphans(orphans)).ok(); }
                Err(e) => { tx5.send(Action::Error(format!("Failed to check orphans: {}", e))).ok(); }
            }
        });

        let tx7 = tx_load.clone();
        let t7 = tokio::spawn(async move {
            match backend::paru::Paru::get_disk_stats().await {
                Ok(stats) => { tx7.send(Action::SetDiskStats(stats)).ok(); }
                Err(e) => { tx7.send(Action::Error(format!("Failed to load disk stats: {}", e))).ok(); }
            }
        });

        if !is_online {
            let _ = tokio::join!(t1, t4, t5, t7, t8);
            tx_load.send(Action::SetStatus("⚠️ Offline Mode (Network connection failed)".to_string())).ok();
            return;
        }

        tx_load.send(Action::SetStatus("Loading data...".to_string())).ok();

        let tx2 = tx_load.clone();
        let t2 = tokio::spawn(async move {
            let mut all_updates = Vec::new();
            if let Ok(Ok(mut updates)) = tokio::time::timeout(std::time::Duration::from_secs(10), backend::paru::Paru::get_updates()).await {
                all_updates.append(&mut updates);
            }
            if backend::flatpak::Flatpak::is_available().await {
                if let Ok(Ok(mut flatpak_updates)) = tokio::time::timeout(std::time::Duration::from_secs(10), backend::flatpak::Flatpak::get_updates()).await {
                    all_updates.append(&mut flatpak_updates);
                }
            }
            tx2.send(Action::SetUpdates(all_updates)).ok();
        });

        let tx3 = tx_load.clone();
        let t3 = tokio::spawn(async move {
            if let Ok(Ok(news)) = tokio::time::timeout(std::time::Duration::from_secs(10), backend::news::fetch_arch_news()).await {
                tx3.send(Action::SetNews(news)).ok();
            }
        });

        let tx6 = tx_load.clone();
        let t6 = tokio::spawn(async move {
            let flatpak_avail = backend::flatpak::Flatpak::is_available().await;
            tx6.send(Action::SetFlatpakAvailable(flatpak_avail)).ok();
            if flatpak_avail {
                if let Ok(Ok(flatpaks)) = tokio::time::timeout(std::time::Duration::from_secs(10), backend::flatpak::Flatpak::get_installed()).await {
                    tx6.send(Action::SetFlatpakInstalled(flatpaks)).ok();
                }
            }
        });

        let _ = tokio::join!(t1, t2, t3, t4, t5, t6, t7, t8);
        tx_load.send(Action::SetStatus("Ready".to_string())).ok();
    });

    // Task to handle input events
    let tick_rate = Duration::from_millis(250);
    let tx_input = tx.clone();
    let input_active_clone = input_active.clone();
    let input_paused_clone = input_paused.clone();
    tokio::spawn(async move {
        let mut last_tick = Instant::now();
        loop {
            if !input_active_clone.load(std::sync::atomic::Ordering::SeqCst) {
                input_paused_clone.store(true, std::sync::atomic::Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(50)).await;
                continue;
            }
            input_paused_clone.store(false, std::sync::atomic::Ordering::SeqCst);

            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            #[allow(clippy::collapsible_if)]
            if crossterm::event::poll(timeout).unwrap_or(false)
                && input_active_clone.load(std::sync::atomic::Ordering::SeqCst)
            {
                if let Ok(Event::Key(key)) = event::read() {
                    if tx_input.send(Action::Key(key)).is_err() {
                        break;
                    }
                }
            }
            if last_tick.elapsed() >= tick_rate {
                if tx_input.send(Action::Tick).is_err() {
                    break;
                }
                last_tick = Instant::now();
            }
        }
    });

    // Spawn resource monitor loop
    let tx_stats = tx.clone();
    tokio::spawn(async move {
        let mut prev_cpu = None;
        loop {
            if let Ok(stats) = backend::system::get_cpu_mem_stats(&mut prev_cpu).await {
                if tx_stats.send(Action::SetCpuMemStats(stats)).is_err() {
                    break;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
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
                    app.installed_packages_set = pkgs.iter().map(|p| p.name.clone()).collect();
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
                Action::SetOrphans(orphans) => {
                    app.orphans = orphans;
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
                Action::ViewPkgbuild(pkg_name) => {
                    let tx_view = tx.clone();
                    app.status_message = Some(format!("Fetching PKGBUILD for {}...", pkg_name));
                    app.is_loading = true;

                    let installed_ver = app.installed_packages.iter()
                        .find(|p| p.name == pkg_name)
                        .and_then(|p| p.installed_version.clone())
                        .or_else(|| {
                            app.updates.iter()
                                .find(|u| u.name == pkg_name)
                                .map(|u| u.old_version.clone())
                        });

                    let name_clone = pkg_name.clone();
                    tokio::spawn(async move {
                        let raw_res = backend::paru::Paru::get_pkgbuild(&name_clone).await;
                        let diff_res = backend::paru::Paru::get_pkgbuild_diff(&name_clone, installed_ver.as_deref()).await;

                        match raw_res {
                            Ok(raw_content) => {
                                let diff_content = diff_res.unwrap_or_else(|e| format!("Could not generate diff: {}", e));
                                tx_view.send(Action::SetPkgbuildData {
                                    name: name_clone,
                                    raw_content,
                                    diff_content,
                                }).ok();
                            }
                            Err(e) => {
                                tx_view.send(Action::Error(format!("Failed to get PKGBUILD: {}", e))).ok();
                            }
                        }
                    });
                }
                Action::SetPkgbuildData { name, raw_content, diff_content } => {
                    app.is_loading = false;
                    app.pkgbuild_name = name;
                    app.pkgbuild_raw_lines = backend::highlight::highlight_pkgbuild(&raw_content);

                    if diff_content.starts_with("No differences") || diff_content.starts_with("Could not generate diff") {
                        app.pkgbuild_diff_lines = vec![ratatui::text::Line::from(ratatui::text::Span::styled(
                            diff_content.clone(),
                            ratatui::style::Style::default().fg(ratatui::style::Color::Rgb(160, 160, 180))
                        ))];
                    } else {
                        app.pkgbuild_diff_lines = backend::highlight::highlight_diff(&diff_content);
                    }

                    app.pkgbuild_scroll = 0;
                    app.pkgbuild_view_mode = if diff_content.starts_with("No differences") || diff_content.starts_with("Could not generate diff") {
                        app::PkgbuildViewMode::Full
                    } else {
                        app::PkgbuildViewMode::Diff
                    };
                    app.route = app::Route::DiffViewer;
                    app.status_message = Some("PKGBUILD and diff loaded.".to_string());
                }
                Action::TogglePkgbuildViewMode => {
                    app.pkgbuild_view_mode = match app.pkgbuild_view_mode {
                        app::PkgbuildViewMode::Full => app::PkgbuildViewMode::Diff,
                        app::PkgbuildViewMode::Diff => app::PkgbuildViewMode::Full,
                    };
                    app.pkgbuild_scroll = 0;
                }
                Action::InstallPackages(pkg_names) => {
                    if pkg_names.is_empty() {
                        continue;
                    }
                    input_active.store(false, std::sync::atomic::Ordering::SeqCst);
                    while !input_paused.load(std::sync::atomic::Ordering::SeqCst) {
                        tokio::time::sleep(Duration::from_millis(5)).await;
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
                    terminal.clear()?;

                    app.selected_packages.clear();
                    input_active.store(true, std::sync::atomic::Ordering::SeqCst);

                    // Refresh data
                    spawn_refresh(tx.clone(), app.flatpak_available);
                }
                Action::RemovePackages(pkg_names) => {
                    if pkg_names.is_empty() {
                        continue;
                    }
                    input_active.store(false, std::sync::atomic::Ordering::SeqCst);
                    while !input_paused.load(std::sync::atomic::Ordering::SeqCst) {
                        tokio::time::sleep(Duration::from_millis(5)).await;
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
                    println!("\n>>> paru -Rns {}\n", pkgs_str);
                    let status = std::process::Command::new("paru")
                        .arg("-Rns")
                        .args(&pkg_names)
                        .status();

                    match status {
                        Ok(s) if s.success() => {
                            println!("\n✅ Removal complete. Press Enter to return...");
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
                    terminal.clear()?;

                    app.selected_packages.clear();
                    input_active.store(true, std::sync::atomic::Ordering::SeqCst);

                    // Refresh data
                    spawn_refresh(tx.clone(), app.flatpak_available);
                }
                Action::ToggleSelect(name) => {
                    if app.selected_packages.contains(&name) {
                        app.selected_packages.remove(&name);
                    } else {
                        app.selected_packages.insert(name);
                    }
                }
                Action::UpdateSingle(update) => {
                    input_active.store(false, std::sync::atomic::Ordering::SeqCst);
                    while !input_paused.load(std::sync::atomic::Ordering::SeqCst) {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }

                    disable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        LeaveAlternateScreen,
                        DisableMouseCapture
                    )?;
                    terminal.show_cursor()?;

                    let status = if update.repository == "flatpak" {
                        println!("\n>>> flatpak update -y {}\n", update.name);
                        std::process::Command::new("flatpak")
                            .args(["update", "-y", &update.name])
                            .status()
                    } else {
                        println!("\n>>> paru -S {}\n", update.name);
                        std::process::Command::new("paru")
                            .args(["-S", &update.name])
                            .status()
                    };

                    match status {
                        Ok(s) if s.success() => {
                            println!("\n✅ Upgrade completed successfully. Press Enter to return...");
                        }
                        Ok(s) => {
                            println!("\n⚠️ Upgrade command exited with code: {}. Press Enter to return...", s.code().unwrap_or(-1));
                        }
                        Err(e) => {
                            println!("\n❌ Failed to run upgrade: {}. Press Enter to return...", e);
                        }
                    }

                    let _ = std::io::stdin().read_line(&mut String::new());

                    enable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        EnterAlternateScreen,
                        EnableMouseCapture
                    )?;
                    terminal.clear()?;

                    input_active.store(true, std::sync::atomic::Ordering::SeqCst);
                    spawn_refresh(tx.clone(), app.flatpak_available);
                }
                Action::UpdateAll => {
                    input_active.store(false, std::sync::atomic::Ordering::SeqCst);
                    while !input_paused.load(std::sync::atomic::Ordering::SeqCst) {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }

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
                    terminal.clear()?;

                    input_active.store(true, std::sync::atomic::Ordering::SeqCst);

                    spawn_refresh(tx.clone(), app.flatpak_available);
                }
                Action::CleanCache(entry) => {
                    app.status_message = Some(format!("Deleting cache for {}...", entry.name));
                    app.is_loading = true;
                    let tx_clean = tx.clone();
                    let entry_clone = entry.clone();
                    tokio::spawn(async move {
                        match backend::paru::Paru::clean_cache(entry_clone).await {
                            Ok(_) => {
                                tx_clean.send(Action::CleanCacheSuccess(entry.name)).ok();
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
                    spawn_refresh(tx.clone(), app.flatpak_available);
                }
                Action::CleanAllCache => {
                    app.status_message = Some("Cleaning all cache...".to_string());
                    app.is_loading = true;
                    let tx_clean = tx.clone();
                    tokio::spawn(async move {
                        match backend::paru::Paru::clean_all_cache().await {
                            Ok(skipped) => {
                                tx_clean.send(Action::CleanAllCacheSuccess(skipped)).ok();
                            }
                            Err(e) => {
                                tx_clean.send(Action::Error(format!("Failed to clean cache: {}", e))).ok();
                            }
                        }
                    });
                }
                Action::CleanAllCacheSuccess(skipped) => {
                    app.is_loading = false;
                    app.cache_entries.clear();
                    if skipped {
                        app.status_message = Some("✅ AUR cache cleaned. Pacman cache skipped (run with 'sudo' or use [c]/[C]).".to_string());
                    } else {
                        app.status_message = Some("✅ All cache cleaned.".to_string());
                    }
                    spawn_refresh(tx.clone(), app.flatpak_available);
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
                Action::SetFlatpakAvailable(avail) => {
                    app.flatpak_available = avail;
                }
                Action::SetFlatpakInstalled(apps) => {
                    app.is_loading = false;
                    app.installed_flatpaks = apps;
                }
                Action::SetFlatpakSearchResults(hits) => {
                    app.is_loading = false;
                    app.flatpak_search_results = hits;
                }
                Action::InstallFlatpakTool => {
                    if !command_exists("paru") {
                        app.status_message = Some("paru is not installed. Install paru first, then retry Flatpak support.".to_string());
                        continue;
                    }

                    input_active.store(false, std::sync::atomic::Ordering::SeqCst);
                    while !input_paused.load(std::sync::atomic::Ordering::SeqCst) {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }

                    disable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        LeaveAlternateScreen,
                        DisableMouseCapture
                    )?;
                    terminal.show_cursor()?;

                    println!("\n>>> paru -S flatpak\n");
                    let status = std::process::Command::new("paru")
                        .arg("-S")
                        .arg("flatpak")
                        .status();

                    match status {
                        Ok(s) if s.success() => {
                            println!("\n✅ Flatpak installed successfully. Press Enter to return...");
                        }
                        Ok(s) => {
                            println!("\n⚠ paru exited with code: {}. Press Enter to return...", s.code().unwrap_or(-1));
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
                    terminal.clear()?;

                    input_active.store(true, std::sync::atomic::Ordering::SeqCst);

                    let tx_check = tx.clone();
                    tokio::spawn(async move {
                        let avail = backend::flatpak::Flatpak::is_available().await;
                        tx_check.send(Action::SetFlatpakAvailable(avail)).ok();
                        if avail {
                            if let Ok(flatpaks) = backend::flatpak::Flatpak::get_installed().await {
                                tx_check.send(Action::SetFlatpakInstalled(flatpaks)).ok();
                            }
                        }
                    });
                }
                Action::InstallFlatpakApp(app_id) => {
                    input_active.store(false, std::sync::atomic::Ordering::SeqCst);
                    while !input_paused.load(std::sync::atomic::Ordering::SeqCst) {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }

                    disable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        LeaveAlternateScreen,
                        DisableMouseCapture
                    )?;
                    terminal.show_cursor()?;

                    println!("\n>>> flatpak install -y flathub {}\n", app_id);
                    let status = std::process::Command::new("flatpak")
                        .args(["install", "-y", "flathub", &app_id])
                        .status();

                    match status {
                        Ok(s) if s.success() => {
                            println!("\n✅ Application installed successfully. Press Enter to return...");
                        }
                        Ok(s) => {
                            println!("\n⚠ flatpak exited with code: {}. Press Enter to return...", s.code().unwrap_or(-1));
                        }
                        Err(e) => {
                            println!("\n❌ Failed to run flatpak: {}. Press Enter to return...", e);
                        }
                    }

                    let _ = std::io::stdin().read_line(&mut String::new());

                    enable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        EnterAlternateScreen,
                        EnableMouseCapture
                    )?;
                    terminal.clear()?;

                    input_active.store(true, std::sync::atomic::Ordering::SeqCst);

                    spawn_refresh(tx.clone(), app.flatpak_available);
                }
                Action::RemoveFlatpakApp(app_id) => {
                    input_active.store(false, std::sync::atomic::Ordering::SeqCst);
                    while !input_paused.load(std::sync::atomic::Ordering::SeqCst) {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }

                    disable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        LeaveAlternateScreen,
                        DisableMouseCapture
                    )?;
                    terminal.show_cursor()?;

                    println!("\n>>> flatpak uninstall -y {}\n", app_id);
                    let status = std::process::Command::new("flatpak")
                        .args(["uninstall", "-y", &app_id])
                        .status();

                    match status {
                        Ok(s) if s.success() => {
                            println!("\n✅ Application uninstalled successfully. Press Enter to return...");
                        }
                        Ok(s) => {
                            println!("\n⚠ flatpak exited with code: {}. Press Enter to return...", s.code().unwrap_or(-1));
                        }
                        Err(e) => {
                            println!("\n❌ Failed to run flatpak: {}. Press Enter to return...", e);
                        }
                    }

                    let _ = std::io::stdin().read_line(&mut String::new());

                    enable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        EnterAlternateScreen,
                        EnableMouseCapture
                    )?;
                    terminal.clear()?;

                    input_active.store(true, std::sync::atomic::Ordering::SeqCst);

                    spawn_refresh(tx.clone(), app.flatpak_available);
                }
                Action::SetDiskStats(stats) => {
                    app.disk_stats = stats;
                }
                Action::SetCpuMemStats(stats) => {
                    app.cpu_history.push(stats.cpu_usage as u64);
                    if app.cpu_history.len() > 100 {
                        app.cpu_history.remove(0);
                    }

                    let mem_ratio = if stats.mem_total_bytes > 0 {
                        stats.mem_used_bytes as f64 / stats.mem_total_bytes as f64
                    } else {
                        0.0
                    };
                    app.mem_history.push((mem_ratio * 100.0) as u64);
                    if app.mem_history.len() > 100 {
                        app.mem_history.remove(0);
                    }

                    app.cpu_mem_stats = stats;
                }
                Action::SetSystemInfo(info) => {
                    app.system_info = info;
                }
                Action::SetFailedServices(services) => {
                    app.failed_services = services;
                    if app.failed_services.is_empty() {
                        app.systemd_list_state.select(None);
                        app.systemd_selected_logs.clear();
                    } else {
                        let selected = app.systemd_list_state.selected().unwrap_or(0);
                        if selected >= app.failed_services.len() {
                            app.systemd_list_state.select(Some(0));
                            if let Some(first) = app.failed_services.first() {
                                tx.send(Action::StartSystemdLogsLoad(first.unit.clone())).ok();
                            }
                        } else {
                            app.systemd_list_state.select(Some(selected));
                            if let Some(service) = app.failed_services.get(selected) {
                                tx.send(Action::StartSystemdLogsLoad(service.unit.clone())).ok();
                            }
                        }
                    }
                }
                Action::SetSystemdLogs(logs) => {
                    app.systemd_logs_loading = false;
                    app.systemd_selected_logs = logs;
                }
                Action::StartSystemdLogsLoad(service) => {
                    app.systemd_logs_loading = true;
                    let tx_logs = tx.clone();
                    tokio::spawn(async move {
                        let logs = backend::systemd::get_journal_logs(&service, 50).await.unwrap_or_else(|e| {
                            vec![format!("Error loading logs: {}", e)]
                        });
                        tx_logs.send(Action::SetSystemdLogs(logs)).ok();
                    });
                }
                Action::SystemdActionSuccess(msg) => {
                    app.status_message = Some(msg);

                    let tx_refresh = tx.clone();
                    tokio::spawn(async move {
                        if let Ok(list) = backend::systemd::get_failed_services().await {
                            tx_refresh.send(Action::SetFailedServices(list)).ok();
                        }
                    });
                    spawn_refresh(tx.clone(), app.flatpak_available);
                }
                Action::SystemdRestart(unit) => {
                    input_active.store(false, std::sync::atomic::Ordering::SeqCst);
                    while !input_paused.load(std::sync::atomic::Ordering::SeqCst) {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }

                    disable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        LeaveAlternateScreen,
                        DisableMouseCapture
                    )?;
                    terminal.show_cursor()?;

                    println!("\n⚙️ Restarting systemd unit '{}'...", unit);
                    println!(">>> sudo systemctl restart {}\n", unit);

                    let status = std::process::Command::new("sudo")
                        .args(["systemctl", "restart", &unit])
                        .status();

                    match status {
                        Ok(s) if s.success() => {
                            println!("\n✅ Restarted successfully.");
                            tx.send(Action::SystemdActionSuccess(format!("Unit {} restarted successfully", unit))).ok();
                        }
                        Ok(s) => {
                            println!("\n⚠️ Command failed with code: {}.", s.code().unwrap_or(-1));
                        }
                        Err(e) => {
                            println!("\n❌ Failed to execute: {}.", e);
                        }
                    }

                    println!("\nPress Enter to return...");
                    let _ = std::io::stdin().read_line(&mut String::new());

                    enable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        EnterAlternateScreen,
                        EnableMouseCapture
                    )?;
                    terminal.clear()?;

                    input_active.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                Action::SystemdStop(unit) => {
                    input_active.store(false, std::sync::atomic::Ordering::SeqCst);
                    while !input_paused.load(std::sync::atomic::Ordering::SeqCst) {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }

                    disable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        LeaveAlternateScreen,
                        DisableMouseCapture
                    )?;
                    terminal.show_cursor()?;

                    println!("\n🛑 Stopping systemd unit '{}'...", unit);
                    println!(">>> sudo systemctl stop {}\n", unit);

                    let status = std::process::Command::new("sudo")
                        .args(["systemctl", "stop", &unit])
                        .status();

                    match status {
                        Ok(s) if s.success() => {
                            println!("\n✅ Stopped successfully.");
                            tx.send(Action::SystemdActionSuccess(format!("Unit {} stopped successfully", unit))).ok();
                        }
                        Ok(s) => {
                            println!("\n⚠️ Command failed with code: {}.", s.code().unwrap_or(-1));
                        }
                        Err(e) => {
                            println!("\n❌ Failed to execute: {}.", e);
                        }
                    }

                    println!("\nPress Enter to return...");
                    let _ = std::io::stdin().read_line(&mut String::new());

                    enable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        EnterAlternateScreen,
                        EnableMouseCapture
                    )?;
                    terminal.clear()?;

                    input_active.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                Action::SystemdDisable(unit) => {
                    input_active.store(false, std::sync::atomic::Ordering::SeqCst);
                    while !input_paused.load(std::sync::atomic::Ordering::SeqCst) {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }

                    disable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        LeaveAlternateScreen,
                        DisableMouseCapture
                    )?;
                    terminal.show_cursor()?;

                    println!("\n⚙️ Disabling systemd unit '{}'...", unit);
                    println!(">>> sudo systemctl disable {}\n", unit);

                    let status = std::process::Command::new("sudo")
                        .args(["systemctl", "disable", &unit])
                        .status();

                    match status {
                        Ok(s) if s.success() => {
                            println!("\n✅ Disabled successfully.");
                            tx.send(Action::SystemdActionSuccess(format!("Unit {} disabled successfully", unit))).ok();
                        }
                        Ok(s) => {
                            println!("\n⚠️ Command failed with code: {}.", s.code().unwrap_or(-1));
                        }
                        Err(e) => {
                            println!("\n❌ Failed to execute: {}.", e);
                        }
                    }

                    println!("\nPress Enter to return...");
                    let _ = std::io::stdin().read_line(&mut String::new());

                    enable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        EnterAlternateScreen,
                        EnableMouseCapture
                    )?;
                    terminal.clear()?;

                    input_active.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                Action::SystemUpgrade { use_snapper } => {
                    input_active.store(false, std::sync::atomic::Ordering::SeqCst);
                    while !input_paused.load(std::sync::atomic::Ordering::SeqCst) {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }

                    disable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        LeaveAlternateScreen,
                        DisableMouseCapture
                    )?;
                    terminal.show_cursor()?;

                    // Validate sudo credentials first
                    println!("🔒 Validating sudo credentials...");
                    let _ = std::process::Command::new("sudo").arg("-v").status();

                    let mut pre_num = String::new();
                    let mut snapper_success = false;

                    if use_snapper {
                        println!("\n🔨 Creating Btrfs pre-upgrade snapshot (snapper)...");
                        let snapper_out = std::process::Command::new("sudo")
                            .args(["snapper", "-c", "root", "create", "--type", "pre", "--print-number", "--description", "Before Aurum System Upgrade"])
                            .output();
                        match snapper_out {
                            Ok(out) if out.status.success() => {
                                pre_num = String::from_utf8_lossy(&out.stdout).trim().to_string();
                                println!("✅ Pre-upgrade snapshot created (ID: {}).", pre_num);
                                snapper_success = true;
                            }
                            Ok(out) => {
                                println!("⚠️  Failed to create snapper snapshot: {}", String::from_utf8_lossy(&out.stderr));
                            }
                            Err(e) => {
                                println!("⚠️  Failed to execute snapper: {}", e);
                            }
                        }
                    }

                    println!("\n>>> paru -Syu\n");
                    let status = std::process::Command::new("paru")
                        .args(["-Syu"])
                        .status();

                    let upgrade_success = match status {
                        Ok(s) if s.success() => {
                            println!("\n✅ System upgrade completed successfully.");
                            true
                        }
                        Ok(s) => {
                            println!("\n⚠️ Upgrade command exited with code: {}.", s.code().unwrap_or(-1));
                            false
                        }
                        Err(e) => {
                            println!("\n❌ Failed to run upgrade: {}.", e);
                            false
                        }
                    };

                    if upgrade_success && snapper_success && !pre_num.is_empty() {
                        println!("\n🔨 Creating Btrfs post-upgrade snapshot (snapper)...");
                        let snapper_post = std::process::Command::new("sudo")
                            .args(["snapper", "-c", "root", "create", "--type", "post", "--pre-number", &pre_num, "--description", "After Aurum System Upgrade"])
                            .status();
                        match snapper_post {
                            Ok(s) if s.success() => {
                                println!("✅ Post-upgrade snapshot created.");
                            }
                            _ => {
                                println!("⚠️  Failed to create post-upgrade snapshot.");
                            }
                        }
                    }

                    println!("\nPress Enter to return...");
                    let _ = std::io::stdin().read_line(&mut String::new());

                    enable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        EnterAlternateScreen,
                        EnableMouseCapture
                    )?;
                    terminal.clear()?;

                    input_active.store(true, std::sync::atomic::Ordering::SeqCst);
                    spawn_refresh(tx.clone(), app.flatpak_available);
                }
                Action::TroubleshootFixKeyring => {
                    input_active.store(false, std::sync::atomic::Ordering::SeqCst);
                    while !input_paused.load(std::sync::atomic::Ordering::SeqCst) {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }

                    disable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        LeaveAlternateScreen,
                        DisableMouseCapture
                    )?;
                    terminal.show_cursor()?;

                    println!("\n🔨 Fixing Arch Keyring and Signatures...");
                    println!(">>> sudo pacman -Sy archlinux-keyring && sudo pacman-key --refresh-keys\n");

                    let s1 = std::process::Command::new("sudo")
                        .args(["pacman", "-Sy", "archlinux-keyring"])
                        .status();

                    if let Ok(s) = s1 {
                        if s.success() {
                            let _ = std::process::Command::new("sudo")
                                .args(["pacman-key", "--refresh-keys"])
                                .status();
                            println!("\n✅ Keyring and keys updated. Press Enter to return...");
                        } else {
                            println!("\n⚠️  Keyring package update failed. Press Enter to return...");
                        }
                    } else {
                        println!("\n❌ Failed to run pacman command. Press Enter to return...");
                    }

                    let _ = std::io::stdin().read_line(&mut String::new());

                    enable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        EnterAlternateScreen,
                        EnableMouseCapture
                    )?;
                    terminal.clear()?;

                    input_active.store(true, std::sync::atomic::Ordering::SeqCst);
                    spawn_refresh(tx.clone(), app.flatpak_available);
                }
                Action::TroubleshootResetKeys => {
                    input_active.store(false, std::sync::atomic::Ordering::SeqCst);
                    while !input_paused.load(std::sync::atomic::Ordering::SeqCst) {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }

                    disable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        LeaveAlternateScreen,
                        DisableMouseCapture
                    )?;
                    terminal.show_cursor()?;

                    println!("\n🔨 Re-initializing Pacman Keys...");
                    println!(">>> sudo pacman-key --init && sudo pacman-key --populate archlinux\n");

                    let s1 = std::process::Command::new("sudo")
                        .args(["pacman-key", "--init"])
                        .status();

                    if let Ok(s) = s1 {
                        if s.success() {
                            let _ = std::process::Command::new("sudo")
                                .args(["pacman-key", "--populate", "archlinux"])
                                .status();
                            println!("\n✅ Keys re-initialized. Press Enter to return...");
                        } else {
                            println!("\n⚠️  pacman-key --init failed. Press Enter to return...");
                        }
                    } else {
                        println!("\n❌ Failed to run pacman-key command. Press Enter to return...");
                    }

                    let _ = std::io::stdin().read_line(&mut String::new());

                    enable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        EnterAlternateScreen,
                        EnableMouseCapture
                    )?;
                    terminal.clear()?;

                    input_active.store(true, std::sync::atomic::Ordering::SeqCst);
                    spawn_refresh(tx.clone(), app.flatpak_available);
                }
                Action::TroubleshootRemoveLock => {
                    let lock_path = std::path::Path::new("/var/lib/pacman/db.lck");
                    if lock_path.exists() {
                        let tx_ref = tx.clone();
                        tokio::spawn(async move {
                            let status = tokio::process::Command::new("sudo")
                                .args(["rm", "-f", "/var/lib/pacman/db.lck"])
                                .status()
                                .await;
                            match status {
                                Ok(s) if s.success() => {
                                    tx_ref.send(Action::SetStatus("✅ pacman database unlocked".to_string())).ok();
                                }
                                _ => {
                                    tx_ref.send(Action::Error("❌ Failed to unlock pacman database (sudo failed)".to_string())).ok();
                                }
                            }
                        });
                    }
                }
                Action::TroubleshootUpdateMirrors => {
                    input_active.store(false, std::sync::atomic::Ordering::SeqCst);
                    while !input_paused.load(std::sync::atomic::Ordering::SeqCst) {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }

                    disable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        LeaveAlternateScreen,
                        DisableMouseCapture
                    )?;
                    terminal.show_cursor()?;

                    println!("\n⚡ Updating Arch Linux mirrorlist using Reflector...");
                    println!(">>> sudo reflector --latest 20 --protocol https --sort rate --save /etc/pacman.d/mirrorlist && sudo pacman -Sy\n");

                    let s1 = std::process::Command::new("sudo")
                        .args(["reflector", "--latest", "20", "--protocol", "https", "--sort", "rate", "--save", "/etc/pacman.d/mirrorlist"])
                        .status();

                    if let Ok(s) = s1 {
                        if s.success() {
                            println!("\n✅ Mirrorlist updated successfully. Refreshing pacman databases...");
                            let s2 = std::process::Command::new("sudo")
                                .args(["pacman", "-Sy"])
                                .status();
                            if let Ok(s) = s2 {
                                if s.success() {
                                    println!("\n✅ Databases synchronized. Press Enter to return...");
                                } else {
                                    println!("\n⚠️  pacman database synchronization failed. Press Enter to return...");
                                }
                            } else {
                                println!("\n❌ Failed to run pacman command. Press Enter to return...");
                            }
                        } else {
                            println!("\n⚠️  reflector failed to update mirrorlist. Press Enter to return...");
                        }
                    } else {
                        println!("\n❌ Failed to run reflector command. Press Enter to return...");
                    }

                    let _ = std::io::stdin().read_line(&mut String::new());

                    enable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        EnterAlternateScreen,
                        EnableMouseCapture
                    )?;
                    terminal.clear()?;

                    input_active.store(true, std::sync::atomic::Ordering::SeqCst);
                    spawn_refresh(tx.clone(), app.flatpak_available);
                }
                Action::TroubleshootInstallLtsKernel => {
                    input_active.store(false, std::sync::atomic::Ordering::SeqCst);
                    while !input_paused.load(std::sync::atomic::Ordering::SeqCst) {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }

                    disable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        LeaveAlternateScreen,
                        DisableMouseCapture
                    )?;
                    terminal.show_cursor()?;

                    let (kernel_pkg, headers_pkg) = if app.system_info.cachyos_kernel_installed {
                        ("linux-cachyos-lts", "linux-cachyos-lts-headers")
                    } else {
                        ("linux-lts", "linux-lts-headers")
                    };

                    println!("\n🛡️ Installing Backup LTS Kernel & Headers for safety...");
                    println!(">>> paru -S {} {}\n", kernel_pkg, headers_pkg);

                    let status = std::process::Command::new("paru")
                        .args(["-S", kernel_pkg, headers_pkg])
                        .status();

                    match status {
                        Ok(s) if s.success() => {
                            println!("\n✅ LTS Kernel and Headers installed successfully. Press Enter to return...");
                        }
                        _ => {
                            println!("\n⚠️ Installation failed or was cancelled. Press Enter to return...");
                        }
                    }

                    let _ = std::io::stdin().read_line(&mut String::new());

                    enable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        EnterAlternateScreen,
                        EnableMouseCapture
                    )?;
                    terminal.clear()?;

                    input_active.store(true, std::sync::atomic::Ordering::SeqCst);
                    spawn_refresh(tx.clone(), app.flatpak_available);
                }
                Action::CleanPacmanCache(all) => {
                    input_active.store(false, std::sync::atomic::Ordering::SeqCst);
                    while !input_paused.load(std::sync::atomic::Ordering::SeqCst) {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }

                    disable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        LeaveAlternateScreen,
                        DisableMouseCapture
                    )?;
                    terminal.show_cursor()?;

                    let _ = std::process::Command::new("sudo")
                        .args(["sh", "-c", "rm -rf /var/cache/pacman/pkg/download-*"])
                        .status();

                    let status = if all {
                        println!("\n>>> yes | sudo pacman -Scc\n");
                        std::process::Command::new("sh")
                            .args(["-c", "yes | sudo pacman -Scc"])
                            .status()
                    } else if command_exists("paccache") {
                        println!("\n>>> sudo paccache -r\n");
                        std::process::Command::new("sudo")
                            .args(["paccache", "-r"])
                            .status()
                    } else {
                        println!("\n⚠️  paccache (pacman-contrib) not found. Falling back to pacman -Sc --noconfirm...\n");
                        std::process::Command::new("sudo")
                            .args(["pacman", "-Sc", "--noconfirm"])
                            .status()
                    };

                    match status {
                        Ok(s) if s.success() => {
                            println!("\n✅ Pacman cache cleaned. Press Enter to return...");
                        }
                        Ok(s) => {
                            println!("\n⚠ Command exited with code: {}. Press Enter to return...", s.code().unwrap_or(-1));
                        }
                        Err(e) => {
                            println!("\n❌ Failed to run command: {}. Press Enter to return...", e);
                        }
                    }

                    let _ = std::io::stdin().read_line(&mut String::new());

                    enable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        EnterAlternateScreen,
                        EnableMouseCapture
                    )?;
                    terminal.clear()?;

                    input_active.store(true, std::sync::atomic::Ordering::SeqCst);
                    spawn_refresh(tx.clone(), app.flatpak_available);
                }
                Action::CleanFlatpakUnused => {
                    input_active.store(false, std::sync::atomic::Ordering::SeqCst);
                    while !input_paused.load(std::sync::atomic::Ordering::SeqCst) {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }

                    disable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        LeaveAlternateScreen,
                        DisableMouseCapture
                    )?;
                    terminal.show_cursor()?;

                    println!("\n>>> flatpak uninstall --unused\n");
                    let status = std::process::Command::new("flatpak")
                        .arg("uninstall")
                        .arg("--unused")
                        .status();

                    match status {
                        Ok(s) if s.success() => {
                            println!("\n✅ Unused Flatpak runtimes removed. Press Enter to return...");
                        }
                        Ok(s) => {
                            println!("\n⚠ flatpak exited with code: {}. Press Enter to return...", s.code().unwrap_or(-1));
                        }
                        Err(e) => {
                            println!("\n❌ Failed to run flatpak: {}. Press Enter to return...", e);
                        }
                    }

                    let _ = std::io::stdin().read_line(&mut String::new());

                    enable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        EnterAlternateScreen,
                        EnableMouseCapture
                    )?;
                    terminal.clear()?;

                    input_active.store(true, std::sync::atomic::Ordering::SeqCst);
                    spawn_refresh(tx.clone(), app.flatpak_available);
                }
                Action::ToggleHelp => {
                    app.show_help = !app.show_help;
                }
                Action::Key(mut key) => {
                    if app.input_mode != app::InputMode::Editing {
                        if let KeyCode::Char(c) = key.code {
                            key.code = KeyCode::Char(translate_cyrillic(c));
                        }
                    }
                    if app.show_help {
                        app.show_help = false;
                    } else if app.route == app::Route::DiffViewer {
                        match key.code {
                            KeyCode::Char('j') | KeyCode::Down => {
                                let active_len = match app.pkgbuild_view_mode {
                                    app::PkgbuildViewMode::Full => app.pkgbuild_raw_lines.len(),
                                    app::PkgbuildViewMode::Diff => app.pkgbuild_diff_lines.len(),
                                };
                                if app.pkgbuild_scroll + 1 < active_len {
                                    app.pkgbuild_scroll += 1;
                                }
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                if app.pkgbuild_scroll > 0 {
                                    app.pkgbuild_scroll -= 1;
                                }
                            }
                            KeyCode::Char('d') | KeyCode::Char('D') => {
                                tx.send(Action::TogglePkgbuildViewMode).ok();
                            }
                            KeyCode::Esc | KeyCode::Char('q') => {
                                app.route = app::App::tab_route(app.tab_index);
                            }
                            _ => {}
                        }
                    } else if app.route == app::Route::PackageDetails {
                        match key.code {
                            KeyCode::Esc | KeyCode::Char('q') => {
                                app.route = app::App::tab_route(app.tab_index);
                            }
                            KeyCode::Char('o') | KeyCode::Char('O') => {
                                if let Some(ref pkg) = app.selected_package {
                                    if let Some(ref url) = pkg.url {
                                        if !url.is_empty() {
                                            let _ = std::process::Command::new("xdg-open")
                                                .arg(url)
                                                .spawn();
                                            app.status_message = Some(format!("Opening homepage: {}", url));
                                        }
                                    }
                                }
                            }
                            KeyCode::Char('i') | KeyCode::Char('I') => {
                                if let Some(ref pkg) = app.selected_package {
                                    let name = pkg.name.clone();
                                    tx.send(Action::ShowConfirm(
                                        format!("Install package '{}'?", name),
                                        Box::new(Action::InstallPackages(vec![name])),
                                    )).ok();
                                }
                            }
                            _ => {}
                        }
                    } else if app.route == app::Route::Systemd {
                        match key.code {
                            KeyCode::Esc | KeyCode::Char('q') => {
                                app.route = app::Route::Dashboard;
                            }
                            KeyCode::Char('j') | KeyCode::Down => {
                                let len = app.failed_services.len();
                                if len > 0 {
                                    let i = match app.systemd_list_state.selected() {
                                        Some(i) => if i >= len - 1 { 0 } else { i + 1 },
                                        None => 0,
                                    };
                                    app.systemd_list_state.select(Some(i));
                                    if let Some(service) = app.failed_services.get(i) {
                                        tx.send(Action::StartSystemdLogsLoad(service.unit.clone())).ok();
                                    }
                                }
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                let len = app.failed_services.len();
                                if len > 0 {
                                    let i = match app.systemd_list_state.selected() {
                                        Some(i) => if i == 0 { len - 1 } else { i - 1 },
                                        None => 0,
                                    };
                                    app.systemd_list_state.select(Some(i));
                                    if let Some(service) = app.failed_services.get(i) {
                                        tx.send(Action::StartSystemdLogsLoad(service.unit.clone())).ok();
                                    }
                                }
                            }
                            KeyCode::Char('r') | KeyCode::Char('R') => {
                                if let Some(i) = app.systemd_list_state.selected() {
                                    if let Some(service) = app.failed_services.get(i) {
                                        let unit = service.unit.clone();
                                        tx.send(Action::ShowConfirm(
                                            format!("Restart systemd unit '{}'?", unit),
                                            Box::new(Action::SystemdRestart(unit)),
                                        )).ok();
                                    }
                                }
                            }
                            KeyCode::Char('s') | KeyCode::Char('S') => {
                                if let Some(i) = app.systemd_list_state.selected() {
                                    if let Some(service) = app.failed_services.get(i) {
                                        let unit = service.unit.clone();
                                        tx.send(Action::ShowConfirm(
                                            format!("Stop systemd unit '{}'?", unit),
                                            Box::new(Action::SystemdStop(unit)),
                                        )).ok();
                                    }
                                }
                            }
                            KeyCode::Char('d') | KeyCode::Char('D') => {
                                if let Some(i) = app.systemd_list_state.selected() {
                                    if let Some(service) = app.failed_services.get(i) {
                                        let unit = service.unit.clone();
                                        tx.send(Action::ShowConfirm(
                                            format!("Disable systemd unit '{}'?", unit),
                                            Box::new(Action::SystemdDisable(unit)),
                                        )).ok();
                                    }
                                }
                            }
                            _ => {}
                        }
                    } else if app.confirm_dialog.is_some() {
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
                                KeyCode::Char('?') => {
                                    tx.send(Action::ToggleHelp).ok();
                                }
                                KeyCode::Char('j') | KeyCode::Down => {
                                    if app.route == app::Route::Store {
                                        if app.store_active_pane == 0 {
                                            let cat_count = backend::store::get_categories().len();
                                            if cat_count > 0 {
                                                app.store_category_index = (app.store_category_index + 1) % cat_count;
                                                app.store_app_index = 0;
                                            }
                                        } else {
                                            let current_cat = backend::store::get_categories()[app.store_category_index];
                                            let apps_count = backend::store::get_apps_by_category(current_cat).len();
                                            if apps_count > 0 {
                                                app.store_app_index = (app.store_app_index + 1) % apps_count;
                                            }
                                        }
                                    } else if app.route == app::Route::Cache {
                                        if app.cache_active_pane == 0 {
                                            app.select_next();
                                        } else {
                                            let len = app.orphans.len();
                                            if len > 0 {
                                                let i = match app.orphans_list_state.selected() {
                                                    Some(i) => if i >= len - 1 { 0 } else { i + 1 },
                                                    None => 0,
                                                };
                                                app.orphans_list_state.select(Some(i));
                                            }
                                        }
                                    } else if app.route == app::Route::Settings {
                                        let total_items = 6 + app.config.risky_patterns.len() + 1;
                                        app.settings_selected_index = (app.settings_selected_index + 1) % total_items;
                                    } else {
                                        app.select_next();
                                    }
                                }
                                KeyCode::Char('k') | KeyCode::Up => {
                                    if app.route == app::Route::Store {
                                        if app.store_active_pane == 0 {
                                            let cat_count = backend::store::get_categories().len();
                                            if cat_count > 0 {
                                                if app.store_category_index > 0 {
                                                    app.store_category_index -= 1;
                                                } else {
                                                    app.store_category_index = cat_count - 1;
                                                }
                                                app.store_app_index = 0;
                                            }
                                        } else {
                                            let current_cat = backend::store::get_categories()[app.store_category_index];
                                            let apps_count = backend::store::get_apps_by_category(current_cat).len();
                                            if apps_count > 0 {
                                                if app.store_app_index > 0 {
                                                    app.store_app_index -= 1;
                                                } else {
                                                    app.store_app_index = apps_count - 1;
                                                }
                                            }
                                        }
                                    } else if app.route == app::Route::Cache {
                                        if app.cache_active_pane == 0 {
                                            app.select_previous();
                                        } else {
                                            let len = app.orphans.len();
                                            if len > 0 {
                                                let i = match app.orphans_list_state.selected() {
                                                    Some(i) => if i == 0 { len - 1 } else { i - 1 },
                                                    None => 0,
                                                };
                                                app.orphans_list_state.select(Some(i));
                                            }
                                        }
                                    } else if app.route == app::Route::Settings {
                                        let total_items = 6 + app.config.risky_patterns.len() + 1;
                                        app.settings_selected_index = (app.settings_selected_index + total_items - 1) % total_items;
                                    } else {
                                        app.select_previous();
                                    }
                                }
                                KeyCode::Char('h') | KeyCode::Left => {
                                    if app.route == app::Route::Store {
                                        app.store_active_pane = 0;
                                    } else if app.route == app::Route::Cache {
                                        app.cache_active_pane = 0;
                                    } else if app.route == app::Route::Settings {
                                        if app.settings_selected_index == 0 {
                                            let theme_options = ["default", "nord", "gruvbox", "dracula", "cyberpunk"];
                                            let current_idx = theme_options.iter().position(|t| t.eq_ignore_ascii_case(&app.config.theme)).unwrap_or(0);
                                            let new_idx = if current_idx == 0 { theme_options.len() - 1 } else { current_idx - 1 };
                                            app.config.theme = theme_options[new_idx].to_string();
                                            app.config.save().ok();
                                            app.status_message = Some(format!("Theme changed to {}", theme_options[new_idx]));
                                        } else if app.settings_selected_index == 4 {
                                            app.config.auto_clean_cache = !app.config.auto_clean_cache;
                                            app.config.save().ok();
                                            app.status_message = Some(format!("Auto clean cache changed to {}", if app.config.auto_clean_cache { "On" } else { "Off" }));
                                        }
                                    }
                                }
                                KeyCode::Char('l') | KeyCode::Right => {
                                    if app.route == app::Route::Store {
                                        let current_cat = backend::store::get_categories()[app.store_category_index];
                                        let apps_count = backend::store::get_apps_by_category(current_cat).len();
                                        if apps_count > 0 {
                                            app.store_active_pane = 1;
                                        }
                                    } else if app.route == app::Route::Cache {
                                        let orphans_count = app.orphans.len();
                                        if orphans_count > 0 {
                                            app.cache_active_pane = 1;
                                            if app.orphans_list_state.selected().is_none() {
                                                app.orphans_list_state.select(Some(0));
                                            }
                                        }
                                    } else if app.route == app::Route::Settings {
                                        if app.settings_selected_index == 0 {
                                            let theme_options = ["default", "nord", "gruvbox", "dracula", "cyberpunk"];
                                            let current_idx = theme_options.iter().position(|t| t.eq_ignore_ascii_case(&app.config.theme)).unwrap_or(0);
                                            let new_idx = (current_idx + 1) % theme_options.len();
                                            app.config.theme = theme_options[new_idx].to_string();
                                            app.config.save().ok();
                                            app.status_message = Some(format!("Theme changed to {}", theme_options[new_idx]));
                                        } else if app.settings_selected_index == 4 {
                                            app.config.auto_clean_cache = !app.config.auto_clean_cache;
                                            app.config.save().ok();
                                            app.status_message = Some(format!("Auto clean cache changed to {}", if app.config.auto_clean_cache { "On" } else { "Off" }));
                                        }
                                    }
                                }
                                KeyCode::Tab => {
                                    app.next_tab();
                                }
                                KeyCode::BackTab => app.previous_tab(),
                                KeyCode::Char('[') => {
                                    app.previous_tab();
                                }
                                KeyCode::Char(']') => {
                                    app.next_tab();
                                }
                                KeyCode::Char(c) if c.is_ascii_digit() => {
                                    let digit = c.to_digit(10).unwrap() as usize;
                                    if (1..=9).contains(&digit) {
                                        app.tab_index = digit - 1;
                                        app.route = app::App::tab_route(app.tab_index);
                                        app.list_state.select(None);
                                    }
                                }
                                KeyCode::Char('t') | KeyCode::Char('T') => {
                                    match app.route {
                                        app::Route::Search => {
                                            app.search_source = match app.search_source {
                                                app::SearchSource::Aur => app::SearchSource::Flatpak,
                                                app::SearchSource::Flatpak => app::SearchSource::Aur,
                                            };
                                            app.list_state.select(None);
                                        }
                                        app::Route::Installed => {
                                            app.installed_source = match app.installed_source {
                                                app::InstalledSource::System => app::InstalledSource::Flatpak,
                                                app::InstalledSource::Flatpak => app::InstalledSource::System,
                                            };
                                            app.list_state.select(None);
                                        }
                                        _ => {}
                                    }
                                }
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
                                        app::Route::Store => {
                                            if app.store_active_pane == 1 {
                                                let current_cat = backend::store::get_categories()[app.store_category_index];
                                                let apps = backend::store::get_apps_by_category(current_cat);
                                                apps.get(app.store_app_index).map(|a| a.name.to_string())
                                            } else {
                                                None
                                            }
                                        }
                                        _ => None,
                                    };
                                    if let Some(name) = pkg_name {
                                        tx.send(Action::ToggleSelect(name)).ok();
                                    }
                                }
                                // Security scan on selected package
                                KeyCode::Char('s') => {
                                    let scan_target = match app.route {
                                        app::Route::Updates => {
                                            app.list_state.selected()
                                                .and_then(|i| app.updates.get(i).map(|u| (u.name.clone(), u.repository.clone())))
                                        }
                                        app::Route::Search => {
                                            app.list_state.selected()
                                                .and_then(|i| app.search_results.get(i).map(|p| (p.name.clone(), p.repository.clone())))
                                        }
                                        app::Route::Installed => {
                                            app.list_state.selected()
                                                .and_then(|i| app.installed_packages.get(i).map(|p| (p.name.clone(), p.repository.clone())))
                                        }
                                        _ => None,
                                    };
                                    if let Some((name, repo)) = scan_target {
                                        if repo != "aur" {
                                            tx.send(Action::ShowConfirm(
                                                "⚠️ Security scan only works for AUR packages (repo packages are pre-compiled & trusted). Press Esc/N to close.".to_string(),
                                                Box::new(Action::ConfirmNo),
                                            )).ok();
                                        } else {
                                            tx.send(Action::ScanPackage(name)).ok();
                                        }
                                    }
                                }
                                // View PKGBUILD
                                KeyCode::Char('v') => {
                                    let view_target = match app.route {
                                        app::Route::Updates => {
                                            app.list_state.selected()
                                                .and_then(|i| app.updates.get(i).map(|u| (u.name.clone(), u.repository.clone())))
                                        }
                                        app::Route::Search => {
                                            app.list_state.selected()
                                                .and_then(|i| app.search_results.get(i).map(|p| (p.name.clone(), p.repository.clone())))
                                        }
                                        app::Route::Installed => {
                                            app.list_state.selected()
                                                .and_then(|i| app.installed_packages.get(i).map(|p| (p.name.clone(), p.repository.clone())))
                                        }
                                        app::Route::Store => {
                                            if app.store_active_pane == 1 {
                                                let current_cat = backend::store::get_categories()[app.store_category_index];
                                                let apps = backend::store::get_apps_by_category(current_cat);
                                                apps.get(app.store_app_index).map(|a| (a.name.to_string(), "aur".to_string()))
                                            } else {
                                                None
                                            }
                                        }
                                        _ => None,
                                    };
                                    if let Some((name, repo)) = view_target {
                                        if repo != "aur" {
                                            tx.send(Action::ShowConfirm(
                                                "⚠️ PKGBUILD is only available for AUR packages. Press Esc/N to close.".to_string(),
                                                Box::new(Action::ConfirmNo),
                                            )).ok();
                                        } else {
                                            tx.send(Action::ViewPkgbuild(name)).ok();
                                        }
                                    }
                                }
                                // Install selected package(s) (Enter in Updates/Search/Store)
                                KeyCode::Enter => {
                                    if !app.selected_packages.is_empty() && app.route != app::Route::Settings {
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
                                                match app.search_source {
                                                    app::SearchSource::Aur => {
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
                                                    app::SearchSource::Flatpak => {
                                                        if let Some(i) = app.list_state.selected() {
                                                            if let Some(a) = app.flatpak_search_results.get(i) {
                                                                let app_id = a.app_id.clone();
                                                                tx.send(Action::ShowConfirm(
                                                                    format!("Install Flatpak app '{}'?", a.name),
                                                                    Box::new(Action::InstallFlatpakApp(app_id)),
                                                                )).ok();
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            app::Route::Store => {
                                                let current_cat = backend::store::get_categories()[app.store_category_index];
                                                let apps = backend::store::get_apps_by_category(current_cat);
                                                if let Some(a) = apps.get(app.store_app_index) {
                                                    let name = a.name.to_string();
                                                    tx.send(Action::ShowConfirm(
                                                        format!("Install '{}'?", name),
                                                        Box::new(Action::InstallPackages(vec![name])),
                                                    )).ok();
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
                                            app::Route::Settings => {
                                                match app.settings_selected_index {
                                                    0 => {}
                                                    1 => {
                                                        app.input_mode = app::InputMode::Editing;
                                                        app.settings_field_edit = Some(app::SettingsField::CheckInterval);
                                                        app.settings_input = tui_input::Input::new(app.config.check_interval_minutes.to_string());
                                                    }
                                                    2 => {
                                                        app.input_mode = app::InputMode::Editing;
                                                        app.settings_field_edit = Some(app::SettingsField::MaxCacheSize);
                                                        app.settings_input = tui_input::Input::new(app.config.max_cache_size_mb.to_string());
                                                    }
                                                    3 => {
                                                        app.input_mode = app::InputMode::Editing;
                                                        app.settings_field_edit = Some(app::SettingsField::AurUrl);
                                                        app.settings_input = tui_input::Input::new(app.config.aur_rpc_url.clone());
                                                    }
                                                    4 => {}
                                                    5 => {
                                                        app.input_mode = app::InputMode::Editing;
                                                        app.settings_field_edit = Some(app::SettingsField::AutoCleanInterval);
                                                        app.settings_input = tui_input::Input::new(app.config.auto_clean_interval_days.to_string());
                                                    }
                                                    idx if idx >= 6 && idx < 6 + app.config.risky_patterns.len() => {
                                                        let p_idx = idx - 6;
                                                        app.input_mode = app::InputMode::Editing;
                                                        app.settings_field_edit = Some(app::SettingsField::RiskyPattern(p_idx));
                                                        app.settings_input = tui_input::Input::new(app.config.risky_patterns[p_idx].clone());
                                                    }
                                                    idx if idx == 6 + app.config.risky_patterns.len() => {
                                                        app.input_mode = app::InputMode::Editing;
                                                        app.settings_field_edit = Some(app::SettingsField::AddRiskyPattern);
                                                        app.settings_input = tui_input::Input::default();
                                                    }
                                                    _ => {}
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
                                                    Box::new(Action::UpdateSingle(u.clone())),
                                                )).ok();
                                            }
                                        }
                                    }
                                }
                                // Update all / Full System Upgrade
                                KeyCode::Char('U') => {
                                    if app.route == app::Route::Updates && !app.updates.is_empty() {
                                        if app.system_info.snapper_available {
                                            tx.send(Action::ShowConfirm(
                                                "Create Btrfs snapper snapshot and perform full system upgrade?".to_string(),
                                                Box::new(Action::SystemUpgrade { use_snapper: true }),
                                            )).ok();
                                        } else {
                                            tx.send(Action::ShowConfirm(
                                                "Perform full system upgrade (paru -Syu)?".to_string(),
                                                Box::new(Action::SystemUpgrade { use_snapper: false }),
                                            )).ok();
                                        }
                                    }
                                }
                                // Troubleshooting keys
                                KeyCode::Char('K') => {
                                    tx.send(Action::ShowConfirm(
                                        "Fix Arch keyring and refresh signature keys?".to_string(),
                                        Box::new(Action::TroubleshootFixKeyring),
                                    )).ok();
                                }
                                KeyCode::Char('L') => {
                                    if app.system_info.pacman_lock_exists {
                                        tx.send(Action::ShowConfirm(
                                            "Force unlock pacman database (delete db.lck)?".to_string(),
                                            Box::new(Action::TroubleshootRemoveLock),
                                        )).ok();
                                    }
                                }
                                KeyCode::Char('R') => {
                                    tx.send(Action::ShowConfirm(
                                        "Re-initialize and populate pacman keyring (Reset Keys)?".to_string(),
                                        Box::new(Action::TroubleshootResetKeys),
                                    )).ok();
                                }
                                KeyCode::Char('M') => {
                                    tx.send(Action::ShowConfirm(
                                        "Update fast mirrors list using Reflector? (requires sudo)".to_string(),
                                        Box::new(Action::TroubleshootUpdateMirrors),
                                    )).ok();
                                }
                                KeyCode::Char('B') => {
                                    let (kernel, _) = if app.system_info.cachyos_kernel_installed {
                                        ("linux-cachyos-lts", "linux-cachyos-lts-headers")
                                    } else {
                                        ("linux-lts", "linux-lts-headers")
                                    };
                                    tx.send(Action::ShowConfirm(
                                        format!("Install safety backup LTS kernel and headers ({})?", kernel),
                                        Box::new(Action::TroubleshootInstallLtsKernel),
                                    )).ok();
                                }
                                KeyCode::Char('S') => {
                                    app.route = app::Route::Systemd;
                                    app.systemd_list_state.select(Some(0));
                                    app.systemd_selected_logs.clear();

                                    let tx_systemd = tx.clone();
                                    app.is_loading = true;
                                    tokio::spawn(async move {
                                        if let Ok(list) = backend::systemd::get_failed_services().await {
                                            if let Some(first) = list.first() {
                                                tx_systemd.send(Action::StartSystemdLogsLoad(first.unit.clone())).ok();
                                            }
                                            tx_systemd.send(Action::SetFailedServices(list)).ok();
                                        }
                                    });
                                }
                                // Cache: delete selected / remove selected orphan package
                                KeyCode::Char('d') => {
                                    if app.route == app::Route::Cache {
                                        if app.cache_active_pane == 0 {
                                            if let Some(i) = app.list_state.selected() {
                                                if let Some(c) = app.cache_entries.get(i) {
                                                    let entry = c.clone();
                                                    tx.send(Action::ShowConfirm(
                                                        format!("Delete cache for '{}'?", entry.name),
                                                        Box::new(Action::CleanCache(entry)),
                                                    )).ok();
                                                }
                                            }
                                        } else if let Some(i) = app.orphans_list_state.selected() {
                                            if let Some(pkg) = app.orphans.get(i) {
                                                let name = pkg.name.clone();
                                                tx.send(Action::ShowConfirm(
                                                    format!("Remove orphan package '{}' ?", name),
                                                    Box::new(Action::RemovePackages(vec![name])),
                                                )).ok();
                                            }
                                        }
                                    } else if app.route == app::Route::Installed && app.installed_source == app::InstalledSource::Flatpak {
                                        if let Some(i) = app.list_state.selected() {
                                            if let Some(a) = app.installed_flatpaks.get(i) {
                                                let app_id = a.app_id.clone();
                                                tx.send(Action::ShowConfirm(
                                                    format!("Uninstall Flatpak app '{}'?", a.name),
                                                    Box::new(Action::RemoveFlatpakApp(app_id)),
                                                )).ok();
                                            }
                                        }
                                    } else if app.route == app::Route::Settings {
                                        let idx = app.settings_selected_index;
                                        if idx >= 6 && idx < 6 + app.config.risky_patterns.len() {
                                            let p_idx = idx - 6;
                                            let deleted = app.config.risky_patterns.remove(p_idx);
                                            app.config.save().ok();
                                            app.status_message = Some(format!("Deleted pattern: {}", deleted));
                                            let total_items = 6 + app.config.risky_patterns.len() + 1;
                                            if app.settings_selected_index >= total_items {
                                                app.settings_selected_index = total_items - 1;
                                            }
                                        }
                                    }
                                }
                                // Cache: delete all cache / clean all orphans
                                KeyCode::Char('D') => {
                                    if app.route == app::Route::Cache {
                                        if app.cache_active_pane == 0 {
                                            if !app.cache_entries.is_empty() {
                                                tx.send(Action::ShowConfirm(
                                                    "Delete ALL cache entries?".to_string(),
                                                    Box::new(Action::CleanAllCache),
                                                )).ok();
                                            }
                                        } else if !app.orphans.is_empty() {
                                            let names: Vec<String> = app.orphans.iter().map(|o| o.name.clone()).collect();
                                            tx.send(Action::ShowConfirm(
                                                format!("Remove ALL {} orphan packages?", names.len()),
                                                Box::new(Action::RemovePackages(names)),
                                            )).ok();
                                        }
                                    }
                                }
                                KeyCode::Char('c') => {
                                    if app.route == app::Route::Cache {
                                        tx.send(Action::ShowConfirm(
                                            "Clean old pacman cache (keep latest 3 versions)?".to_string(),
                                            Box::new(Action::CleanPacmanCache(false)),
                                        )).ok();
                                    }
                                }
                                KeyCode::Char('C') => {
                                    if app.route == app::Route::Cache {
                                        tx.send(Action::ShowConfirm(
                                            "Clean ALL pacman cache files (including installed)?".to_string(),
                                            Box::new(Action::CleanPacmanCache(true)),
                                        )).ok();
                                    }
                                }
                                KeyCode::Char('f') | KeyCode::Char('F') => {
                                    if app.route == app::Route::Cache {
                                        if app.flatpak_available {
                                            tx.send(Action::ShowConfirm(
                                                "Remove unused Flatpak runtimes?".to_string(),
                                                Box::new(Action::CleanFlatpakUnused),
                                            )).ok();
                                        }
                                    } else if !app.flatpak_available && (
                                        (app.route == app::Route::Search && app.search_source == app::SearchSource::Flatpak) ||
                                        (app.route == app::Route::Installed && app.installed_source == app::InstalledSource::Flatpak)
                                    ) {
                                        tx.send(Action::ShowConfirm(
                                            "Install Flatpak package manager?".to_string(),
                                            Box::new(Action::InstallFlatpakTool),
                                        )).ok();
                                    }
                                }
                                KeyCode::Char('o') | KeyCode::Char('O') => {
                                    if app.route == app::Route::News {
                                        if let Some(idx) = app.list_state.selected() {
                                            if let Some(news) = app.news_items.get(idx) {
                                                let link = news.link.clone();
                                                if !link.is_empty() {
                                                    let _ = std::process::Command::new("xdg-open")
                                                        .arg(&link)
                                                        .spawn();
                                                    app.status_message = Some(format!("Opening link: {}", link));
                                                }
                                            }
                                        }
                                    } else if app.route == app::Route::Search {
                                        if let Some(idx) = app.list_state.selected() {
                                            if app.search_source == app::SearchSource::Aur {
                                                if let Some(pkg) = app.search_results.get(idx) {
                                                    if let Some(ref url) = pkg.url {
                                                        if !url.is_empty() {
                                                            let _ = std::process::Command::new("xdg-open")
                                                                .arg(url)
                                                                .spawn();
                                                            app.status_message = Some(format!("Opening homepage: {}", url));
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            },
                            app::InputMode::Editing => {
                                if app.route == app::Route::Settings {
                                    match key.code {
                                        KeyCode::Esc => {
                                            app.input_mode = app::InputMode::Normal;
                                            app.settings_field_edit = None;
                                        }
                                        KeyCode::Enter => {
                                            let value = app.settings_input.value().trim().to_string();
                                            let mut success = true;
                                            if let Some(ref field) = app.settings_field_edit {
                                                match field {
                                                    app::SettingsField::CheckInterval => {
                                                        if let Ok(val) = value.parse::<u64>() {
                                                            app.config.check_interval_minutes = val;
                                                        } else {
                                                            app.status_message = Some("Error: Check Interval must be a valid number".to_string());
                                                            success = false;
                                                        }
                                                    }
                                                    app::SettingsField::MaxCacheSize => {
                                                        if let Ok(val) = value.parse::<u64>() {
                                                            app.config.max_cache_size_mb = val;
                                                        } else {
                                                            app.status_message = Some("Error: Max Cache Size must be a valid number".to_string());
                                                            success = false;
                                                        }
                                                    }
                                                    app::SettingsField::AurUrl => {
                                                        if !value.is_empty() {
                                                            app.config.aur_rpc_url = value;
                                                        } else {
                                                            app.status_message = Some("Error: AUR RPC URL cannot be empty".to_string());
                                                            success = false;
                                                        }
                                                    }
                                                    app::SettingsField::AutoCleanInterval => {
                                                        if let Ok(val) = value.parse::<u64>() {
                                                            app.config.auto_clean_interval_days = val;
                                                        } else {
                                                            app.status_message = Some("Error: Auto Clean Interval must be a valid number".to_string());
                                                            success = false;
                                                        }
                                                    }
                                                    app::SettingsField::AutoCleanCache => {}
                                                    app::SettingsField::RiskyPattern(p_idx) => {
                                                        if !value.is_empty() {
                                                            app.config.risky_patterns[*p_idx] = value;
                                                        } else {
                                                            app.status_message = Some("Error: Pattern cannot be empty".to_string());
                                                            success = false;
                                                        }
                                                    }
                                                    app::SettingsField::AddRiskyPattern => {
                                                        if !value.is_empty() {
                                                            app.config.risky_patterns.push(value);
                                                        } else {
                                                            app.status_message = Some("Error: Pattern cannot be empty".to_string());
                                                            success = false;
                                                        }
                                                    }
                                                }
                                            }
                                            if success {
                                                if let Err(e) = app.config.save() {
                                                    app.status_message = Some(format!("Error saving config: {}", e));
                                                } else {
                                                    app.status_message = Some("Configuration saved successfully!".to_string());
                                                }
                                                app.input_mode = app::InputMode::Normal;
                                                app.settings_field_edit = None;
                                            }
                                        }
                                        _ => {
                                            use tui_input::backend::crossterm::EventHandler;
                                            app.settings_input.handle_event(&crossterm::event::Event::Key(key));
                                        }
                                    }
                                } else {
                                    match key.code {
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

                                            match app.search_source {
                                                app::SearchSource::Aur => {
                                                    app.search_results.clear();
                                                    tokio::spawn(async move {
                                                        let aur = backend::aur::AurClient::new();
                                                        match aur.search(&query).await {
                                                            Ok(pkgs) => tx_search.send(Action::SetSearchResults(pkgs)).ok(),
                                                            Err(e) => tx_search.send(Action::Error(format!("Search failed: {}", e))).ok(),
                                                        };
                                                    });
                                                }
                                                app::SearchSource::Flatpak => {
                                                    app.flatpak_search_results.clear();
                                                    tokio::spawn(async move {
                                                        match backend::flatpak::Flatpak::search(&query).await {
                                                            Ok(hits) => tx_search.send(Action::SetFlatpakSearchResults(hits)).ok(),
                                                            Err(e) => tx_search.send(Action::Error(format!("Flathub search failed: {}", e))).ok(),
                                                        }
                                                    });
                                                }
                                            }
                                        }
                                        _ => {
                                            use tui_input::backend::crossterm::EventHandler;
                                            app.search_input.handle_event(&crossterm::event::Event::Key(key));
                                        }
                                    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translate_cyrillic() {
        // Test lowercase mapping
        assert_eq!(translate_cyrillic('й'), 'q');
        assert_eq!(translate_cyrillic('о'), 'j');
        assert_eq!(translate_cyrillic('л'), 'k');
        assert_eq!(translate_cyrillic('в'), 'd');
        assert_eq!(translate_cyrillic('е'), 't');
        
        // Test uppercase mapping
        assert_eq!(translate_cyrillic('Й'), 'Q');
        assert_eq!(translate_cyrillic('Г'), 'U');
        assert_eq!(translate_cyrillic('В'), 'D');

        // Test non-cyrillic remains unchanged
        assert_eq!(translate_cyrillic('q'), 'q');
        assert_eq!(translate_cyrillic('1'), '1');
        assert_eq!(translate_cyrillic('['), '[');
    }

    #[test]
    fn test_expand_tilde() {
        // Test paths that don't start with ~
        assert_eq!(expand_tilde("/usr/bin"), std::path::PathBuf::from("/usr/bin"));
        assert_eq!(expand_tilde("some/relative/path"), std::path::PathBuf::from("some/relative/path"));

        // Test ~ path itself
        if let Some(home) = dirs::home_dir() {
            assert_eq!(expand_tilde("~"), home);
            assert_eq!(expand_tilde("~/Downloads"), home.join("Downloads"));
        }
    }

    #[test]
    fn test_theme_colors() {
        use crate::theme::get_theme;
        let t_nord = get_theme("nord");
        assert_eq!(t_nord.name, "Nord");
        
        let t_dracula = get_theme("Dracula");
        assert_eq!(t_dracula.name, "Dracula");

        let t_default = get_theme("unknown");
        assert_eq!(t_default.name, "Default");
    }

    #[test]
    fn test_classify_target() {
        // Test repository package names (non-path-like and don't exist)
        assert_eq!(
            classify_target("some-pkg-name"),
            InstallTargetType::RepoPackage("some-pkg-name".to_string())
        );

        // Test non-existent path-like target
        match classify_target("./nonexistent-pkg.pkg.tar.zst") {
            InstallTargetType::Invalid(_, reason) => {
                assert!(reason.contains("Path does not exist"));
            }
            other => panic!("Expected Invalid target, got {:?}", other),
        }

        // Test file format check for non-existent path
        match classify_target("/foo/bar/badfile.txt") {
            InstallTargetType::Invalid(_, reason) => {
                assert!(reason.contains("Path does not exist"));
            }
            other => panic!("Expected Invalid target, got {:?}", other),
        }

        // Test creation of actual files/directories using a temp folder
        let temp_dir = std::env::temp_dir().join("aurum_test_classify");
        let _ = std::fs::create_dir_all(&temp_dir);

        // 1. Create a valid package file
        let pkg_file = temp_dir.join("testpkg-1.0-1-x86_64.pkg.tar.zst");
        let _ = std::fs::File::create(&pkg_file);
        assert_eq!(
            classify_target(&pkg_file.to_string_lossy()),
            InstallTargetType::LocalPackageFile(pkg_file.clone())
        );

        // 2. Create an invalid file format
        let invalid_file = temp_dir.join("testpkg.txt");
        let _ = std::fs::File::create(&invalid_file);
        match classify_target(&invalid_file.to_string_lossy()) {
            InstallTargetType::Invalid(_, reason) => {
                assert!(reason.contains("File is not a valid Arch"));
            }
            other => panic!("Expected Invalid target, got {:?}", other),
        }

        // 3. Create a PKGBUILD file directly
        let pkgbuild_file = temp_dir.join("PKGBUILD");
        let _ = std::fs::File::create(&pkgbuild_file);
        assert_eq!(
            classify_target(&pkgbuild_file.to_string_lossy()),
            InstallTargetType::PkgbuildFile(pkgbuild_file.clone())
        );

        // 4. Create a PKGBUILD directory
        let pkgbuild_dir = temp_dir.join("pkgbuild_dir");
        let _ = std::fs::create_dir_all(&pkgbuild_dir);
        let _ = std::fs::File::create(pkgbuild_dir.join("PKGBUILD"));
        assert_eq!(
            classify_target(&pkgbuild_dir.to_string_lossy()),
            InstallTargetType::PkgbuildDir(pkgbuild_dir.clone())
        );

        // 5. Create a directory WITHOUT PKGBUILD
        let empty_dir = temp_dir.join("empty_dir");
        let _ = std::fs::create_dir_all(&empty_dir);
        match classify_target(&empty_dir.to_string_lossy()) {
            InstallTargetType::Invalid(_, reason) => {
                assert!(reason.contains("Directory does not contain a PKGBUILD"));
            }
            other => panic!("Expected Invalid target, got {:?}", other),
        }

        // 6. Test Git URLs
        assert_eq!(
            classify_target("https://aur.archlinux.org/paru-git.git"),
            InstallTargetType::GitUrl("https://aur.archlinux.org/paru-git.git".to_string())
        );
        assert_eq!(
            classify_target("http://github.com/some/repo.git"),
            InstallTargetType::GitUrl("http://github.com/some/repo.git".to_string())
        );

        // 7. Test Tarball URLs
        assert_eq!(
            classify_target("https://aur.archlinux.org/cgit/aur.git/snapshot/paru-git.tar.gz"),
            InstallTargetType::TarballUrl("https://aur.archlinux.org/cgit/aur.git/snapshot/paru-git.tar.gz".to_string())
        );
        assert_eq!(
            classify_target("https://example.com/package.tar.xz"),
            InstallTargetType::TarballUrl("https://example.com/package.tar.xz".to_string())
        );

        // 8. Test Invalid URLs
        match classify_target("https://google.com") {
            InstallTargetType::Invalid(_, reason) => {
                assert!(reason.contains("URL is neither a git repository"));
            }
            other => panic!("Expected Invalid target, got {:?}", other),
        }

        // Cleanup temp folder
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_is_pacman_installed() {
        // 'pacman' package itself must be installed on Arch Linux/CachyOS systems
        assert!(is_pacman_installed("pacman"));

        // Non-existent package should not be installed
        assert!(!is_pacman_installed("this-package-does-not-exist-12345"));
    }

    #[test]
    fn test_is_snapper_available() {
        // Should run without crashing, regardless of whether snapper is installed or not
        let _ = is_snapper_available();
    }
}

fn expand_tilde<P: AsRef<std::path::Path>>(path: P) -> std::path::PathBuf {
    let path = path.as_ref();
    if path.starts_with("~") {
        if let Some(home) = dirs::home_dir() {
            if path == std::path::Path::new("~") {
                return home;
            }
            if let Ok(stripped) = path.strip_prefix("~") {
                return home.join(stripped);
            }
        }
    }
    path.to_path_buf()
}

#[derive(Debug, PartialEq, Eq)]
enum InstallTargetType {
    LocalPackageFile(std::path::PathBuf),
    PkgbuildFile(std::path::PathBuf),
    PkgbuildDir(std::path::PathBuf),
    RepoPackage(String),
    GitUrl(String),
    TarballUrl(String),
    Invalid(String, String), // target, reason
}

fn classify_target(target: &str) -> InstallTargetType {
    if target.starts_with("http://") || target.starts_with("https://") {
        if target.ends_with(".tar.gz")
            || target.ends_with(".tar.xz")
            || target.ends_with(".tar.zst")
            || target.ends_with(".zip")
            || target.contains("/snapshot/")
        {
            InstallTargetType::TarballUrl(target.to_string())
        } else if target.ends_with(".git") || target.contains("/aur.git/") || target.contains("/aur.git") {
            InstallTargetType::GitUrl(target.to_string())
        } else {
            InstallTargetType::Invalid(
                target.to_string(),
                "URL is neither a git repository (.git) nor a supported archive (.tar.gz, .tar.xz, .tar.zst, .zip, or snapshot).".to_string()
            )
        }
    } else {
        let expanded = expand_tilde(target);
        let path = std::path::Path::new(&expanded);
        let is_path_like = target.contains('/') || target.contains('.') || target.starts_with('~');

        if path.exists() {
            if path.is_file() {
                let filename = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
                if filename == "PKGBUILD" {
                    InstallTargetType::PkgbuildFile(path.to_path_buf())
                } else {
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    if ext == "zst" || ext == "xz" {
                        InstallTargetType::LocalPackageFile(path.to_path_buf())
                    } else {
                        InstallTargetType::Invalid(target.to_string(), "File is not a valid Arch package archive (.pkg.tar.zst or .pkg.tar.xz) or PKGBUILD.".to_string())
                    }
                }
            } else if path.is_dir() {
                if path.join("PKGBUILD").exists() {
                    InstallTargetType::PkgbuildDir(path.to_path_buf())
                } else {
                    InstallTargetType::Invalid(target.to_string(), "Directory does not contain a PKGBUILD file.".to_string())
                }
            } else {
                InstallTargetType::Invalid(target.to_string(), "Path is neither a regular file nor a directory.".to_string())
            }
        } else if is_path_like {
            InstallTargetType::Invalid(target.to_string(), "Path does not exist.".to_string())
        } else {
            InstallTargetType::RepoPackage(target.to_string())
        }
    }
}

fn is_pacman_installed(pkg: &str) -> bool {
    std::process::Command::new("pacman")
        .args(["-Q", pkg])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn is_snapper_available() -> bool {
    let snapper_exists = std::process::Command::new("which")
        .arg("snapper")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !snapper_exists {
        return false;
    }

    let output = std::process::Command::new("snapper")
        .arg("list-configs")
        .output();

    if let Ok(out) = output {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            text.lines().any(|l| l.contains("root"))
        } else {
            false
        }
    } else {
        false
    }
}


async fn handle_cli(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let cmd = args[1].as_str();
    if cmd == "install" || cmd == "i" {
        if args.len() < 3 {
            println!("⚠️ Error: Please specify a package name, file path, or directory containing a PKGBUILD.");
            println!("Usage: aurum install <package | file | directory>");
            std::process::exit(1);
        }

        let targets = &args[2..];
        let mut local_packages = Vec::new();
        let mut pkgbuild_dirs = Vec::new();
        let mut pkgbuild_files = Vec::new();
        let mut repo_packages = Vec::new();
        let mut git_urls = Vec::new();
        let mut tarball_urls = Vec::new();
        let mut invalid_targets = Vec::new();

        for target in targets {
            match classify_target(target) {
                InstallTargetType::LocalPackageFile(path) => local_packages.push(path),
                InstallTargetType::PkgbuildFile(path) => pkgbuild_files.push(path),
                InstallTargetType::PkgbuildDir(path) => pkgbuild_dirs.push(path),
                InstallTargetType::RepoPackage(name) => repo_packages.push(name),
                InstallTargetType::GitUrl(url) => git_urls.push(url),
                InstallTargetType::TarballUrl(url) => tarball_urls.push(url),
                InstallTargetType::Invalid(t, reason) => invalid_targets.push((t, reason)),
            }
        }

        // 1. Check for invalid targets
        if !invalid_targets.is_empty() {
            for (target, reason) in invalid_targets {
                println!("❌ Error: Invalid target '{}' - {}", target, reason);
            }
            std::process::exit(1);
        }

        // 2. Check for mixed targets
        let has_local_pkg = !local_packages.is_empty();
        let has_pkgbuild = !pkgbuild_dirs.is_empty() || !pkgbuild_files.is_empty();
        let has_repo = !repo_packages.is_empty();
        let has_git_url = !git_urls.is_empty();
        let has_tarball_url = !tarball_urls.is_empty();

        let mut types_count = 0;
        if has_local_pkg { types_count += 1; }
        if has_pkgbuild { types_count += 1; }
        if has_repo { types_count += 1; }
        if has_git_url { types_count += 1; }
        if has_tarball_url { types_count += 1; }

        if types_count > 1 {
            println!("❌ Error: Mixed installation targets are not supported.");
            println!("Please do not mix repository package names, local package files, PKGBUILD sources, or URLs in a single command.");
            println!("Install them separately.");
            std::process::exit(1);
        }

        // 3. Execute installation based on type
        if has_local_pkg {
            let files: Vec<String> = local_packages.iter().map(|p| p.to_string_lossy().into_owned()).collect();
            println!("📦 Detected local Arch package archive(s):");
            for f in &files {
                println!("  • {}", f);
                let metadata = std::process::Command::new("pacman")
                    .args(["-Qip", f])
                    .output();
                match metadata {
                    Ok(out) if out.status.success() => {
                        let text = String::from_utf8_lossy(&out.stdout);
                        for line in text.lines() {
                            let l = line.trim();
                            if l.starts_with("Name")
                                || l.starts_with("Version")
                                || l.starts_with("Description")
                                || l.starts_with("Installed Size")
                                || l.starts_with("Packager")
                            {
                                println!("    {}", l);
                            }
                        }
                    }
                    _ => {
                        println!("    (Could not retrieve package metadata)");
                    }
                }
            }

            print!("\nDo you want to proceed with installation? [y/N] ");
            use std::io::Write;
            let _ = std::io::stdout().flush();
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            let reply = input.trim().to_lowercase();
            if reply != "y" && reply != "yes" {
                println!("Installation aborted.");
                return Ok(());
            }

            println!("\n>>> sudo pacman -U {}", files.join(" "));
            let status = std::process::Command::new("sudo")
                .arg("pacman")
                .arg("-U")
                .args(&files)
                .status()?;
            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
            return Ok(());
        }

        if has_pkgbuild {
            // Safety check: makepkg cannot run as root
            let is_root = std::process::Command::new("id")
                .arg("-u")
                .output()
                .map(|out| String::from_utf8_lossy(&out.stdout).trim() == "0")
                .unwrap_or(false);
                
            if is_root {
                println!("\n❌ Error: You are running aurum as root/sudo.");
                println!("Building Arch packages (makepkg) as root is not allowed for security reasons.");
                println!("Please run this command without sudo:");
                println!("  aurum install [targets]");
                println!("Aurum will request password escalation automatically during installation.");
                std::process::exit(1);
            }

            // Build PKGBUILD files first
            for file_path in &pkgbuild_files {
                let dir = file_path.parent().unwrap_or(std::path::Path::new("."));
                println!("🛠️ Detected PKGBUILD file: {}", file_path.display());
                println!(">>> makepkg -si (inside {})", dir.display());
                let status = std::process::Command::new("makepkg")
                    .args(["-si"])
                    .current_dir(dir)
                    .status()?;
                if !status.success() {
                    std::process::exit(status.code().unwrap_or(1));
                }
            }

            // Build PKGBUILD directories
            for dir_path in &pkgbuild_dirs {
                println!("🛠️ Detected PKGBUILD source directory: {}", dir_path.display());
                println!(">>> makepkg -si (inside {})", dir_path.display());
                let status = std::process::Command::new("makepkg")
                    .args(["-si"])
                    .current_dir(dir_path)
                    .status()?;
                if !status.success() {
                    std::process::exit(status.code().unwrap_or(1));
                }
            }
            
            return Ok(());
        }

        if has_git_url {
            // Safety check: makepkg cannot run as root
            let is_root = std::process::Command::new("id")
                .arg("-u")
                .output()
                .map(|out| String::from_utf8_lossy(&out.stdout).trim() == "0")
                .unwrap_or(false);
                
            if is_root {
                println!("\n❌ Error: You are running aurum as root/sudo.");
                println!("Building Arch packages (makepkg) as root is not allowed for security reasons.");
                println!("Please run this command without sudo:");
                println!("  aurum install [git-urls]");
                std::process::exit(1);
            }

            let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
            let build_dir = home.join(".cache/aurum/cli_build");
            std::fs::create_dir_all(&build_dir)?;

            for git_url in &git_urls {
                let repo_name = git_url
                    .split('/')
                    .next_back()
                    .unwrap_or("pkg")
                    .strip_suffix(".git")
                    .unwrap_or_else(|| git_url.split('/').next_back().unwrap_or("pkg"));
                let dest = build_dir.join(repo_name);

                if dest.exists() {
                    let _ = std::fs::remove_dir_all(&dest);
                }

                println!("📥 Cloning git repository: {}", git_url);
                println!(">>> git clone {} {}", git_url, dest.display());
                let status = std::process::Command::new("git")
                    .args(["clone", git_url, &dest.to_string_lossy()])
                    .status()?;
                if !status.success() {
                    println!("❌ Error: Failed to clone git repository '{}'.", git_url);
                    std::process::exit(1);
                }

                if !dest.join("PKGBUILD").exists() {
                    println!("❌ Error: Cloned repository '{}' does not contain a PKGBUILD file.", repo_name);
                    std::process::exit(1);
                }

                println!("🛠️ Building package...");
                println!(">>> makepkg -si (inside {})", dest.display());
                let status = std::process::Command::new("makepkg")
                    .args(["-si"])
                    .current_dir(&dest)
                    .status()?;
                if !status.success() {
                    std::process::exit(status.code().unwrap_or(1));
                }
            }
            return Ok(());
        }

        if has_tarball_url {
            // Safety check: makepkg cannot run as root
            let is_root = std::process::Command::new("id")
                .arg("-u")
                .output()
                .map(|out| String::from_utf8_lossy(&out.stdout).trim() == "0")
                .unwrap_or(false);
                
            if is_root {
                println!("\n❌ Error: You are running aurum as root/sudo.");
                println!("Building Arch packages (makepkg) as root is not allowed for security reasons.");
                println!("Please run this command without sudo:");
                println!("  aurum install [tarball-urls]");
                std::process::exit(1);
            }

            let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
            let build_dir = home.join(".cache/aurum/cli_build");
            std::fs::create_dir_all(&build_dir)?;

            for tarball_url in &tarball_urls {
                let filename = tarball_url.split('/').next_back().unwrap_or("archive.tar.gz");
                let download_path = build_dir.join(filename);

                println!("📥 Downloading archive: {}", tarball_url);
                let response = reqwest::get(tarball_url).await?;
                if !response.status().is_success() {
                    println!("❌ Error: Failed to download archive from '{}'. Status: {}", tarball_url, response.status());
                    std::process::exit(1);
                }
                let bytes = response.bytes().await?;
                std::fs::write(&download_path, bytes)?;

                println!("📦 Extracting archive...");
                println!(">>> tar -xf {} -C {}", download_path.display(), build_dir.display());
                let status = std::process::Command::new("tar")
                    .args(["-xf", &download_path.to_string_lossy(), "-C", &build_dir.to_string_lossy()])
                    .status()?;
                if !status.success() {
                    println!("❌ Error: Failed to extract archive.");
                    std::process::exit(1);
                }

                // Locate the extracted directory containing PKGBUILD
                let mut pkgbuild_dir = None;
                if let Ok(entries) = std::fs::read_dir(&build_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() && path.join("PKGBUILD").exists() {
                            pkgbuild_dir = Some(path);
                            break;
                        }
                    }
                }
                if build_dir.join("PKGBUILD").exists() {
                    pkgbuild_dir = Some(build_dir.to_path_buf());
                }

                if let Some(dir) = pkgbuild_dir {
                    println!("🛠️ Building package...");
                    println!(">>> makepkg -si (inside {})", dir.display());
                    let status = std::process::Command::new("makepkg")
                        .args(["-si"])
                        .current_dir(&dir)
                        .status()?;
                    
                    // Cleanup extracted directory and archive on success
                    let _ = std::fs::remove_file(&download_path);
                    let _ = std::fs::remove_dir_all(&dir);

                    if !status.success() {
                        std::process::exit(status.code().unwrap_or(1));
                    }
                } else {
                    println!("❌ Error: Could not find a directory containing a PKGBUILD in the extracted archive.");
                    let _ = std::fs::remove_file(&download_path);
                    std::process::exit(1);
                }
            }
            return Ok(());
        }

        // Default: install via paru
        println!("🔍 Installing packages via paru: {}", repo_packages.join(" "));
        println!(">>> paru -S {}", repo_packages.join(" "));
        let status = std::process::Command::new("paru")
            .arg("-S")
            .args(&repo_packages)
            .status()?;
            
        if !status.success() {
            std::process::exit(status.code().unwrap_or(1));
        }
        Ok(())
    } else if cmd == "remove" || cmd == "r" {
        if args.len() < 3 {
            println!("⚠️ Error: Please specify a package name to remove.");
            println!("Usage: aurum remove <package1 package2 ...>");
            std::process::exit(1);
        }

        let targets = &args[2..];

        // Fetch installed Flatpak application IDs
        let mut installed_flatpaks = Vec::new();
        if backend::flatpak::Flatpak::is_available().await {
            if let Ok(list) = backend::flatpak::Flatpak::get_installed().await {
                installed_flatpaks = list.into_iter().map(|app| app.app_id).collect::<Vec<String>>();
            }
        }

        let mut to_remove_pacman = Vec::new();
        let mut to_remove_flatpak = Vec::new();
        let mut not_installed = Vec::new();

        for target in targets {
            if is_pacman_installed(target) {
                to_remove_pacman.push(target.to_string());
            } else if installed_flatpaks.contains(&target.to_string()) {
                to_remove_flatpak.push(target.to_string());
            } else {
                not_installed.push(target.to_string());
            }
        }

        // Print styled summary
        println!("📋 Uninstallation Plan:");
        for pkg in &to_remove_pacman {
            println!("  \x1b[32m•\x1b[0m {:<25} \x1b[36m[pacman]\x1b[0m       🗑️  Will be removed", pkg);
        }
        for pkg in &to_remove_flatpak {
            println!("  \x1b[32m•\x1b[0m {:<25} \x1b[35m[flatpak]\x1b[0m      🗑️  Will be uninstalled", pkg);
        }
        for pkg in &not_installed {
            println!("  \x1b[33m•\x1b[0m {:<25} \x1b[33m[not installed]\x1b[0m ⚠️  Skipped (not installed)", pkg);
        }
        println!();

        if to_remove_pacman.is_empty() && to_remove_flatpak.is_empty() {
            println!("⚠️ Notice: No installed packages selected for removal.");
            std::process::exit(0);
        }

        if !not_installed.is_empty() {
            println!("⚠️ Notice: Some specified packages are not installed and will be skipped.");
        }

        print!("Do you want to proceed with uninstallation? [y/N] ");
        use std::io::Write;
        let _ = std::io::stdout().flush();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let reply = input.trim().to_lowercase();
        if reply != "y" && reply != "yes" {
            println!("Aborted.");
            return Ok(());
        }

        // Execute Pacman removals
        if !to_remove_pacman.is_empty() {
            let use_snapper = if is_snapper_available() {
                print!("Create Btrfs snapper snapshot and perform package removal? [y/N] ");
                let _ = std::io::stdout().flush();
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                let reply = input.trim().to_lowercase();
                reply == "y" || reply == "yes"
            } else {
                false
            };

            // Validate sudo credentials first
            println!("🔒 Validating sudo credentials...");
            let sudo_val = std::process::Command::new("sudo")
                .arg("-v")
                .status();
            if let Ok(s) = sudo_val {
                if !s.success() {
                    println!("❌ Error: sudo validation failed.");
                    std::process::exit(1);
                }
            } else {
                println!("❌ Error: Failed to execute sudo.");
                std::process::exit(1);
            }

            let mut pre_num = String::new();
            let mut snapper_success = false;

            if use_snapper {
                println!("\n🔨 Creating Btrfs pre-removal snapshot (snapper)...");
                let snapper_out = std::process::Command::new("sudo")
                    .args(["snapper", "-c", "root", "create", "--type", "pre", "--print-number", "--description", "Before Aurum Package Removal"])
                    .output();
                match snapper_out {
                    Ok(out) if out.status.success() => {
                        pre_num = String::from_utf8_lossy(&out.stdout).trim().to_string();
                        println!("✅ Pre-removal snapshot created (ID: {}).", pre_num);
                        snapper_success = true;
                    }
                    Ok(out) => {
                        println!("⚠️  Failed to create snapper snapshot: {}", String::from_utf8_lossy(&out.stderr));
                    }
                    Err(e) => {
                        println!("⚠️  Failed to execute snapper: {}", e);
                    }
                }
            }

            println!("\n>>> sudo pacman -Rns {}\n", to_remove_pacman.join(" "));
            let status = std::process::Command::new("sudo")
                .arg("pacman")
                .arg("-Rns")
                .args(&to_remove_pacman)
                .status()?;

            let remove_success = status.success();

            if remove_success && snapper_success && !pre_num.is_empty() {
                println!("\n🔨 Creating Btrfs post-removal snapshot (snapper)...");
                let snapper_post = std::process::Command::new("sudo")
                    .args(["snapper", "-c", "root", "create", "--type", "post", "--pre-number", &pre_num, "--description", "After Aurum Package Removal"])
                    .status();
                match snapper_post {
                    Ok(s) if s.success() => {
                        println!("✅ Post-removal snapshot created.");
                    }
                    _ => {
                        println!("⚠️  Failed to create post-removal snapshot.");
                    }
                }
            }

            if !remove_success {
                std::process::exit(status.code().unwrap_or(1));
            }
        }

        // Execute Flatpak removals
        if !to_remove_flatpak.is_empty() {
            println!("\n>>> flatpak uninstall -y {}\n", to_remove_flatpak.join(" "));
            let status = std::process::Command::new("flatpak")
                .args(["uninstall", "-y"])
                .args(&to_remove_flatpak)
                .status()?;

            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
        }

        println!("\n✅ Uninstallation process finished successfully.");
        Ok(())
    } else if cmd == "--help" || cmd == "-h" || cmd == "help" {
        print_usage();
        Ok(())
    } else {
        println!("⚠️ Unknown command: {}", cmd);
        print_usage();
        std::process::exit(1);
    }
}

fn print_usage() {
    println!("Aurum — Smart Package Manager and TUI for CachyOS / Arch Linux\n");
    println!("Usage:");
    println!("  aurum                      Launch the TUI Dashboard");
    println!("  aurum install [target]     Install packages, files, or PKGBUILD directories");
    println!("  aurum remove [packages]    Remove packages and their unused dependencies");
    println!("  aurum help, -h, --help     Show this help message\n");
    println!("Examples:");
    println!("  aurum install telegram-desktop          Install from official repositories or AUR");
    println!("  aurum install ./some-package.pkg.tar.zst Install a local package archive");
    println!("  aurum install ./local-pkgbuild-folder/  Build and install a package from source");
    println!("  aurum remove vlc                        Remove VLC and its unused dependencies");
}
