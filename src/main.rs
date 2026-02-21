use anyhow::{Result, bail, ensure};
use git2::Repository;
use std::env;
use std::process::{Command, exit};

mod git;
mod tui;

const HELP: &str = "\
commits-of-interest - Identify commits with meaningful code changes

Analyzes the commits between a given revision and HEAD, filtering out changes to
non-essential paths (e.g., CI configuration, lock files, tests) and presenting
the remaining commits in an interactive TUI for review.

The filtered components can be customized by adding a .filtered_components.txt
file to the repository root. Each non-empty line names an additional path
component to exclude.

USAGE:
    commits-of-interest [<revision>]

ARGUMENTS:
    <revision>    The base revision to compare against HEAD (default: most recent tag)

OPTIONS:
    -h, --help    Print this help message";

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{HELP}");
        exit(0);
    }

    let revision = match args.as_slice() {
        [_, revision] => revision.clone(),
        [_] => {
            let tag = most_recent_tag()?;
            eprintln!("No revision specified; using most recent tag: {tag}");
            tag
        }
        _ => bail!("expect at most one argument: previous revision"),
    };

    let repo = Repository::open(".")?;
    let commits = git::collect_commits(&repo, &revision)?;

    tui::run(commits)?;

    Ok(())
}

fn most_recent_tag() -> Result<String> {
    let output = Command::new("git")
        .args(["describe", "--tags", "--abbrev=0"])
        .output()?;
    ensure!(
        output.status.success(),
        "no previous tag found; specify a revision explicitly"
    );
    let tag = std::str::from_utf8(&output.stdout)?.trim().to_string();
    Ok(tag)
}
