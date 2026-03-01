mod event;
mod ui;

use commits_of_interest_core::{
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
use std::{fmt::Write, fs, io, io::Write as IoWrite, path::Path};

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

    // +1 for the space after the label.
    let indent = pr_groups
        .iter()
        .map(|(label, _)| label.len() + 1)
        .max()
        .unwrap_or(0);

    let mut entries = Vec::new();
    for (label, commit_indices) in pr_groups {
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

fn format_proposed_changelog(
    entries: &[ListEntry],
    commits: &[CommitInfo],
    owner: &str,
    name: &str,
) -> String {
    let mut content = String::new();
    for entry in entries {
        if let ListEntry::Commit { commit_idx, .. } = entry {
            let commit = &commits[*commit_idx];
            let url = format!("https://github.com/{owner}/{name}/commit/{}", commit.oid);
            writeln!(content, "- {} [{}]({})", commit.message, commit.short_id, url).unwrap();
        }
    }
    content
}

#[cfg(test)]
mod tests {
    use super::*;
    use commits_of_interest_core::git::{CommitInfo, FileDiff};
    use std::path::PathBuf;

    #[test]
    fn format_proposed_changelog_basic() {
        let commits = vec![
            make_commit(
                "abc1234",
                "abc1234abc1234abc1234abc1234abc1234abc1234",
                "Fix the widget",
                Some(42),
            ),
            make_commit(
                "def5678",
                "def5678def5678def5678def5678def5678def5678",
                "Update tests",
                None,
            ),
        ];
        let entries = entries_from_commits(&commits);
        let content = format_proposed_changelog(&entries, &commits, "owner", "repo");
        assert_eq!(
            content,
            "\
- Fix the widget [abc1234](https://github.com/owner/repo/commit/abc1234abc1234abc1234abc1234abc1234abc1234)
- Update tests [def5678](https://github.com/owner/repo/commit/def5678def5678def5678def5678def5678def5678)
"
        );
    }

    #[test]
    fn entries_groups_by_pr() {
        let commits = vec![
            make_commit("aaa", "aaa", "first", Some(1)),
            make_commit("bbb", "bbb", "second", Some(2)),
            make_commit("ccc", "ccc", "third", Some(1)),
        ];
        let entries = entries_from_commits(&commits);

        // PR #1 group comes first (first appearance), then PR #2.
        // Commit 0, Commit 2, Commit 1.
        let commit_indices: Vec<usize> = entries
            .iter()
            .filter_map(|entry| match entry {
                ListEntry::Commit { commit_idx, .. } => Some(*commit_idx),
                _ => None,
            })
            .collect();
        assert_eq!(commit_indices, vec![0, 2, 1]);
    }

    #[test]
    fn entries_pr_label_on_first_commit_only() {
        let commits = vec![
            make_commit("aaa", "aaa", "first", Some(5)),
            make_commit("bbb", "bbb", "second", Some(5)),
        ];
        let entries = entries_from_commits(&commits);

        let labels: Vec<Option<&str>> = entries
            .iter()
            .filter_map(|entry| match entry {
                ListEntry::Commit { pr_label, .. } => {
                    Some(pr_label.as_deref())
                }
                _ => None,
            })
            .collect();
        assert_eq!(labels, vec![Some("#5"), None]);
    }

    #[test]
    fn entries_unknown_pr_uses_question_marks() {
        let commits = vec![make_commit("aaa", "aaa", "orphan", None)];
        let entries = entries_from_commits(&commits);

        let label = match &entries[0] {
            ListEntry::Commit { pr_label, .. } => pr_label.as_deref(),
            _ => panic!("expected Commit entry"),
        };
        assert_eq!(label, Some("??"));
    }

    #[test]
    fn entries_indent_is_global_maximum() {
        // "#1234" is 5 chars + 1 space = 6. "#1" is 2 chars + 1 space = 3.
        // All entries should use the maximum indent of 6.
        let commits = vec![
            make_commit("aaa", "aaa", "first", Some(1234)),
            make_commit("bbb", "bbb", "second", Some(1)),
        ];
        let entries = entries_from_commits(&commits);

        let indents: Vec<usize> = entries
            .iter()
            .map(|entry| match entry {
                ListEntry::Commit { indent, .. } | ListEntry::Path { indent, .. } => *indent,
            })
            .collect();
        assert!(indents.iter().all(|&indent| indent == 6));
    }

    #[test]
    fn entries_interleaves_paths() {
        let commits = vec![make_commit_with_files(
            "aaa",
            "aaa",
            "msg",
            Some(1),
            &["src/lib.rs", "src/main.rs"],
        )];
        let entries = entries_from_commits(&commits);

        // Should be: Commit, Path(0), Path(1).
        assert_eq!(entries.len(), 3);
        assert!(matches!(entries[0], ListEntry::Commit { .. }));
        assert!(matches!(
            entries[1],
            ListEntry::Path {
                file_idx: 0,
                ..
            }
        ));
        assert!(matches!(
            entries[2],
            ListEntry::Path {
                file_idx: 1,
                ..
            }
        ));
    }

    #[test]
    fn first_entry_finds_first_path() {
        let commits = vec![make_commit_with_files(
            "aaa",
            "aaa",
            "msg",
            Some(1),
            &["src/lib.rs"],
        )];
        let entries = entries_from_commits(&commits);

        // Entry 0 is a Commit, entry 1 is the first Path.
        assert_eq!(first_entry(&entries), Some(1));
    }

    #[test]
    fn first_entry_returns_none_when_no_paths() {
        let commits = vec![make_commit("aaa", "aaa", "msg", Some(1))];
        let entries = entries_from_commits(&commits);

        assert_eq!(first_entry(&entries), None);
    }

    fn make_commit(short_id: &str, oid: &str, message: &str, pr: Option<u64>) -> CommitInfo {
        CommitInfo {
            short_id: short_id.to_owned(),
            oid: oid.to_owned(),
            message: message.to_owned(),
            pr,
            file_diffs: Vec::new(),
        }
    }

    fn make_commit_with_files(
        short_id: &str,
        oid: &str,
        message: &str,
        pr: Option<u64>,
        paths: &[&str],
    ) -> CommitInfo {
        CommitInfo {
            short_id: short_id.to_owned(),
            oid: oid.to_owned(),
            message: message.to_owned(),
            pr,
            file_diffs: paths
                .iter()
                .map(|path| FileDiff {
                    path: PathBuf::from(path),
                    lines: Vec::new(),
                })
                .collect(),
        }
    }
}
