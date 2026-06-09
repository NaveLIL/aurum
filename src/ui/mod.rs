use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Line},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Tabs, Wrap, ListState},
    Frame,
};
use crate::app::{App, Route, InputMode, InstalledSource, SearchSource};
use crate::backend::paru::format_size;

pub fn draw(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Main Content
            Constraint::Length(3), // Footer
        ])
        .split(f.size());

    draw_header(f, app, chunks[0]);
    draw_main(f, app, chunks[1]);
    draw_footer(f, app, chunks[2]);

    // Draw confirm dialog on top if present
    if app.confirm_dialog.is_some() {
        draw_confirm_dialog(f, app);
    }

    // Draw help modal on top if present
    if app.show_help {
        draw_help_modal(f, app);
    }
}

fn draw_header(f: &mut Frame, app: &mut App, area: Rect) {
    let tab_names = ["Dashboard", "Updates", "Installed", "Search", "Store", "News", "Cache", "Scanner"];
    let titles: Vec<Line> = tab_names
        .iter()
        .map(|t| {
            let (first, rest) = t.split_at(1);
            Line::from(vec![
                Span::styled(first, Style::default().fg(Color::Rgb(255, 200, 50))),
                Span::styled(rest, Style::default().fg(Color::Rgb(100, 220, 100))),
            ])
        })
        .collect();

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(
                    " 📦 Aurum ",
                    Style::default().fg(Color::Rgb(80, 180, 255)).add_modifier(Modifier::BOLD),
                )),
        )
        .select(app.tab_index)
        .style(Style::default().fg(Color::Rgb(120, 120, 140)))
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Rgb(255, 220, 80)),
        )
        .divider(Span::styled(" │ ", Style::default().fg(Color::Rgb(60, 60, 80))));

    f.render_widget(tabs, area);
}

fn draw_main(f: &mut Frame, app: &mut App, area: Rect) {
    match app.route {
        Route::Dashboard => draw_dashboard(f, app, area),
        Route::Updates => draw_updates(f, app, area),
        Route::Installed => draw_installed(f, app, area),
        Route::Search => draw_search(f, app, area),
        Route::Store => draw_store(f, app, area),
        Route::News => draw_news(f, app, area),
        Route::Cache => draw_cache(f, app, area),
        Route::Scanner => draw_scanner(f, app, area),
        Route::DiffViewer => draw_pkgbuild_viewer(f, app, area),
        _ => draw_placeholder(f, area),
    }
}
fn draw_dashboard(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Left: Status & Recent News
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(chunks[0]);

    let status_block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" ⚡ Status ", Style::default().fg(Color::Rgb(80, 180, 255))));

    let updates_count = app.updates.len();
    let installed_count = app.installed_packages.len();
    let last_check = app.last_checked.as_deref().unwrap_or("Never");
    let news_count = app.news_items.len();
    let cache_total: u64 = app.cache_entries.iter().map(|c| c.size_bytes).sum();

    let loading_indicator = if app.is_loading {
        format!(" {} Loading...", app.spinner_char())
    } else {
        String::new()
    };

    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("  Updates Available: "),
            Span::styled(
                updates_count.to_string(),
                Style::default()
                    .fg(if updates_count > 0 { Color::Rgb(255, 100, 100) } else { Color::Rgb(100, 220, 100) })
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Installed (AUR):   "),
            Span::styled(installed_count.to_string(), Style::default().fg(Color::Rgb(100, 160, 255))),
        ]),
        Line::from(vec![
            Span::raw("  News Items:        "),
            Span::styled(news_count.to_string(), Style::default().fg(Color::Rgb(200, 180, 255))),
        ]),
        Line::from(vec![
            Span::raw("  Cache Size:        "),
            Span::styled(format_size(cache_total), Style::default().fg(Color::Rgb(255, 200, 100))),
        ]),
        Line::from(vec![
            Span::raw("  Last Checked:      "),
            Span::styled(last_check, Style::default().fg(Color::Rgb(140, 140, 160))),
        ]),
        Line::from(Span::styled(loading_indicator, Style::default().fg(Color::Rgb(255, 220, 80)))),
    ];

    let paragraph = Paragraph::new(text).block(status_block);
    f.render_widget(paragraph, left_chunks[0]);

    // Recent news preview
    let news_block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" 📰 Latest News ", Style::default().fg(Color::Rgb(200, 180, 255))));

    let news_text: Vec<Line> = if app.news_items.is_empty() {
        vec![Line::from("  Loading news...")]
    } else {
        app.news_items.iter().take(3).map(|n| {
            Line::from(vec![
                Span::styled("  • ", Style::default().fg(Color::Rgb(255, 200, 100))),
                Span::styled(n.title.chars().take(50).collect::<String>(), Style::default().fg(Color::Rgb(200, 200, 220))),
            ])
        }).collect()
    };

    let news_para = Paragraph::new(news_text).block(news_block);
    f.render_widget(news_para, left_chunks[1]);

    // Right: System Health (Top) & Keyboard Quick Help (Bottom)
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(11), // System Health & Upgrade Alert
            Constraint::Min(0),     // Keyboard Help
        ])
        .split(chunks[1]);

    let health_block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" 🩺 System Health ", Style::default().fg(Color::Rgb(100, 220, 100))));

    let mut health_lines = Vec::new();
    health_lines.push(Line::from(""));

    let days = app.system_info.last_upgrade_days;
    let age_style = if days > 14 {
        Style::default().fg(Color::Rgb(255, 80, 80)).add_modifier(Modifier::BOLD)
    } else if days > 7 {
        Style::default().fg(Color::Rgb(255, 200, 80))
    } else {
        Style::default().fg(Color::Rgb(100, 220, 100))
    };

    health_lines.push(Line::from(vec![
        Span::raw("  Last Upgrade:  "),
        Span::styled(
            if days == 0 { "Today".to_string() } else { format!("{} days ago", days) },
            age_style,
        ),
    ]));

    if app.system_info.pacman_lock_exists {
        health_lines.push(Line::from(vec![
            Span::styled("  ⚠️  Database Lock Active!", Style::default().fg(Color::Rgb(255, 100, 100)).add_modifier(Modifier::BOLD)),
        ]));
    } else {
        health_lines.push(Line::from(vec![
            Span::raw("  Database Lock: "),
            Span::styled("None", Style::default().fg(Color::Rgb(100, 220, 100))),
        ]));
    }

    health_lines.push(Line::from(vec![
        Span::raw("  Btrfs Snapper: "),
        Span::styled(
            if app.system_info.snapper_available { "Configured & Active (root)" } else { "Not Available" },
            Style::default().fg(if app.system_info.snapper_available { Color::Rgb(100, 220, 100) } else { Color::Rgb(140, 140, 160) }),
        ),
    ]));

    let has_lts = app.system_info.lts_kernel_installed;
    let multiple_kernels = app.system_info.multiple_kernels_installed;
    health_lines.push(Line::from(vec![
        Span::raw("  Backup Kernel: "),
        if has_lts {
            Span::styled("Configured (LTS)", Style::default().fg(Color::Rgb(100, 220, 100)))
        } else if multiple_kernels {
            Span::styled("Configured (Multiple)", Style::default().fg(Color::Rgb(255, 200, 80)))
        } else {
            Span::styled("⚠️ None (Press Shift-B to fix)", Style::default().fg(Color::Rgb(255, 80, 80)).add_modifier(Modifier::BOLD))
        }
    ]));

    let pacman_cache = app.disk_stats.pacman_cache_bytes;
    let cache_limit = 5 * 1024 * 1024 * 1024; // 5 GB
    health_lines.push(Line::from(vec![
        Span::raw("  Pacman Cache:  "),
        if pacman_cache > cache_limit {
            Span::styled(format!("⚠️ {} (Large)", format_size(pacman_cache)), Style::default().fg(Color::Rgb(255, 120, 50)).add_modifier(Modifier::BOLD))
        } else {
            Span::styled(format_size(pacman_cache), Style::default().fg(Color::Rgb(100, 220, 100)))
        }
    ]));

    let free_space = app.disk_stats.root_free_bytes;
    let space_limit = 10 * 1024 * 1024 * 1024; // 10 GB
    if free_space > 0 {
        health_lines.push(Line::from(vec![
            Span::raw("  Disk Free:     "),
            if free_space < space_limit {
                Span::styled(format!("⚠️ {} (Low!)", format_size(free_space)), Style::default().fg(Color::Rgb(255, 80, 80)).add_modifier(Modifier::BOLD))
            } else {
                Span::styled(format_size(free_space), Style::default().fg(Color::Rgb(100, 220, 100)))
            }
        ]));
    }

    let health_para = Paragraph::new(health_lines).block(health_block);
    f.render_widget(health_para, right_chunks[0]);

    let help_block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" ⌨ Quick Help ", Style::default().fg(Color::Rgb(80, 180, 255))));

    let help_text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Tab / [/] ", Style::default().fg(Color::Rgb(255, 220, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("Switch tabs  "),
            Span::styled("? ", Style::default().fg(Color::Rgb(255, 220, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("All Keybindings"),
        ]),
        Line::from(vec![
            Span::styled("  j/k / ↑/↓ ", Style::default().fg(Color::Rgb(255, 220, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("Navigate  "),
            Span::styled("Enter ", Style::default().fg(Color::Rgb(255, 220, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("Install/Details"),
        ]),
        Line::from(vec![
            Span::styled("  u / U     ", Style::default().fg(Color::Rgb(255, 220, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("Upgrade selected / Full upgrade"),
        ]),
        Line::from(vec![
            Span::styled("  /         ", Style::default().fg(Color::Rgb(255, 220, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("Search  "),
            Span::styled("t ", Style::default().fg(Color::Rgb(255, 220, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("Toggle AUR/Flatpak"),
        ]),
        Line::from(vec![
            Span::styled("  K         ", Style::default().fg(Color::Rgb(255, 100, 100)).add_modifier(Modifier::BOLD)),
            Span::raw("Fix Keyring errors"),
        ]),
        Line::from(vec![
            Span::styled("  L         ", Style::default().fg(Color::Rgb(255, 100, 100)).add_modifier(Modifier::BOLD)),
            Span::raw("Unlock pacman database"),
        ]),
        Line::from(vec![
            Span::styled("  M         ", Style::default().fg(Color::Rgb(255, 100, 100)).add_modifier(Modifier::BOLD)),
            Span::raw("Update mirrors (Reflector)"),
        ]),
        Line::from(vec![
            Span::styled("  B         ", Style::default().fg(Color::Rgb(255, 100, 100)).add_modifier(Modifier::BOLD)),
            Span::raw("Install backup LTS kernel"),
        ]),
    ];

    let help_para = Paragraph::new(help_text).block(help_block);
    f.render_widget(help_para, right_chunks[1]);
}

fn draw_updates(f: &mut Frame, app: &mut App, area: Rect) {
    if app.updates.is_empty() {
        let paragraph = Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  ✅ ", Style::default().fg(Color::Rgb(100, 220, 100))),
                Span::raw("No updates available — system is up to date!"),
            ]),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(" Updates ", Style::default().fg(Color::Rgb(100, 220, 100)))),
        );
        f.render_widget(paragraph, area);
        return;
    }

    let items: Vec<ListItem> = app
        .updates
        .iter()
        .map(|u| {
            let is_selected = app.selected_packages.contains(&u.name);
            let select_marker = if is_selected {
                Span::styled(" [x] ", Style::default().fg(Color::Rgb(100, 220, 100)).add_modifier(Modifier::BOLD))
            } else {
                Span::styled(" [ ] ", Style::default().fg(Color::Rgb(140, 140, 160)))
            };

            let is_repo = u.repository == "repo";
            let repo_tag = if is_repo {
                Span::styled(" [repo] ", Style::default().fg(Color::Rgb(80, 180, 255)))
            } else {
                Span::styled(" [aur]  ", Style::default().fg(Color::Rgb(255, 200, 80)))
            };

            let name_style = if is_repo {
                Style::default().fg(Color::Rgb(100, 220, 255)).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Rgb(255, 200, 80)).add_modifier(Modifier::BOLD)
            };

            let content = vec![Line::from(vec![
                select_marker,
                repo_tag,
                Span::styled(format!("{:<25}", u.name), name_style),
                Span::styled(&u.old_version, Style::default().fg(Color::Rgb(180, 80, 80))),
                Span::styled(" → ", Style::default().fg(Color::Rgb(140, 140, 160))),
                Span::styled(&u.new_version, Style::default().fg(Color::Rgb(100, 220, 100))),
            ])];
            ListItem::new(content)
        })
        .collect();

    let selected_count = app.selected_packages.len();
    let title = if selected_count > 0 {
        format!(" Updates ({}) — Selected {} — [Space] Select  [Enter] Install Selected  [u] Update  [U] Update All  [s] Scan ", app.updates.len(), selected_count)
    } else {
        format!(" Updates ({}) — [Space] Select  [Enter] Install Selected  [u] Update  [U] Update All  [s] Scan ", app.updates.len())
    };
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(title, Style::default().fg(Color::Rgb(255, 160, 80)))),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(40, 45, 60))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");

    f.render_stateful_widget(list, area, &mut app.list_state);
}

fn draw_flatpak_missing_warning(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(255, 100, 100)))
        .title(Span::styled(" ⚠ Flatpak Not Installed ", Style::default().fg(Color::Rgb(255, 80, 80)).add_modifier(Modifier::BOLD)));
    
    let text = vec![
        Line::from(""),
        Line::from(Span::styled("Flatpak is not installed on this system.", Style::default().fg(Color::Rgb(255, 120, 120)).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from("To manage Flatpak and Flathub applications, you must install the utility first."),
        Line::from(""),
        Line::from(vec![
            Span::raw("Press "),
            Span::styled(" [F] ", Style::default().fg(Color::Rgb(255, 200, 80)).add_modifier(Modifier::BOLD)),
            Span::raw(" to install flatpak via paru."),
        ]),
        Line::from(""),
    ];

    let paragraph = Paragraph::new(text)
        .block(block)
        .alignment(ratatui::layout::Alignment::Center);

    f.render_widget(paragraph, area);
}

fn draw_installed(f: &mut Frame, app: &mut App, area: Rect) {
    // Check if flatpak source is selected but flatpak is not available
    if app.installed_source == InstalledSource::Flatpak && !app.flatpak_available {
        draw_flatpak_missing_warning(f, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // List
            Constraint::Length(1), // Source indicator / helper
        ])
        .split(area);

    let (items, title_text) = match app.installed_source {
        InstalledSource::System => {
            if app.installed_packages.is_empty() {
                let msg = if app.is_loading {
                    format!("  {} Loading packages...", app.spinner_char())
                } else {
                    "  No AUR packages found.".to_string()
                };
                let paragraph = Paragraph::new(msg)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(Span::styled(" Installed Packages (System) ", Style::default().fg(Color::Rgb(100, 160, 255)))),
                    )
                    .style(Style::default().fg(Color::Rgb(140, 140, 160)));
                f.render_widget(paragraph, chunks[0]);
                
                // Draw source footer helper
                let helper = Paragraph::new(Span::styled(" [t] Switch to Flatpak Mode ", Style::default().fg(Color::Rgb(120, 120, 140))));
                f.render_widget(helper, chunks[1]);
                return;
            }

            let list_items: Vec<ListItem> = app.installed_packages.iter().map(|p| {
                ListItem::new(vec![Line::from(vec![
                    Span::styled(format!("{:<30}", p.name), Style::default().fg(Color::Rgb(100, 180, 255)).add_modifier(Modifier::BOLD)),
                    Span::styled(&p.version, Style::default().fg(Color::Rgb(180, 180, 200))),
                ])])
            }).collect();
            
            (list_items, format!(" Installed System Packages ({}) — [Enter] Details  [s] Scan ", app.installed_packages.len()))
        }
        InstalledSource::Flatpak => {
            if app.installed_flatpaks.is_empty() {
                let msg = if app.is_loading {
                    format!("  {} Loading Flatpak apps...", app.spinner_char())
                } else {
                    "  No Flatpak applications found.".to_string()
                };
                let paragraph = Paragraph::new(msg)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(Span::styled(" Installed Applications (Flatpak) ", Style::default().fg(Color::Rgb(0, 180, 180)))),
                    )
                    .style(Style::default().fg(Color::Rgb(140, 140, 160)));
                f.render_widget(paragraph, chunks[0]);

                let helper = Paragraph::new(Span::styled(" [t] Switch to System Packages Mode ", Style::default().fg(Color::Rgb(120, 120, 140))));
                f.render_widget(helper, chunks[1]);
                return;
            }

            let list_items: Vec<ListItem> = app.installed_flatpaks.iter().map(|a| {
                ListItem::new(vec![
                    Line::from(vec![
                        Span::styled(format!("{:<30}", a.name), Style::default().fg(Color::Rgb(0, 200, 180)).add_modifier(Modifier::BOLD)),
                        Span::styled(format!(" v{}", a.version), Style::default().fg(Color::Rgb(180, 180, 200))),
                    ]),
                    Line::from(vec![
                        Span::raw("     "),
                        Span::styled(&a.app_id, Style::default().fg(Color::Rgb(120, 120, 140))),
                        Span::raw("  "),
                        Span::styled(format!("(branch: {})", a.branch), Style::default().fg(Color::Rgb(100, 100, 120))),
                    ]),
                ])
            }).collect();

            (list_items, format!(" Installed Flatpak Applications ({}) — [d] Uninstall App ", app.installed_flatpaks.len()))
        }
    };

    let title_color = match app.installed_source {
        InstalledSource::System => Color::Rgb(100, 160, 255),
        InstalledSource::Flatpak => Color::Rgb(0, 180, 180),
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(title_text, Style::default().fg(title_color))),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(40, 45, 60))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");

    f.render_stateful_widget(list, chunks[0], &mut app.list_state);

    // Render footer source switcher help
    let source_label = match app.installed_source {
        InstalledSource::System => "AUR/Pacman",
        InstalledSource::Flatpak => "Flatpak",
    };
    let helper_text = Line::from(vec![
        Span::styled(" [t] ", Style::default().fg(Color::Rgb(255, 200, 80)).add_modifier(Modifier::BOLD)),
        Span::raw(format!("Source: {} (Press t to toggle)", source_label)),
    ]);
    let helper = Paragraph::new(helper_text);
    f.render_widget(helper, chunks[1]);
}

fn draw_search(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Search Input
            Constraint::Min(0),    // Results
            Constraint::Length(1), // Source indicator
        ])
        .split(area);

    let input_style = match app.input_mode {
        InputMode::Normal => Style::default().fg(Color::Rgb(140, 140, 160)),
        InputMode::Editing => Style::default().fg(Color::Rgb(255, 220, 80)),
    };

    let source_title = match app.search_source {
        SearchSource::Aur => "Search AUR/Pacman",
        SearchSource::Flatpak => "Search Flathub (Flatpak)",
    };

    let search_text = app.search_input.value();
    let input = Paragraph::new(search_text).style(input_style).block(
        Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(
                format!(" 🔍 {} (/ to edit, Enter to search, Esc to cancel) ", source_title),
                Style::default().fg(Color::Rgb(200, 180, 255)),
            )),
    );
    f.render_widget(input, chunks[0]);

    // Set cursor for input mode
    if app.input_mode == InputMode::Editing {
        f.set_cursor(
            chunks[0].x + app.search_input.visual_cursor() as u16 + 1,
            chunks[0].y + 1,
        );
    }

    // Check if flatpak source is selected but flatpak is not available
    if app.search_source == SearchSource::Flatpak && !app.flatpak_available {
        draw_flatpak_missing_warning(f, chunks[1]);
        return;
    }

    let is_results_empty = match app.search_source {
        SearchSource::Aur => app.search_results.is_empty(),
        SearchSource::Flatpak => app.flatpak_search_results.is_empty(),
    };

    if is_results_empty {
        let msg = if app.is_loading {
            format!("  {} Searching...", app.spinner_char())
        } else {
            "  No results. Type a query and press Enter.".to_string()
        };
        let paragraph = Paragraph::new(msg)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(Span::styled(" Results ", Style::default().fg(Color::Rgb(140, 140, 160)))),
            )
            .style(Style::default().fg(Color::Rgb(140, 140, 160)));
        f.render_widget(paragraph, chunks[1]);
    } else {
        let (items, title_text, title_color) = match app.search_source {
            SearchSource::Aur => {
                let list_items: Vec<ListItem> = app.search_results.iter().map(|p| {
                    let is_selected = app.selected_packages.contains(&p.name);
                    let select_marker = if is_selected {
                        Span::styled(" [x] ", Style::default().fg(Color::Rgb(100, 220, 100)).add_modifier(Modifier::BOLD))
                    } else {
                        Span::styled(" [ ] ", Style::default().fg(Color::Rgb(140, 140, 160)))
                    };

                    let mut lines = vec![Line::from(vec![
                        select_marker,
                        Span::styled(format!("{:<25}", p.name), Style::default().fg(Color::Rgb(200, 130, 255)).add_modifier(Modifier::BOLD)),
                        Span::styled(format!("v{}", p.version), Style::default().fg(Color::Rgb(180, 180, 200))),
                        Span::raw("  "),
                        Span::styled(format!("⬆{}", p.votes), Style::default().fg(Color::Rgb(100, 220, 100))),
                        Span::raw("  "),
                        Span::styled(format!("★{:.2}", p.popularity), Style::default().fg(Color::Rgb(255, 200, 80))),
                    ])];
                    if let Some(ref desc) = p.description {
                        lines.push(Line::from(vec![
                            Span::styled(format!("       {}", desc.chars().take(80).collect::<String>()), Style::default().fg(Color::Rgb(140, 140, 160))),
                        ]));
                    }
                    ListItem::new(lines)
                }).collect();

                let selected_count = app.selected_packages.len();
                let title = if selected_count > 0 {
                    format!(" AUR/Pacman Results ({}) — Selected {} — [Space] Select  [Enter] Install Selected  [s] Scan ", app.search_results.len(), selected_count)
                } else {
                    format!(" AUR/Pacman Results ({}) — [Space] Select  [Enter] Install Selected  [s] Scan ", app.search_results.len())
                };

                (list_items, title, Color::Rgb(200, 130, 255))
            }
            SearchSource::Flatpak => {
                let list_items: Vec<ListItem> = app.flatpak_search_results.iter().map(|a| {
                    let mut lines = vec![
                        Line::from(vec![
                            Span::styled(format!("{:<25}", a.name), Style::default().fg(Color::Rgb(0, 200, 180)).add_modifier(Modifier::BOLD)),
                            Span::raw("  "),
                            Span::styled(&a.app_id, Style::default().fg(Color::Rgb(120, 120, 140))),
                        ])
                    ];
                    if let Some(ref summary) = a.summary {
                        lines.push(Line::from(vec![
                            Span::styled(format!("       {}", summary.chars().take(80).collect::<String>()), Style::default().fg(Color::Rgb(140, 140, 160))),
                        ]));
                    }
                    ListItem::new(lines)
                }).collect();

                (list_items, format!(" Flathub Results ({}) — [Enter] Install App ", app.flatpak_search_results.len()), Color::Rgb(0, 180, 180))
            }
        };

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(Span::styled(title_text, Style::default().fg(title_color))),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::Rgb(40, 45, 60))
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▸ ");

        f.render_stateful_widget(list, chunks[1], &mut app.list_state);
    }

    // Render footer source switcher help
    let source_label = match app.search_source {
        SearchSource::Aur => "AUR/Pacman",
        SearchSource::Flatpak => "Flathub (Flatpak)",
    };
    let helper_text = Line::from(vec![
        Span::styled(" [t] ", Style::default().fg(Color::Rgb(255, 200, 80)).add_modifier(Modifier::BOLD)),
        Span::raw(format!("Search Source: {} (Press t to toggle)", source_label)),
    ]);
    let helper = Paragraph::new(helper_text);
    f.render_widget(helper, chunks[2]);
}

fn draw_news(f: &mut Frame, app: &mut App, area: Rect) {
    if app.news_items.is_empty() {
        let msg = if app.is_loading {
            format!("  {} Loading Arch Linux news...", app.spinner_char())
        } else {
            "  No news loaded.".to_string()
        };
        let paragraph = Paragraph::new(msg)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(Span::styled(" 📰 Arch Linux News ", Style::default().fg(Color::Rgb(200, 180, 255)))),
            )
            .style(Style::default().fg(Color::Rgb(140, 140, 160)));
        f.render_widget(paragraph, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    // News list
    let items: Vec<ListItem> = app
        .news_items
        .iter()
        .map(|n| {
            let content = vec![Line::from(vec![
                Span::styled(
                    format!("{:<12}", n.pub_date.chars().take(16).collect::<String>()),
                    Style::default().fg(Color::Rgb(140, 140, 160)),
                ),
                Span::styled(
                    &n.title,
                    Style::default()
                        .fg(Color::Rgb(200, 200, 240))
                        .add_modifier(Modifier::BOLD),
                ),
            ])];
            ListItem::new(content)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(
                    " 📰 Arch Linux News ",
                    Style::default().fg(Color::Rgb(200, 180, 255)),
                )),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(40, 45, 60))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");

    f.render_stateful_widget(list, chunks[0], &mut app.list_state);

    // News detail (selected item)
    let detail_text = if let Some(idx) = app.list_state.selected() {
        if let Some(news) = app.news_items.get(idx) {
            vec![
                Line::from(vec![
                    Span::styled(&news.title, Style::default().fg(Color::Rgb(255, 220, 80)).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Date: ", Style::default().fg(Color::Rgb(140, 140, 160))),
                    Span::raw(&news.pub_date),
                ]),
                Line::from(vec![
                    Span::styled("Link: ", Style::default().fg(Color::Rgb(140, 140, 160))),
                    Span::styled(&news.link, Style::default().fg(Color::Rgb(100, 160, 255))),
                ]),
                Line::from(""),
                Line::from(Span::raw(&news.description)),
            ]
        } else {
            vec![Line::from("  Select a news item to read.")]
        }
    } else {
        vec![Line::from("  Select a news item to read.")]
    };

    let detail = Paragraph::new(detail_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(" Details ", Style::default().fg(Color::Rgb(140, 140, 160)))),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(detail, chunks[1]);
}
fn draw_cache(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Split Left Pane vertically
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8), // Disk usage stats & cleaning actions
            Constraint::Min(0),    // AUR Clone Cache list
        ])
        .split(chunks[0]);

    // System Disk Info
    let free_str = format_size(app.disk_stats.root_free_bytes);
    let total_str = format_size(app.disk_stats.root_total_bytes);
    let used_bytes = app.disk_stats.root_total_bytes.saturating_sub(app.disk_stats.root_free_bytes);
    let used_percent = if app.disk_stats.root_total_bytes > 0 {
        (used_bytes as f64 / app.disk_stats.root_total_bytes as f64 * 100.0) as u8
    } else {
        0
    };

    // Construct a text progress bar for disk usage
    let bar_width = 15;
    let filled = ((used_percent as f64 / 100.0) * bar_width as f64) as usize;
    let bar = format!(
        "[{}{}] {}%",
        "■".repeat(filled),
        " ".repeat(bar_width - filled),
        used_percent
    );

    let disk_text = vec![
        Line::from(vec![
            Span::styled("  Root Partition: ", Style::default().fg(Color::Rgb(140, 140, 160))),
            Span::styled(format!("{} free / {} total ", free_str, total_str), Style::default().fg(Color::Rgb(255, 255, 255))),
            Span::styled(bar, Style::default().fg(if used_percent > 85 { Color::Rgb(255, 100, 100) } else { Color::Rgb(80, 180, 255) })),
        ]),
        Line::from(vec![
            Span::styled("  Pacman Cache:   ", Style::default().fg(Color::Rgb(140, 140, 160))),
            Span::styled(format_size(app.disk_stats.pacman_cache_bytes), Style::default().fg(Color::Rgb(255, 200, 100)).add_modifier(Modifier::BOLD)),
            Span::styled("   Paru Cache: ", Style::default().fg(Color::Rgb(140, 140, 160))),
            Span::styled(format_size(app.disk_stats.paru_cache_bytes), Style::default().fg(Color::Rgb(255, 200, 100)).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Cleaners: ", Style::default().fg(Color::Rgb(140, 140, 160))),
            Span::styled(" [c] ", Style::default().fg(Color::Rgb(255, 200, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("paccache -r  "),
            Span::styled(" [C] ", Style::default().fg(Color::Rgb(255, 150, 50)).add_modifier(Modifier::BOLD)),
            Span::raw("pacman -Sc  "),
            Span::styled(" [f] ", Style::default().fg(Color::Rgb(200, 100, 255)).add_modifier(Modifier::BOLD)),
            Span::raw("flatpak unused"),
        ]),
    ];

    let disk_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(60, 60, 80)))
        .title(Span::styled(" 💾 System Disk Usage ", Style::default().fg(Color::Rgb(80, 180, 255))));

    let disk_para = Paragraph::new(disk_text).block(disk_block);
    f.render_widget(disk_para, left_chunks[0]);

    // Left Pane (lower): Cache Manager
    let cache_border_style = if app.cache_active_pane == 0 {
        Style::default().fg(Color::Rgb(255, 200, 80))
    } else {
        Style::default().fg(Color::Rgb(60, 60, 80))
    };

    if app.cache_entries.is_empty() {
        let msg = if app.is_loading {
            format!("  {} Scanning cache...", app.spinner_char())
        } else {
            "  Cache is empty or not found.".to_string()
        };
        let paragraph = Paragraph::new(msg)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(cache_border_style)
                    .title(Span::styled(" 🧹 Cache Manager ", Style::default().fg(Color::Rgb(255, 200, 100)))),
            )
            .style(Style::default().fg(Color::Rgb(140, 140, 160)));
        f.render_widget(paragraph, left_chunks[1]);
    } else {
        let total_size: u64 = app.cache_entries.iter().map(|c| c.size_bytes).sum();
        let items: Vec<ListItem> = app
            .cache_entries
            .iter()
            .map(|c| {
                let content = vec![Line::from(vec![
                    Span::styled(
                        format!("{:<25}", c.name),
                        Style::default()
                            .fg(Color::Rgb(255, 200, 100))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{:>10}", format_size(c.size_bytes)),
                        Style::default().fg(Color::Rgb(180, 180, 200)),
                    ),
                ])];
                ListItem::new(content)
            })
            .collect();

        let title = format!(
            " 🧹 Cache ({} entries, {}) ",
            app.cache_entries.len(),
            format_size(total_size)
        );
        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(cache_border_style)
                    .title(Span::styled(title, Style::default().fg(Color::Rgb(255, 200, 100)))),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::Rgb(40, 45, 60))
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▸ ");

        let mut list_state = app.list_state.clone();
        if app.cache_active_pane != 0 {
            list_state.select(None);
        }
        f.render_stateful_widget(list, left_chunks[1], &mut list_state);
    }

    // Right pane: Orphans cleaner
    let orphans_border_style = if app.cache_active_pane == 1 {
        Style::default().fg(Color::Rgb(255, 200, 80))
    } else {
        Style::default().fg(Color::Rgb(60, 60, 80))
    };

    let orphan_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(3)])
        .split(chunks[1]);

    if app.orphans.is_empty() {
        let msg = if app.is_loading {
            format!("  {} Checking for orphans...", app.spinner_char())
        } else {
            "  No orphan packages found. System is clean! ✨".to_string()
        };
        let paragraph = Paragraph::new(msg)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(orphans_border_style)
                    .title(Span::styled(" 🍂 Orphan Dependencies ", Style::default().fg(Color::Rgb(255, 100, 100)))),
            )
            .style(Style::default().fg(Color::Rgb(140, 140, 160)));
        f.render_widget(paragraph, orphan_chunks[0]);
    } else {
        let items: Vec<ListItem> = app
            .orphans
            .iter()
            .map(|pkg| {
                let content = vec![Line::from(vec![
                    Span::styled(
                        format!("{:<25}", pkg.name),
                        Style::default()
                            .fg(Color::Rgb(255, 100, 100))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{:<15}", pkg.version),
                        Style::default().fg(Color::Rgb(140, 140, 160)),
                    ),
                    Span::styled(
                        pkg.size.as_deref().unwrap_or(""),
                        Style::default().fg(Color::Rgb(255, 200, 100)),
                    ),
                ])];
                ListItem::new(content)
            })
            .collect();

        let title = format!(" 🍂 Orphans ({} packages) ", app.orphans.len());
        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(orphans_border_style)
                    .title(Span::styled(title, Style::default().fg(Color::Rgb(255, 100, 100)))),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::Rgb(55, 40, 40))
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▸ ");

        let mut orphans_state = app.orphans_list_state.clone();
        if app.cache_active_pane != 1 {
            orphans_state.select(None);
        }
        f.render_stateful_widget(list, orphan_chunks[0], &mut orphans_state);
    }
    // Help/actions bar for cache / orphans
    let help_text = if app.cache_active_pane == 0 {
        vec![
            Span::styled(" [j/k/↑/↓] ", Style::default().fg(Color::Rgb(255, 200, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("Navigate Cache  "),
            Span::styled(" [d] ", Style::default().fg(Color::Rgb(255, 200, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("Delete Selected  "),
            Span::styled(" [D] ", Style::default().fg(Color::Rgb(255, 200, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("Clean All Cache  "),
            Span::styled(" [l/→] ", Style::default().fg(Color::Rgb(255, 200, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("Switch to Orphans"),
        ]
    } else {
        vec![
            Span::styled(" [j/k/↑/↓] ", Style::default().fg(Color::Rgb(255, 200, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("Navigate Orphans  "),
            Span::styled(" [d] ", Style::default().fg(Color::Rgb(255, 200, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("Remove Selected  "),
            Span::styled(" [D] ", Style::default().fg(Color::Rgb(255, 200, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("Remove All Orphans  "),
            Span::styled(" [h/←] ", Style::default().fg(Color::Rgb(255, 200, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("Switch to Cache"),
        ]
    };

    let help_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(60, 60, 80)))
        .title(Span::styled(" Actions ", Style::default().fg(Color::Rgb(140, 140, 160))));
    let help_para = Paragraph::new(Line::from(help_text)).block(help_block);
    f.render_widget(help_para, orphan_chunks[1]);
}

fn draw_scanner(f: &mut Frame, app: &mut App, area: Rect) {
    if let Some(result) = app.scan_results.last() {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4), // Header with Score
                Constraint::Min(0),    // Vulnerabilities List
            ])
            .split(area);

        let risk_color = if result.score > 60 {
            Color::Rgb(255, 80, 80)
        } else if result.score > 30 {
            Color::Rgb(255, 200, 80)
        } else {
            Color::Rgb(100, 220, 100)
        };

        let header_text = vec![
            Line::from(vec![
                Span::raw("  Package: "),
                Span::styled(&result.package_name, Style::default().fg(Color::Rgb(100, 180, 255)).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::raw("  Risk Score: "),
                Span::styled(
                    format!("{}/100", result.score),
                    Style::default().fg(risk_color).add_modifier(Modifier::BOLD),
                ),
            ]),
        ];

        let header = Paragraph::new(header_text).block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(" 🛡 Security Scan ", Style::default().fg(risk_color))),
        );

        f.render_widget(header, chunks[0]);

        if result.vulnerabilities.is_empty() {
            let safe_msg = Paragraph::new(vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled("  ✅ ", Style::default().fg(Color::Rgb(100, 220, 100))),
                    Span::raw("No obvious threats detected."),
                ]),
            ])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(Span::styled(" Details ", Style::default().fg(Color::Rgb(100, 220, 100)))),
            );
            f.render_widget(safe_msg, chunks[1]);
        } else {
            let items: Vec<ListItem> = result
                .vulnerabilities
                .iter()
                .map(|v| {
                    let risk_color = v.risk_level.color();
                    let color = Color::Rgb(risk_color.0, risk_color.1, risk_color.2);

                    let content = vec![
                        Line::from(vec![Span::styled(
                            format!("[{:?}] {}", v.risk_level, v.check_name),
                            Style::default().fg(color).add_modifier(Modifier::BOLD),
                        )]),
                        Line::from(vec![Span::styled(
                            format!(
                                "  Line {}: {}",
                                v.line_number.unwrap_or(0),
                                v.line_content.clone().unwrap_or_default().trim()
                            ),
                            Style::default().fg(Color::Rgb(180, 180, 200)),
                        )]),
                    ];
                    ListItem::new(content)
                })
                .collect();

            let list = List::new(items).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(Span::styled(" ⚠ Detected Issues ", Style::default().fg(Color::Rgb(255, 160, 80)))),
            );
            f.render_widget(list, chunks[1]);
        }
    } else {
        let paragraph = Paragraph::new(vec![
            Line::from(""),
            Line::from("  No scan results."),
            Line::from("  Select a package in Updates/Installed/Search and press 's' to scan."),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(" 🛡 Security Scanner ", Style::default().fg(Color::Rgb(140, 140, 160)))),
        );
        f.render_widget(paragraph, area);
    }
}

fn draw_placeholder(f: &mut Frame, area: Rect) {
    let paragraph = Paragraph::new("  Not implemented yet.")
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(paragraph, area);
}

fn draw_footer(f: &mut Frame, app: &mut App, area: Rect) {
    let status = if app.is_loading {
        format!(
            "{} {}",
            app.spinner_char(),
            app.status_message.as_deref().unwrap_or("Loading...")
        )
    } else {
        app.status_message.as_deref().unwrap_or("Ready").to_string()
    };

    let route_name = match app.route {
        Route::Dashboard => "Dashboard",
        Route::Updates => "Updates",
        Route::Installed => "Installed",
        Route::Search => "Search",
        Route::Store => "Store",
        Route::News => "News",
        Route::Cache => "Cache",
        Route::Scanner => "Scanner",
        _ => "Unknown",
    };

    let team = format!("🔨 Built by {}", crate::types::TEAM_SIG);
    let left_len = route_name.len() + 3 + status.len() + 2;
    let padding_len = (area.width as usize).saturating_sub(left_len + team.len() + 4);
    let padding = " ".repeat(padding_len);

    let footer_line = Line::from(vec![
        Span::styled(
            format!(" {} ", route_name),
            Style::default()
                .fg(Color::Rgb(30, 30, 40))
                .bg(Color::Rgb(80, 180, 255))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(status, Style::default().fg(Color::Rgb(160, 180, 220))),
        Span::raw(padding),
        Span::styled(team, Style::default().fg(Color::Rgb(100, 110, 130))),
    ]);

    let paragraph = Paragraph::new(footer_line).block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default()),
    );
    f.render_widget(paragraph, area);
}

fn draw_confirm_dialog(f: &mut Frame, app: &mut App) {
    let area = f.size();
    // Center the dialog
    let dialog_width = 50u16.min(area.width.saturating_sub(4));
    let dialog_height = 7u16.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(dialog_width)) / 2;
    let y = (area.height.saturating_sub(dialog_height)) / 2;
    let dialog_area = Rect::new(x, y, dialog_width, dialog_height);

    f.render_widget(Clear, dialog_area);

    if let Some((ref msg, _)) = app.confirm_dialog {
        let text = vec![
            Line::from(""),
            Line::from(Span::styled(
                msg.as_str(),
                Style::default().fg(Color::Rgb(255, 220, 80)),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("  [y] ", Style::default().fg(Color::Rgb(100, 220, 100)).add_modifier(Modifier::BOLD)),
                Span::raw("Yes   "),
                Span::styled("[n] ", Style::default().fg(Color::Rgb(255, 100, 100)).add_modifier(Modifier::BOLD)),
                Span::raw("No"),
            ]),
        ];

        let dialog = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(Span::styled(
                        " ⚠ Confirm ",
                        Style::default()
                            .fg(Color::Rgb(255, 200, 80))
                            .add_modifier(Modifier::BOLD),
                    ))
                    .style(Style::default().bg(Color::Rgb(30, 30, 45))),
            )
            .style(Style::default().bg(Color::Rgb(30, 30, 45)));

        f.render_widget(dialog, dialog_area);
    }
}

fn draw_pkgbuild_viewer(f: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " 📝 PKGBUILD Viewer ([j/↓] Down  [k/↑] Up  [Esc] Return) ",
            Style::default().fg(Color::Rgb(255, 200, 80)).add_modifier(Modifier::BOLD),
        ))
        .style(Style::default());

    let lines: Vec<Line<'_>> = app.pkgbuild_lines.iter().map(|line| {
        let spans: Vec<Span<'_>> = line.iter().map(|span| {
            Span::styled(&*span.content, span.style)
        }).collect();
        Line::from(spans)
    }).collect();

    let paragraph = Paragraph::new(lines)
        .block(block)
        .scroll((app.pkgbuild_scroll.try_into().unwrap_or(u16::MAX), 0));

    f.render_widget(paragraph, area);
}

fn draw_store(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);

    let categories = crate::backend::store::get_categories();

    // Left pane: Categories
    let category_border_style = if app.store_active_pane == 0 {
        Style::default().fg(Color::Rgb(255, 200, 80))
    } else {
        Style::default().fg(Color::Rgb(60, 60, 80))
    };

    let category_items: Vec<ListItem> = categories
        .iter()
        .enumerate()
        .map(|(i, cat)| {
            let style = if i == app.store_category_index {
                Style::default()
                    .fg(Color::Rgb(255, 200, 80))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Rgb(180, 180, 200))
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("  {} ", if i == app.store_category_index { "→" } else { " " }), style),
                Span::styled(*cat, style),
            ]))
        })
        .collect();

    let category_block = Block::default()
        .borders(Borders::ALL)
        .border_style(category_border_style)
        .title(Span::styled(
            " 📂 Categories ",
            Style::default().fg(Color::Rgb(80, 180, 255)).add_modifier(Modifier::BOLD),
        ));

    let category_list = List::new(category_items)
        .block(category_block)
        .highlight_style(Style::default().bg(Color::Rgb(40, 40, 55)));

    let mut category_list_state = ListState::default();
    category_list_state.select(Some(app.store_category_index));
    f.render_stateful_widget(category_list, chunks[0], &mut category_list_state);

    // Right pane: Apps in current category
    let app_border_style = if app.store_active_pane == 1 {
        Style::default().fg(Color::Rgb(255, 200, 80))
    } else {
        Style::default().fg(Color::Rgb(60, 60, 80))
    };

    let current_cat = categories[app.store_category_index];
    let store_apps = crate::backend::store::get_apps_by_category(current_cat);

    let app_items: Vec<ListItem> = store_apps
        .iter()
        .enumerate()
        .map(|(i, store_app)| {
            let is_selected = app.selected_packages.contains(store_app.name);
            let select_marker = if is_selected {
                Span::styled(" [x] ", Style::default().fg(Color::Rgb(100, 220, 100)).add_modifier(Modifier::BOLD))
            } else {
                Span::styled(" [ ] ", Style::default().fg(Color::Rgb(140, 140, 160)))
            };

            let is_installed = app.installed_packages_set.contains(store_app.name);
            let status_span = if is_installed {
                Span::styled(" (Installed)", Style::default().fg(Color::Rgb(100, 220, 100)).add_modifier(Modifier::ITALIC))
            } else {
                Span::raw("")
            };

            let name_style = if i == app.store_app_index && app.store_active_pane == 1 {
                Style::default().fg(Color::Rgb(255, 220, 80)).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Rgb(80, 180, 255)).add_modifier(Modifier::BOLD)
            };

            let content = vec![
                Line::from(vec![
                    select_marker,
                    Span::styled(format!("{:<28}", store_app.name), name_style),
                    status_span,
                ]),
                Line::from(vec![
                    Span::raw("     "),
                    Span::styled(store_app.description, Style::default().fg(Color::Rgb(140, 140, 160))),
                ]),
                Line::from(""), // spacing
            ];
            ListItem::new(content)
        })
        .collect();

    let app_title = format!(" 📦 Applications in {} ", current_cat);
    let app_block = Block::default()
        .borders(Borders::ALL)
        .border_style(app_border_style)
        .title(Span::styled(
            app_title,
            Style::default().fg(Color::Rgb(80, 180, 255)).add_modifier(Modifier::BOLD),
        ));

    // Splitting the right pane into apps list and help/info bar
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(3)])
        .split(chunks[1]);

    let app_list = List::new(app_items)
        .block(app_block)
        .highlight_style(Style::default().bg(Color::Rgb(40, 40, 55)));

    let mut app_list_state = ListState::default();
    app_list_state.select(Some(app.store_app_index));
    f.render_stateful_widget(app_list, right_chunks[0], &mut app_list_state);

    // Help/actions bar
    let help_text = if app.store_active_pane == 0 {
        vec![
            Span::styled(" [j/k/↑/↓] ", Style::default().fg(Color::Rgb(255, 200, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("Navigate Categories  "),
            Span::styled(" [l/→] ", Style::default().fg(Color::Rgb(255, 200, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("Go to Applications List"),
        ]
    } else {
        vec![
            Span::styled(" [j/k/↑/↓] ", Style::default().fg(Color::Rgb(255, 200, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("Navigate Apps  "),
            Span::styled(" [Space] ", Style::default().fg(Color::Rgb(255, 200, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("Toggle Batch Selection  "),
            Span::styled(" [v] ", Style::default().fg(Color::Rgb(255, 200, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("View PKGBUILD  "),
            Span::styled(" [Enter] ", Style::default().fg(Color::Rgb(255, 200, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("Install Selected  "),
            Span::styled(" [h/←] ", Style::default().fg(Color::Rgb(255, 200, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("Back to Categories"),
        ]
    };

    let help_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(60, 60, 80)))
        .title(Span::styled(" Actions ", Style::default().fg(Color::Rgb(140, 140, 160))));
    let help_para = Paragraph::new(Line::from(help_text)).block(help_block);
    f.render_widget(help_para, right_chunks[1]);
}

fn draw_help_modal(f: &mut Frame, _app: &App) {
    let area = f.size();
    // Center the dialog
    let dialog_width = 76u16.min(area.width.saturating_sub(4));
    let dialog_height = 27u16.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(dialog_width)) / 2;
    let y = (area.height.saturating_sub(dialog_height)) / 2;
    let dialog_area = Rect::new(x, y, dialog_width, dialog_height);

    f.render_widget(Clear, dialog_area);

    let border_style = Style::default().fg(Color::Rgb(80, 180, 255));
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(Span::styled(
            " ⌨ Keyboard Shortcuts ",
            Style::default().fg(Color::Rgb(80, 180, 255)).add_modifier(Modifier::BOLD),
        ));

    // Split inside the modal into two horizontal halves (columns)
    let inner_area = block.inner(dialog_area);
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner_area);

    // Left Column: Navigation & Global
    let left_text = vec![
        Line::from(Span::styled("── Global & Navigation ────────────────", Style::default().fg(Color::Rgb(100, 100, 120)))),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Tab / ]       ", Style::default().fg(Color::Rgb(255, 200, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("Next Tab"),
        ]),
        Line::from(vec![
            Span::styled("  Shift-Tab / [ ", Style::default().fg(Color::Rgb(255, 200, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("Prev Tab"),
        ]),
        Line::from(vec![
            Span::styled("  1 - 8         ", Style::default().fg(Color::Rgb(255, 200, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("Switch to Tab 1-8"),
        ]),
        Line::from(vec![
            Span::styled("  j / k / ↑ / ↓ ", Style::default().fg(Color::Rgb(255, 200, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("Navigate List"),
        ]),
        Line::from(vec![
            Span::styled("  h / l / ← / → ", Style::default().fg(Color::Rgb(255, 200, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("Switch Panes"),
        ]),
        Line::from(vec![
            Span::styled("  Esc / q       ", Style::default().fg(Color::Rgb(255, 100, 100)).add_modifier(Modifier::BOLD)),
            Span::raw("Quit / Close"),
        ]),
        Line::from(vec![
            Span::styled("  ?             ", Style::default().fg(Color::Rgb(80, 180, 255)).add_modifier(Modifier::BOLD)),
            Span::raw("Toggle Help Menu"),
        ]),
    ];

    // Right Column: Package Operations & Caches
    let right_text = vec![
        Line::from(Span::styled("── Actions & Maintenance ─────────────", Style::default().fg(Color::Rgb(100, 100, 120)))),
        Line::from(""),
        Line::from(vec![
            Span::styled("  i             ", Style::default().fg(Color::Rgb(100, 220, 100)).add_modifier(Modifier::BOLD)),
            Span::raw("Install selected package"),
        ]),
        Line::from(vec![
            Span::styled("  d             ", Style::default().fg(Color::Rgb(255, 100, 100)).add_modifier(Modifier::BOLD)),
            Span::raw("Remove / Clean selected"),
        ]),
        Line::from(vec![
            Span::styled("  D             ", Style::default().fg(Color::Rgb(200, 50, 50)).add_modifier(Modifier::BOLD)),
            Span::raw("Remove / Clean ALL"),
        ]),
        Line::from(vec![
            Span::styled("  u / U         ", Style::default().fg(Color::Rgb(80, 180, 255)).add_modifier(Modifier::BOLD)),
            Span::raw("Update Selected / All"),
        ]),
        Line::from(vec![
            Span::styled("  c             ", Style::default().fg(Color::Rgb(255, 200, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("Paccache -r (keep 3 ver)"),
        ]),
        Line::from(vec![
            Span::styled("  C             ", Style::default().fg(Color::Rgb(255, 150, 50)).add_modifier(Modifier::BOLD)),
            Span::raw("Pacman -Sc (clean unused)"),
        ]),
        Line::from(vec![
            Span::styled("  f             ", Style::default().fg(Color::Rgb(200, 100, 255)).add_modifier(Modifier::BOLD)),
            Span::raw("Clean unused Flatpaks"),
        ]),
        Line::from(vec![
            Span::styled("  K             ", Style::default().fg(Color::Rgb(255, 100, 100)).add_modifier(Modifier::BOLD)),
            Span::raw("Fix Keyring errors"),
        ]),
        Line::from(vec![
            Span::styled("  L             ", Style::default().fg(Color::Rgb(255, 100, 100)).add_modifier(Modifier::BOLD)),
            Span::raw("Unlock pacman database"),
        ]),
        Line::from(vec![
            Span::styled("  R             ", Style::default().fg(Color::Rgb(255, 100, 100)).add_modifier(Modifier::BOLD)),
            Span::raw("Reset keys trust database"),
        ]),
        Line::from(vec![
            Span::styled("  M             ", Style::default().fg(Color::Rgb(255, 100, 100)).add_modifier(Modifier::BOLD)),
            Span::raw("Update mirrorlist (Reflector)"),
        ]),
        Line::from(vec![
            Span::styled("  B             ", Style::default().fg(Color::Rgb(255, 100, 100)).add_modifier(Modifier::BOLD)),
            Span::raw("Install backup LTS kernel"),
        ]),
    ];

    let left_para = Paragraph::new(left_text);
    let right_para = Paragraph::new(right_text);

    f.render_widget(block, dialog_area);
    f.render_widget(left_para, columns[0]);
    f.render_widget(right_para, columns[1]);
}
