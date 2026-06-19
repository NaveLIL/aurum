use std::sync::OnceLock;
use syntect::parsing::SyntaxSet;
use syntect::highlighting::{ThemeSet, Style as SyntectStyle};
use syntect::easy::HighlightLines;
use ratatui::style::{Color, Style as RatatuiStyle, Modifier};
use ratatui::text::{Span, Line};

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();

pub fn highlight_pkgbuild(content: &str) -> Vec<Line<'static>> {
    let ps = SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines);
    let ts = THEME_SET.get_or_init(ThemeSet::load_defaults);

    let syntax = ps.find_syntax_by_extension("sh")
        .unwrap_or_else(|| ps.find_syntax_plain_text());

    let theme = &ts.themes["base16-ocean.dark"];

    let mut h = HighlightLines::new(syntax, theme);
    let mut lines = Vec::new();

    for (line_idx, line) in content.lines().enumerate() {
        // syntect HighlightLines expects newline terminated strings
        let line_with_nl = format!("{}\n", line);
        let ranges: Vec<(SyntectStyle, &str)> = match h.highlight_line(&line_with_nl, ps) {
            Ok(r) => r,
            Err(_) => vec![(SyntectStyle::default(), line)],
        };

        let mut spans = Vec::new();
        // Prepend line number span
        let line_num_str = format!("{:>3} │ ", line_idx + 1);
        spans.push(Span::styled(line_num_str, RatatuiStyle::default().fg(Color::Rgb(100, 100, 120))));

        for (style, text) in ranges {
            // Trim trailing newline for rendering spans properly
            let text_trimmed = text.trim_end_matches('\n');
            if text_trimmed.is_empty() {
                continue;
            }

            let fg_color = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
            let mut ratatui_style = RatatuiStyle::default().fg(fg_color);

            // Handle basic font styles
            let font_style = style.font_style;
            if font_style.contains(syntect::highlighting::FontStyle::BOLD) {
                ratatui_style = ratatui_style.add_modifier(Modifier::BOLD);
            }
            if font_style.contains(syntect::highlighting::FontStyle::ITALIC) {
                ratatui_style = ratatui_style.add_modifier(Modifier::ITALIC);
            }
            if font_style.contains(syntect::highlighting::FontStyle::UNDERLINE) {
                ratatui_style = ratatui_style.add_modifier(Modifier::UNDERLINED);
            }

            spans.push(Span::styled(text_trimmed.to_string(), ratatui_style));
        }

        lines.push(Line::from(spans));
    }

    lines
}

pub fn highlight_diff(content: &str) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for line in content.lines() {
        let mut spans = Vec::new();
        let mut style = RatatuiStyle::default();

        if line.starts_with('+') && !line.starts_with("+++") {
            style = style.fg(Color::Rgb(100, 220, 100)); // green -> success_fg
        } else if line.starts_with('-') && !line.starts_with("---") {
            style = style.fg(Color::Rgb(255, 100, 100)); // red -> error_fg
        } else if line.starts_with("@@") {
            style = style.fg(Color::Rgb(100, 160, 255)).add_modifier(Modifier::BOLD); // blue/cyan -> accent_fg
        } else if line.starts_with("diff") || line.starts_with("index") || line.starts_with("---") || line.starts_with("+++") {
            style = style.fg(Color::Rgb(255, 200, 80)).add_modifier(Modifier::BOLD); // yellow/orange -> header_fg
        } else {
            style = style.fg(Color::Rgb(200, 200, 220)); // normal text -> text_fg
        }

        spans.push(Span::styled(line.to_string(), style));
        lines.push(Line::from(spans));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_diff() {
        let diff_content = "diff --git a/PKGBUILD b/PKGBUILD\n\
                            --- a/PKGBUILD\n\
                            +++ b/PKGBUILD\n\
                            @@ -1,4 +1,4 @@\n\
                            -pkgver=1.0.0\n\
                            +pkgver=1.1.0\n\
                             pkgrel=1";

        let lines = highlight_diff(diff_content);
        assert_eq!(lines.len(), 7);

        let diff_span = &lines[0].spans[0];
        assert_eq!(diff_span.content, "diff --git a/PKGBUILD b/PKGBUILD");
        assert_eq!(diff_span.style.fg, Some(Color::Rgb(255, 200, 80)));

        let hunk_span = &lines[3].spans[0];
        assert_eq!(hunk_span.content, "@@ -1,4 +1,4 @@");
        assert_eq!(hunk_span.style.fg, Some(Color::Rgb(100, 160, 255)));

        let del_span = &lines[4].spans[0];
        assert_eq!(del_span.content, "-pkgver=1.0.0");
        assert_eq!(del_span.style.fg, Some(Color::Rgb(255, 100, 100)));

        let add_span = &lines[5].spans[0];
        assert_eq!(add_span.content, "+pkgver=1.1.0");
        assert_eq!(add_span.style.fg, Some(Color::Rgb(100, 220, 100)));

        let normal_span = &lines[6].spans[0];
        assert_eq!(normal_span.content, "pkgrel=1");
        assert_eq!(normal_span.style.fg, Some(Color::Rgb(200, 200, 220)));
    }
}


