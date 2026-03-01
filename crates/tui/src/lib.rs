mod event;
mod ui;

use commits_of_interest_core::{
    entries::{ListEntry, entries_from_commits, first_entry, format_proposed_changelog},
    git::{CommitInfo, FileDiff, collect_commits},
    github,
};
use anyhow::Result;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use git2::Repository;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    style::{Color, Style},
    text::{Line, Span},
};
use std::{fs, io, io::Write as IoWrite, path::Path};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Left,
    Right,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    AddComponent,
}

pub struct App {
    pub commits: Vec<CommitInfo>,
    pub entries: Vec<ListEntry>,
    pub items: Vec<Line<'static>>,
    pub focus: Pane,
    pub offset: usize,
    pub selected: usize,
    pub diff_scroll: usize,
    pub should_quit: bool,
    pub save_proposed_changelog: bool,
    pub input_mode: InputMode,
    pub input_buffer: String,
    pub revision: String,
}

impl App {
    fn new(commits: Vec<CommitInfo>, revision: String) -> Self {
        let entries = entries_from_commits(&commits);
        let items = build_items(&entries, &commits);
        let selected = first_entry(&entries).unwrap_or(0);
        Self {
            commits,
            entries,
            items,
            focus: Pane::Left,
            offset: 0,
            selected,
            diff_scroll: 0,
            should_quit: false,
            save_proposed_changelog: false,
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            revision,
        }
    }

    pub fn selected_file_diff(&self) -> Option<&FileDiff> {
        match self.entries.get(self.selected)? {
            ListEntry::Path {
                commit_idx,
                file_idx,
                ..
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
                // Ensure the commit header above this file is visible.
                if prev > 0 && matches!(self.entries[prev - 1], ListEntry::Commit { .. }) {
                    self.offset = self.offset.min(prev - 1);
                }
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

    pub fn submit_component(&mut self) {
        let component = self.input_buffer.trim().to_owned();
        if component.is_empty() {
            self.input_mode = InputMode::Normal;
            self.input_buffer.clear();
            return;
        }

        if let Ok(mut file) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(".filtered_components.txt")
        {
            let _ = writeln!(file, "{component}");
        }

        self.reload();
        self.input_mode = InputMode::Normal;
        self.input_buffer.clear();
    }

    fn reload(&mut self) {
        let Ok(repo) = Repository::open(".") else {
            return;
        };
        let Ok(mut commits) = collect_commits(&repo, &self.revision) else {
            return;
        };
        github::lookup_prs(&mut commits);

        self.entries = entries_from_commits(&commits);
        self.items = build_items(&self.entries, &commits);
        self.commits = commits;
        self.selected = first_entry(&self.entries).unwrap_or(0);
        self.offset = 0;
        self.diff_scroll = 0;
    }
}

fn build_items(entries: &[ListEntry], commits: &[CommitInfo]) -> Vec<Line<'static>> {
    entries
        .iter()
        .map(|entry| match entry {
            ListEntry::Commit {
                commit_idx,
                pr_label,
                indent,
            } => {
                let commit = &commits[*commit_idx];
                let mut spans = Vec::new();
                if let Some(label) = pr_label {
                    spans.push(Span::styled(
                        label.clone(),
                        Style::default().fg(Color::Cyan),
                    ));
                    spans.push(Span::raw(" "));
                } else {
                    spans.push(Span::raw(" ".repeat(*indent)));
                }
                spans.push(Span::styled(
                    commit.short_id.clone(),
                    Style::default().fg(Color::Yellow),
                ));
                spans.push(Span::raw(" "));
                spans.push(Span::raw(commit.message.clone()));
                Line::from(spans)
            }
            ListEntry::Path {
                commit_idx,
                file_idx,
                indent,
            } => {
                let path = &commits[*commit_idx].file_diffs[*file_idx].path;
                Line::from(vec![
                    Span::raw(" ".repeat(*indent)),
                    Span::raw("  "),
                    Span::raw(path.to_string_lossy().into_owned()),
                ])
            }
        })
        .collect()
}

pub fn run(commits: Vec<CommitInfo>, revision: &str) -> Result<()> {
    let mut stdout = io::stdout();

    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::new(commits, revision.to_owned());
    let result = run_loop(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    terminal.show_cursor()?;

    result?;

    if app.save_proposed_changelog {
        match write_proposed_changelog(&app) {
            Ok(()) => eprintln!("Changelog written to proposed_changelog.md"),
            Err(error) => eprintln!("Error writing changelog: {error}"),
        }
    }

    Ok(())
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

fn write_proposed_changelog(app: &App) -> Result<()> {
    use anyhow::bail;

    let path = Path::new("proposed_changelog.md");
    if path.exists() {
        bail!("proposed_changelog.md already exists; not overwriting");
    }

    let Some((owner, name)) = github::repo_owner_and_name() else {
        bail!("could not determine GitHub repository URL");
    };

    let content = format_proposed_changelog(&app.entries, &app.commits, &owner, &name);
    fs::write(path, content)?;
    Ok(())
}
