mod event;
mod ui;

use crate::git::{CommitInfo, FileDiff};
use anyhow::Result;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal, backend::CrosstermBackend,
    style::{Color, Style},
    text::{Line, Span},
};
use std::io;

pub enum ListEntry {
    Commit { short_id: String, message: String },
    Path { commit_idx: usize, file_idx: usize },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Left,
    Right,
}

pub struct App {
    pub commits: Vec<CommitInfo>,
    pub entries: Vec<ListEntry>,
    pub items: Vec<Line<'static>>,
    pub focus: Pane,
    pub selected: usize,
    pub diff_scroll: usize,
    pub should_quit: bool,
}

impl App {
    fn new(commits: Vec<CommitInfo>) -> Self {
        let entries = entries_from_commits(&commits);
        let items = build_items(&entries, &commits);
        let selected = first_entry(&entries).unwrap_or(0);
        Self {
            commits,
            entries,
            items,
            focus: Pane::Left,
            selected,
            diff_scroll: 0,
            should_quit: false,
        }
    }

    pub fn selected_file_diff(&self) -> Option<&FileDiff> {
        match self.entries.get(self.selected)? {
            ListEntry::Path {
                commit_idx,
                file_idx,
            } => Some(&self.commits[*commit_idx].file_diffs[*file_idx]),
            ListEntry::Commit { .. } => None,
        }
    }

    pub fn next(&mut self) {
        let mut next = self.selected + 1;
        while next < self.entries.len() {
            if matches!(self.entries[next], ListEntry::Path { .. }) {
                self.selected = next;
                self.diff_scroll = 0;
                return;
            }
            next += 1;
        }
    }

    pub fn prev(&mut self) {
        let mut prev = self.selected;
        while prev > 0 {
            prev -= 1;
            if matches!(self.entries[prev], ListEntry::Path { .. }) {
                self.selected = prev;
                self.diff_scroll = 0;
                return;
            }
        }
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            Pane::Left => Pane::Right,
            Pane::Right => Pane::Left,
        };
    }

    pub fn scroll_diff_down(&mut self) {
        self.diff_scroll = self.diff_scroll.saturating_add(1);
    }

    pub fn scroll_diff_up(&mut self) {
        self.diff_scroll = self.diff_scroll.saturating_sub(1);
    }
}

fn entries_from_commits(commits: &[CommitInfo]) -> Vec<ListEntry> {
    let mut entries = Vec::new();
    for (commit_idx, commit) in commits.iter().enumerate() {
        entries.push(ListEntry::Commit {
            short_id: commit.short_id.clone(),
            message: commit.message.clone(),
        });
        for (file_idx, _) in commit.file_diffs.iter().enumerate() {
            entries.push(ListEntry::Path {
                commit_idx,
                file_idx,
            });
        }
    }
    entries
}

fn build_items(entries: &[ListEntry], commits: &[CommitInfo]) -> Vec<Line<'static>> {
    entries
        .iter()
        .map(|entry| match entry {
            ListEntry::Commit { short_id, message } => Line::from(vec![
                Span::styled(short_id.clone(), Style::default().fg(Color::Yellow)),
                Span::raw(" "),
                Span::raw(message.clone()),
            ]),
            ListEntry::Path {
                commit_idx,
                file_idx,
            } => {
                let path = &commits[*commit_idx].file_diffs[*file_idx].path;
                Line::from(vec![
                    Span::raw("  "),
                    Span::raw(path.to_string_lossy().into_owned()),
                ])
            }
        })
        .collect()
}

fn first_entry(entries: &[ListEntry]) -> Option<usize> {
    entries
        .iter()
        .position(|e| matches!(e, ListEntry::Path { .. }))
}

pub fn run(commits: Vec<CommitInfo>) -> Result<()> {
    let mut stdout = io::stdout();

    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::new(commits);
    let result = run_loop(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    terminal.show_cursor()?;

    result
}

fn run_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|frame| ui::draw(frame, app))?;

        if let crossterm::event::Event::Key(key) = crossterm::event::read()?
            && key.kind == crossterm::event::KeyEventKind::Press
        {
            event::handle_key(key, app);
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}
