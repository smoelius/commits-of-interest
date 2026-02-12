use super::{App, Pane};
use crate::git::DiffLine;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{
        Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState,
    },
};

#[cfg_attr(dylint_lib = "supplementary", allow(unnamed_constant))]
pub fn draw(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(frame.area());

    draw_commit_pane(frame, app, chunks[0]);
    draw_diff_pane(frame, app, chunks[1]);
}

fn draw_commit_pane(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app.items.iter().cloned().map(ListItem::new).collect();

    let border_type = if app.focus == Pane::Left {
        BorderType::Thick
    } else {
        BorderType::Plain
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(border_type),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

    let mut state = ListState::default();
    state.select(Some(app.selected));
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_diff_pane(frame: &mut Frame, app: &mut App, area: Rect) {
    let border_type = if app.focus == Pane::Right {
        BorderType::Thick
    } else {
        BorderType::Plain
    };

    let line_count = if let Some(file_diff) = app.selected_file_diff() {
        file_diff.lines.len()
    } else {
        let empty = Paragraph::new("No files found").block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(border_type),
        );
        frame.render_widget(empty, area);
        return;
    };

    let visible_height = area.height.saturating_sub(2) as usize;
    let max_scroll = line_count.saturating_sub(visible_height);
    app.diff_scroll = app.diff_scroll.min(max_scroll);

    let lines: Vec<Line> = app
        .selected_file_diff()
        .unwrap()
        .lines
        .iter()
        .map(colorize_diff_line)
        .collect();

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(border_type),
        )
        .scroll((app.diff_scroll as u16, 0));

    frame.render_widget(paragraph, area);

    let mut scrollbar_state = ScrollbarState::new(max_scroll).position(app.diff_scroll);
    frame.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight),
        area,
        &mut scrollbar_state,
    );
}

fn colorize_diff_line(dl: &DiffLine) -> Line<'_> {
    let style = match dl.origin {
        '+' => Style::default().fg(Color::Green),
        '-' => Style::default().fg(Color::Red),
        'H' => Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        'F' => Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
        _ => Style::default(),
    };

    Line::styled(&dl.content, style)
}
