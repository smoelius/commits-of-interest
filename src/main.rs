use anyhow::{Result, bail};
use git2::{Commit, Oid, Repository, Sort};
use std::{env, path::PathBuf};

const FILTERED_COMPONENTS: &[&str] = &[
    ".github",
    "Cargo.toml",
    "Cargo.lock",
    "examples",
    "fixtures",
    "tests",
];

trait ShortId {
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

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    let [_, revision] = args.as_slice() else {
        bail!("expect one argument: previous revision");
    };

    let repo = Repository::open(".")?;

    process_commits(&repo, revision)?;

    Ok(())
}

fn process_commits(repo: &Repository, revision: &str) -> Result<()> {
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
        if let Some(unfiltered_paths) = get_unfiltered_paths(repo, &commit)? {
            print_commit(&commit, &unfiltered_paths);
        }
    }

    Ok(())
}

fn get_unfiltered_paths(repo: &Repository, commit: &Commit) -> Result<Option<Vec<PathBuf>>> {
    let parent_tree = if commit.parent_count() >= 1 {
        let parent_commit = commit.parent(0)?;
        let parent_tree = parent_commit.tree()?;
        Some(parent_tree)
    } else {
        None
    };

    let commit_tree = commit.tree()?;

    let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&commit_tree), None)?;

    let mut unfiltered_paths = Vec::new();

    for delta in diff.deltas() {
        let old_path = delta.old_file().path();
        let new_path = delta.new_file().path();
        for path in [old_path, new_path].into_iter().flatten() {
            if path.components().any(|component| {
                FILTERED_COMPONENTS
                    .iter()
                    .any(|&filtered| component.as_os_str() == filtered)
            }) {
                continue;
            }
            unfiltered_paths.push(path.to_path_buf());
        }
    }

    if unfiltered_paths.is_empty() {
        return Ok(None);
    }

    unfiltered_paths.sort();
    unfiltered_paths.dedup();

    Ok(Some(unfiltered_paths))
}

fn print_commit(commit: &Commit, unfiltered_paths: &[PathBuf]) {
    let message = commit
        .message()
        .and_then(|message| message.lines().next())
        .unwrap_or("<no message>");
    println!("{} {}", commit.short_id(), message);
    for path in unfiltered_paths {
        println!("    {}", path.display());
    }
}
