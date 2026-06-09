mod app;
mod config;
mod types;
mod ui;
mod backend;
mod scanner;
mod action;
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

    let addr = match "1.1.1.1:53".parse::<std::net::SocketAddr>() {
        Ok(a) => a,
        Err(_) => return false,
    };
    let is_online = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        tokio::net::TcpStream::connect(&addr)
    ).await.is_ok();

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

        if !is_online {
            let _ = tokio::join!(t1, t3, t5, t6);
            tx.send(Action::SetStatus("⚠️ Offline Mode (Network connection failed)".to_string())).ok();
            return;
        }

        tx.send(Action::SetStatus("Refreshing...".to_string())).ok();

        let tx2 = tx.clone();
        let t2 = tokio::spawn(async move {
            if let Ok(Ok(updates)) = tokio::time::timeout(std::time::Duration::from_secs(10), backend::paru::Paru::get_updates()).await {
                tx2.send(Action::SetUpdates(updates)).ok();
            }
        });

        let tx4 = tx.clone();
        let t4 = tokio::spawn(async move {
            if flatpak_available {
                if let Ok(Ok(flatpaks)) = tokio::time::timeout(std::time::Duration::from_secs(10), backend::flatpak::Flatpak::get_installed()).await {
                    tx4.send(Action::SetFlatpakInstalled(flatpaks)).ok();
                }
            }
        });

        let _ = tokio::join!(t1, t2, t3, t4, t5, t6);
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
    let config = Config::load().unwrap_or_default();
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
            match backend::paru::Paru::get_system_info().await {
                Ok(mut info) => {
                    info.is_online = is_online;
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
            match tokio::time::timeout(std::time::Duration::from_secs(10), backend::paru::Paru::get_updates()).await {
                Ok(Ok(updates)) => { tx2.send(Action::SetUpdates(updates)).ok(); }
                _ => {}
            }
        });

        let tx3 = tx_load.clone();
        let t3 = tokio::spawn(async move {
            match tokio::time::timeout(std::time::Duration::from_secs(10), backend::news::fetch_arch_news()).await {
                Ok(Ok(news)) => { tx3.send(Action::SetNews(news)).ok(); }
                _ => {}
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
                    tokio::spawn(async move {
                        match backend::paru::Paru::get_pkgbuild(&pkg_name).await {
                            Ok(content) => {
                                let lines = backend::highlight::highlight_pkgbuild(&content);
                                tx_view.send(Action::SetPkgbuildLines(lines)).ok();
                            }
                            Err(e) => {
                                tx_view.send(Action::Error(format!("Failed to get PKGBUILD: {}", e))).ok();
                            }
                        }
                    });
                }
                Action::SetPkgbuildLines(lines) => {
                    app.is_loading = false;
                    app.pkgbuild_lines = lines;
                    app.pkgbuild_scroll = 0;
                    app.route = app::Route::DiffViewer;
                    app.status_message = Some("PKGBUILD loaded.".to_string());
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
                Action::UpdateSingle(pkg_name) => {
                    tx.send(Action::InstallPackages(vec![pkg_name])).ok();
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
                Action::SetSystemInfo(info) => {
                    app.system_info = info;
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
                        println!("\n>>> sudo pacman -Sc\n");
                        std::process::Command::new("sudo")
                            .arg("pacman")
                            .arg("-Sc")
                            .status()
                    } else {
                        println!("\n>>> sudo paccache -r\n");
                        std::process::Command::new("sudo")
                            .arg("paccache")
                            .arg("-r")
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
                                if app.pkgbuild_scroll + 1 < app.pkgbuild_lines.len() {
                                    app.pkgbuild_scroll += 1;
                                }
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                if app.pkgbuild_scroll > 0 {
                                    app.pkgbuild_scroll -= 1;
                                }
                            }
                            KeyCode::Esc | KeyCode::Char('q') => {
                                app.route = app::App::tab_route(app.tab_index);
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
                                    } else {
                                        app.select_previous();
                                    }
                                }
                                KeyCode::Char('h') | KeyCode::Left => {
                                    if app.route == app::Route::Store {
                                        app.store_active_pane = 0;
                                    } else if app.route == app::Route::Cache {
                                        app.cache_active_pane = 0;
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
                                    if (1..=8).contains(&digit) {
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
                                            tx.send(Action::Error("⚠️ Security scan only works for AUR packages (repo packages are pre-compiled & trusted).".to_string())).ok();
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
                                            tx.send(Action::Error("⚠️ PKGBUILD is only available for AUR packages.".to_string())).ok();
                                        } else {
                                            tx.send(Action::ViewPkgbuild(name)).ok();
                                        }
                                    }
                                }
                                // Install selected package(s) (Enter in Updates/Search/Store)
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
                                // Cache: delete selected / remove selected orphan package
                                KeyCode::Char('d') => {
                                    if app.route == app::Route::Cache {
                                        if app.cache_active_pane == 0 {
                                            if let Some(i) = app.list_state.selected() {
                                                if let Some(c) = app.cache_entries.get(i) {
                                                    let name = c.name.clone();
                                                    tx.send(Action::ShowConfirm(
                                                        format!("Delete cache for '{}'?", name),
                                                        Box::new(Action::CleanCache(name)),
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
                                            "Clean ALL unused pacman cache files?".to_string(),
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

async fn handle_cli(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let cmd = args[1].as_str();
    if cmd == "install" || cmd == "i" {
        if args.len() < 3 {
            println!("⚠️ Error: Please specify a package name, file path, or directory containing a PKGBUILD.");
            println!("Usage: aurum install <package | file | directory>");
            std::process::exit(1);
        }

        let targets = &args[2..];
        
        // If there's only one target, it might be a file or a PKGBUILD directory
        if targets.len() == 1 {
            let target = &targets[0];
            let is_path_like = target.contains('/') || target.contains('.') || target.starts_with('~');
            let expanded_path = expand_tilde(target);
            let path = std::path::Path::new(&expanded_path);
            
            if is_path_like {
                if !path.exists() {
                    println!("❌ Error: Path '{}' does not exist.", path.display());
                    std::process::exit(1);
                }
                
                if path.is_file() {
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    if ext == "zst" || ext == "xz" {
                        println!("📦 Detected local Arch package archive: {}", path.display());
                        println!(">>> sudo pacman -U {}", path.display());
                        let status = std::process::Command::new("sudo")
                            .args(["pacman", "-U", &path.to_string_lossy()])
                            .status()?;
                        if !status.success() {
                            std::process::exit(status.code().unwrap_or(1));
                        }
                        return Ok(());
                    } else {
                        println!("❌ Error: File '{}' is not a valid Arch package archive (.pkg.tar.zst or .pkg.tar.xz).", path.display());
                        std::process::exit(1);
                    }
                } else if path.is_dir() {
                    if path.join("PKGBUILD").exists() {
                        println!("🛠️ Detected PKGBUILD source directory: {}", path.display());
                        
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
                            println!("  aurum install {}", path.display());
                            println!("Aurum will request password escalation automatically during installation.");
                            std::process::exit(1);
                        }
                        
                        println!(">>> makepkg -si (inside {})", path.display());
                        let status = std::process::Command::new("makepkg")
                            .args(["-si"])
                            .current_dir(path)
                            .status()?;
                        if !status.success() {
                            std::process::exit(status.code().unwrap_or(1));
                        }
                        return Ok(());
                    } else {
                        println!("⚠️ Error: Directory '{}' does not contain a PKGBUILD file.", path.display());
                        std::process::exit(1);
                    }
                }
            }
        }

        // Default to paru installation for package names
        println!("🔍 Installing packages via paru: {}", targets.join(" "));
        println!(">>> paru -S {}", targets.join(" "));
        let status = std::process::Command::new("paru")
            .arg("-S")
            .args(targets)
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
        println!("🗑️ Removing packages via paru: {}", targets.join(" "));
        println!(">>> paru -Rns {}", targets.join(" "));
        let status = std::process::Command::new("paru")
            .arg("-Rns")
            .args(targets)
            .status()?;
            
        if !status.success() {
            std::process::exit(status.code().unwrap_or(1));
        }
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
