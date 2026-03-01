use crate::git::CommitInfo;
use std::fmt::Write;

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

pub fn entries_from_commits(commits: &[CommitInfo]) -> Vec<ListEntry> {
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

pub fn first_entry(entries: &[ListEntry]) -> Option<usize> {
    entries
        .iter()
        .position(|e| matches!(e, ListEntry::Path { .. }))
}

pub fn format_proposed_changelog(
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
    use crate::git::{CommitInfo, FileDiff};
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
                ListEntry::Commit { pr_label, .. } => Some(pr_label.as_deref()),
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
