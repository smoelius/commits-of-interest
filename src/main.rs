use anyhow::{Result, bail};
use git2::Repository;
use std::env;

mod git;
mod tui;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    let [_, revision] = args.as_slice() else {
        bail!("expect one argument: previous revision");
    };

    let repo = Repository::open(".")?;
    let commits = git::collect_commits(&repo, revision)?;

    tui::run(commits)?;

    Ok(())
}
