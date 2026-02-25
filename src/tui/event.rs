use super::{App, Pane};
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle_key(key: KeyEvent, app: &mut App) {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
        KeyCode::Char('s') => {
            app.save_proposed_changelog = true;
            app.should_quit = true;
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
