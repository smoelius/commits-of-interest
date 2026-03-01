use super::{App, InputMode, Pane};
use crate::ui::POPUP_MIN_WIDTH;
use crossterm::{
    event::{KeyCode, KeyEvent},
    terminal::size as terminal_size,
};

pub fn handle_key(key: KeyEvent, app: &mut App) {
    match app.input_mode {
        InputMode::Normal => handle_normal_key(key, app),
        InputMode::AddComponent => handle_input_key(key, app),
    }
}

fn handle_normal_key(key: KeyEvent, app: &mut App) {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
        KeyCode::Char('s') => {
            app.save_proposed_changelog = true;
            app.should_quit = true;
        }
        KeyCode::Char('i') => {
            if let Ok((width, _)) = terminal_size()
                && width >= POPUP_MIN_WIDTH
            {
                app.input_mode = InputMode::AddComponent;
            }
        }
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

fn handle_input_key(key: KeyEvent, app: &mut App) {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.input_buffer.clear();
        }
        KeyCode::Enter => app.submit_component(),
        KeyCode::Backspace => {
            app.input_buffer.pop();
        }
        KeyCode::Char(c) if c != '/' && c != '.' => {
            app.input_buffer.push(c);
        }
        _ => {}
    }
}
