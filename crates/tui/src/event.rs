use super::{App, InputMode, Pane};
use commits_of_interest_core::entries::ListEntry;
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Margin, Position};

pub fn handle_key(app: &mut App, key: KeyEvent) {
    match app.input_mode {
        InputMode::Normal => handle_normal_key(app, key),
        InputMode::AddComponent => handle_input_key(app, key),
    }
}

fn handle_normal_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
        KeyCode::Char('s') => {
            app.save_proposed_changelog = true;
            app.should_quit = true;
        }
        KeyCode::Char('i') => app.input_mode = InputMode::AddComponent,
        KeyCode::Tab | KeyCode::BackTab => app.toggle_focus(),
        KeyCode::Left => app.focus = Pane::Left,
        KeyCode::Right => app.focus = Pane::Right,
        KeyCode::Up => match app.focus {
            Pane::Left => app.prev(),
            Pane::Right => app.scroll_diff_up(),
        },
        KeyCode::Down => match app.focus {
            Pane::Left => app.next(),
            Pane::Right => app.scroll_diff_down(),
        },
        _ => {}
    }
}

fn handle_input_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.input_buffer.clear();
        }
        KeyCode::Enter => app.submit_component(),
        KeyCode::Backspace => {
            app.input_buffer.pop();
        }
        KeyCode::Char(c) if c != '/' => {
            app.input_buffer.push(c);
        }
        _ => {}
    }
}

pub fn handle_mouse(app: &mut App, mouse: MouseEvent) {
    if app.input_mode != InputMode::Normal {
        return;
    }

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => handle_mouse_down(app, mouse),
        MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => handle_mouse_scroll(app, mouse),
        _ => {}
    }
}

fn handle_mouse_down(app: &mut App, mouse: MouseEvent) {
    let position = Position::new(mouse.column, mouse.row);
    let [left, right] = app.pane_areas;

    if right.contains(position) {
        app.focus = Pane::Right;
        return;
    }

    if !left.contains(position) {
        return;
    }

    app.focus = Pane::Left;

    let content = left.inner(Margin::new(1, 1));
    if !content.contains(position) {
        return;
    }

    let index = app.offset + usize::from(mouse.row - content.y);
    let selected = match app.entries.get(index) {
        Some(ListEntry::Path { .. }) => index,
        Some(ListEntry::Commit { commit_idx, .. }) => {
            // Commit headers select their first file, if one exists.
            match app.entries.get(index + 1) {
                Some(ListEntry::Path {
                    commit_idx: next, ..
                }) if next == commit_idx => index + 1,
                _ => index,
            }
        }
        None => return,
    };

    if app.selected != selected {
        app.selected = selected;
        app.diff_scroll = 0;
    }
}

fn handle_mouse_scroll(app: &mut App, mouse: MouseEvent) {
    let position = Position::new(mouse.column, mouse.row);
    let [left, right] = app.pane_areas;

    match mouse.kind {
        MouseEventKind::ScrollUp => {
            if left.contains(position) {
                app.prev();
            } else if right.contains(position) {
                app.scroll_diff_up();
            }
        }
        MouseEventKind::ScrollDown => {
            if left.contains(position) {
                app.next();
            } else if right.contains(position) {
                app.scroll_diff_down();
            }
        }
        _ => {}
    }
}
