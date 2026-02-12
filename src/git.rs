use anyhow::Result;
use git2::{Commit, Diff, Oid, Patch, Repository, Sort};
use std::path::PathBuf;

pub const FILTERED_COMPONENTS: &[&str] = &[
    ".github",
    "CHANGELOG.md",
    "Cargo.toml",
    "Cargo.lock",
    "examples",
    "fixtures",
    "tests",
];

pub trait ShortId {
    fn short_id(&self) -> String;
}

impl ShortId for Commit<'_> {
    fn short_id(&self) -> String {
        self.id().short_id()
    }
}

impl ShortId for Oid {
    fn short_id(&self) -> String {
        let s = self.to_string();
        assert!(s.len() >= 7);
        s[..7].to_owned()
    }
}

pub struct CommitInfo {
    pub short_id: String,
    pub message: String,
    pub file_diffs: Vec<FileDiff>,
}

pub struct FileDiff {
    pub path: PathBuf,
    pub lines: Vec<DiffLine>,
}

pub struct DiffLine {
    pub origin: char,
    pub content: String,
}

pub fn collect_commits(repo: &Repository, revision: &str) -> Result<Vec<CommitInfo>> {
    let mut commits = Vec::new();

    let mut revwalk = repo.revwalk()?;
    revwalk.set_sorting(Sort::TOPOLOGICAL | Sort::REVERSE)?;

    let obj = repo.revparse_single(revision)?;
    revwalk.hide(obj.id())?;

    let head = repo.head()?;
    let head_commit = head.peel_to_commit()?;
    revwalk.push(head_commit.id())?;

    for result in revwalk {
        let oid = result?;
        let commit = repo.find_commit(oid)?;
        if let Some(info) = build_commit_info(repo, &commit)? {
            commits.push(info);
        }
    }

    Ok(commits)
}

fn build_commit_info(repo: &Repository, commit: &Commit) -> Result<Option<CommitInfo>> {
    let parent_tree = if commit.parent_count() >= 1 {
        let parent_commit = commit.parent(0)?;
        let parent_tree = parent_commit.tree()?;
        Some(parent_tree)
    } else {
        None
    };

    let commit_tree = commit.tree()?;

    let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&commit_tree), None)?;

    let file_diffs = collect_diffs(&diff)?;
    if file_diffs.is_empty() {
        return Ok(None);
    }

    let message = commit
        .message()
        .and_then(|message| message.lines().next())
        .unwrap_or("<no message>")
        .to_owned();

    Ok(Some(CommitInfo {
        short_id: commit.short_id(),
        message,
        file_diffs,
    }))
}

fn collect_diffs(diff: &Diff) -> Result<Vec<FileDiff>> {
    let mut diffs = Vec::new();

    for file_idx in 0..diff.deltas().len() {
        let delta = diff.deltas().nth(file_idx).unwrap();

        let path = if let Some(path) = delta.new_file().path() {
            path
        } else if let Some(path) = delta.old_file().path() {
            path
        } else {
            continue;
        };

        if path.components().any(|component| {
            FILTERED_COMPONENTS
                .iter()
                .any(|&filtered| component.as_os_str() == filtered)
        }) {
            continue;
        }

        let Some(mut patch) = Patch::from_diff(diff, file_idx)? else {
            continue;
        };

        let mut lines = Vec::new();
        patch.print(&mut |_delta, _hunk, line| {
            let content = String::from_utf8_lossy(line.content())
                .trim_end_matches('\n')
                .to_owned();
            lines.push(DiffLine {
                origin: line.origin(),
                content,
            });
            true
        })?;

        diffs.push(FileDiff {
            path: path.to_path_buf(),
            lines,
        });
    }

    Ok(diffs)
}
