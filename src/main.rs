use anyhow::{Result, bail};
use git2::Repository;
use std::env;

mod git;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    let [_, revision] = args.as_slice() else {
        bail!("expect one argument: previous revision");
    };

    let repo = Repository::open(".")?;

    git::process_commits(&repo, revision)?;

    Ok(())
}
