use anyhow::{Result, bail};
use git2::Repository;
use std::env;
use std::process;

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
    commits-of-interest <revision>

ARGUMENTS:
    <revision>    The base revision to compare against HEAD

OPTIONS:
    -h, --help    Print this help message";

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{HELP}");
        process::exit(0);
    }

    let [_, revision] = args.as_slice() else {
        bail!("expect one argument: previous revision");
    };

    let repo = Repository::open(".")?;
    let commits = git::collect_commits(&repo, revision)?;

    tui::run(commits)?;

    Ok(())
}
