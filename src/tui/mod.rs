mod event;
mod ui;

use crate::{
    git::{CommitInfo, FileDiff},
    github,
};
use anyhow::Result;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    style::{Color, Style},
    text::{Line, Span},
};
use std::{fmt::Write, io, path::Path};

pub enum ListEntry {
    Commit {
        commit_idx: usize,
        pr_label: Option<String>,
        indent: usize,
    },
    Path {
        commit_idx: usize,
        file_idx: usize,
        indent: usize,
    },
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
    pub offset: usize,
    pub selected: usize,
    pub diff_scroll: usize,
    pub should_quit: bool,
    pub save_proposed_changelog: bool,
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
            offset: 0,
            selected,
            diff_scroll: 0,
            should_quit: false,
            save_proposed_changelog: false,
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
}

fn entries_from_commits(commits: &[CommitInfo]) -> Vec<ListEntry> {
    // Group commits by PR, preserving first-appearance order.
    let mut pr_groups: Vec<(String, Vec<usize>)> = Vec::new();
    for (commit_idx, commit) in commits.iter().enumerate() {
        let label = commit
            .pr
            .map(|n| format!("#{n}"))
            .unwrap_or_else(|| "??".to_owned());
        if let Some(group) = pr_groups.iter_mut().find(|(l, _)| *l == label) {
            group.1.push(commit_idx);
        } else {
            pr_groups.push((label, vec![commit_idx]));
        }
    }

    let mut entries = Vec::new();
    for (label, commit_indices) in pr_groups {
        // +1 for the space after the label.
        let indent = label.len() + 1;
        for (i, commit_idx) in commit_indices.into_iter().enumerate() {
            let pr_label = if i == 0 { Some(label.clone()) } else { None };
            entries.push(ListEntry::Commit {
                commit_idx,
                pr_label,
                indent,
            });
            for file_idx in 0..commits[commit_idx].file_diffs.len() {
                entries.push(ListEntry::Path {
                    commit_idx,
                    file_idx,
                    indent,
                });
            }
        }
    }
    entries
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

    let mut content = String::new();
    for entry in &app.entries {
        if let ListEntry::Commit { commit_idx, .. } = entry {
            let commit = &app.commits[*commit_idx];
            let url = format!("https://github.com/{owner}/{name}/commit/{}", commit.oid);
            writeln!(
                content,
                "- {} [{}]({})",
                commit.message, commit.short_id, url
            )?;
        }
    }

    std::fs::write(path, content)?;
    Ok(())
}
