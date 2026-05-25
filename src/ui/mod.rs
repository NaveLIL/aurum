use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Line},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Tabs, Wrap},
    Frame,
};
use crate::app::{App, Route, InputMode};
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
}

fn draw_header(f: &mut Frame, app: &mut App, area: Rect) {
    let tab_names = vec!["Dashboard", "Updates", "Installed", "Search", "News", "Cache", "Scanner"];
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
        Route::News => draw_news(f, app, area),
        Route::Cache => draw_cache(f, app, area),
        Route::Scanner => draw_scanner(f, app, area),
        _ => draw_placeholder(f, area),
    }
}

fn draw_dashboard(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Left: Status
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(chunks[0]);

    let status_block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" ⚡ Status ", Style::default().fg(Color::Rgb(80, 180, 255))))
        .style(Style::default());

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

    // Right: Quick Help
    let help_block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" ⌨ Keybindings ", Style::default().fg(Color::Rgb(100, 220, 100))));

    let help_text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  j/↓  k/↑", Style::default().fg(Color::Rgb(255, 220, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("  Navigate list"),
        ]),
        Line::from(vec![
            Span::styled("  Tab  S-Tab", Style::default().fg(Color::Rgb(255, 220, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("  Switch tab"),
        ]),
        Line::from(vec![
            Span::styled("  Enter     ", Style::default().fg(Color::Rgb(255, 220, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("  Install / Details"),
        ]),
        Line::from(vec![
            Span::styled("  u         ", Style::default().fg(Color::Rgb(255, 220, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("  Update selected"),
        ]),
        Line::from(vec![
            Span::styled("  U         ", Style::default().fg(Color::Rgb(255, 220, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("  Update all"),
        ]),
        Line::from(vec![
            Span::styled("  s         ", Style::default().fg(Color::Rgb(255, 220, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("  Security scan"),
        ]),
        Line::from(vec![
            Span::styled("  /         ", Style::default().fg(Color::Rgb(255, 220, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("  Search AUR"),
        ]),
        Line::from(vec![
            Span::styled("  d / D     ", Style::default().fg(Color::Rgb(255, 220, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("  Delete cache (one / all)"),
        ]),
        Line::from(vec![
            Span::styled("  q         ", Style::default().fg(Color::Rgb(255, 220, 80)).add_modifier(Modifier::BOLD)),
            Span::raw("  Quit"),
        ]),
    ];

    let help_para = Paragraph::new(help_text).block(help_block);
    f.render_widget(help_para, chunks[1]);
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

            let content = vec![Line::from(vec![
                select_marker,
                Span::styled(
                    format!("{:<25}", u.name),
                    Style::default()
                        .fg(Color::Rgb(255, 200, 80))
                        .add_modifier(Modifier::BOLD),
                ),
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

fn draw_installed(f: &mut Frame, app: &mut App, area: Rect) {
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
                    .title(Span::styled(" Installed ", Style::default().fg(Color::Rgb(100, 160, 255)))),
            )
            .style(Style::default().fg(Color::Rgb(140, 140, 160)));
        f.render_widget(paragraph, area);
        return;
    }

    let items: Vec<ListItem> = app
        .installed_packages
        .iter()
        .map(|p| {
            let content = vec![Line::from(vec![
                Span::styled(
                    format!("{:<30}", p.name),
                    Style::default()
                        .fg(Color::Rgb(100, 180, 255))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(&p.version, Style::default().fg(Color::Rgb(180, 180, 200))),
            ])];
            ListItem::new(content)
        })
        .collect();

    let title = format!(" Installed ({}) — [Enter] Details  [s] Scan ", app.installed_packages.len());
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(title, Style::default().fg(Color::Rgb(100, 160, 255)))),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(40, 45, 60))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");

    f.render_stateful_widget(list, area, &mut app.list_state);
}

fn draw_search(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Search Input
            Constraint::Min(0),    // Results
        ])
        .split(area);

    let input_style = match app.input_mode {
        InputMode::Normal => Style::default().fg(Color::Rgb(140, 140, 160)),
        InputMode::Editing => Style::default().fg(Color::Rgb(255, 220, 80)),
    };

    let search_text = app.search_input.value();
    let input = Paragraph::new(search_text).style(input_style).block(
        Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(
                " 🔍 Search AUR (/ to edit, Enter to search, Esc to cancel) ",
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

    if app.search_results.is_empty() {
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
        return;
    }

    let items: Vec<ListItem> = app
        .search_results
        .iter()
        .map(|p| {
            let is_selected = app.selected_packages.contains(&p.name);
            let select_marker = if is_selected {
                Span::styled(" [x] ", Style::default().fg(Color::Rgb(100, 220, 100)).add_modifier(Modifier::BOLD))
            } else {
                Span::styled(" [ ] ", Style::default().fg(Color::Rgb(140, 140, 160)))
            };

            let mut lines = vec![Line::from(vec![
                select_marker,
                Span::styled(
                    format!("{:<25}", p.name),
                    Style::default()
                        .fg(Color::Rgb(200, 130, 255))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("v{}", p.version), Style::default().fg(Color::Rgb(180, 180, 200))),
                Span::raw("  "),
                Span::styled(format!("⬆{}", p.votes), Style::default().fg(Color::Rgb(100, 220, 100))),
                Span::raw("  "),
                Span::styled(format!("★{:.2}", p.popularity), Style::default().fg(Color::Rgb(255, 200, 80))),
            ])];
            if let Some(ref desc) = p.description {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("       {}", desc.chars().take(80).collect::<String>()),
                        Style::default().fg(Color::Rgb(140, 140, 160)),
                    ),
                ]));
            }
            ListItem::new(lines)
        })
        .collect();

    let selected_count = app.selected_packages.len();
    let title = if selected_count > 0 {
        format!(" Results ({}) — Selected {} — [Space] Select  [Enter] Install Selected  [s] Scan ", app.search_results.len(), selected_count)
    } else {
        format!(" Results ({}) — [Space] Select  [Enter] Install Selected  [s] Scan ", app.search_results.len())
    };
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(title, Style::default().fg(Color::Rgb(200, 130, 255)))),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(40, 45, 60))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");

    f.render_stateful_widget(list, chunks[1], &mut app.list_state);
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
                    .title(Span::styled(" 🧹 Cache Manager ", Style::default().fg(Color::Rgb(255, 200, 100)))),
            )
            .style(Style::default().fg(Color::Rgb(140, 140, 160)));
        f.render_widget(paragraph, area);
        return;
    }

    let total_size: u64 = app.cache_entries.iter().map(|c| c.size_bytes).sum();

    let items: Vec<ListItem> = app
        .cache_entries
        .iter()
        .map(|c| {
            let content = vec![Line::from(vec![
                Span::styled(
                    format!("{:<30}", c.name),
                    Style::default()
                        .fg(Color::Rgb(255, 200, 100))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{:>10}", format_size(c.size_bytes)),
                    Style::default().fg(Color::Rgb(180, 180, 200)),
                ),
                Span::raw("  "),
                Span::styled(
                    &c.last_modified,
                    Style::default().fg(Color::Rgb(140, 140, 160)),
                ),
            ])];
            ListItem::new(content)
        })
        .collect();

    let title = format!(
        " 🧹 Cache ({} entries, {}) — [d] Delete  [D] Clean All ",
        app.cache_entries.len(),
        format_size(total_size)
    );
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(title, Style::default().fg(Color::Rgb(255, 200, 100)))),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(40, 45, 60))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");

    f.render_stateful_widget(list, area, &mut app.list_state);
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
        app.status_message.clone().unwrap_or_else(|| "Ready".to_string())
    };

    let route_name = match app.route {
        Route::Dashboard => "Dashboard",
        Route::Updates => "Updates",
        Route::Installed => "Installed",
        Route::Search => "Search",
        Route::News => "News",
        Route::Cache => "Cache",
        Route::Scanner => "Scanner",
        _ => "Unknown",
    };

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
