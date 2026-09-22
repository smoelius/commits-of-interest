use anyhow::{Result, ensure};
use std::{
    ffi::OsStr,
    process::{Command, Stdio},
};

pub fn latest_commit() -> Result<String> {
    let mut command = Command::new("git");
    command.args(["rev-parse", "HEAD"]);
    stdout_as_string(command)
}

pub fn commit_subject(rev: &str) -> Result<String> {
    let mut command = Command::new("git");
    command.args(["log", "-1", "--format=%s", rev]);
    stdout_as_string(command)
}

pub fn latest_tag_date() -> Result<String> {
    let tag = latest_tag()?;
    rev_short_date(&tag)
}

fn latest_tag() -> Result<String> {
    let mut command = Command::new("git");
    command.args(["describe", "--tags", "--abbrev=0"]);
    stdout_as_string(command)
}

pub fn repository_is_clean() -> Result<bool> {
    let mut command = Command::new("git");
    command.stdout(Stdio::null());

    command.args(["diff", "--exit-code"]);
    let status = command.status()?;
    if !status.success() {
        return Ok(false);
    }

    command.arg("--staged");
    let status = command.status()?;

    Ok(status.success())
}

pub fn amend_latest_commit<I, S>(paths: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut paths = paths.into_iter().peekable();

    let git_committer_date = if paths.peek().is_some() {
        rev_iso_date("HEAD").map(Some)?
    } else {
        None
    };

    let mut command = Command::new("git");
    command.arg("add");
    command.args(paths);
    let status = command.status()?;
    ensure!(status.success(), "command failed: {command:?}");

    let mut command = Command::new("git");
    command.args(["commit", "--amend", "--allow-empty", "--no-edit"]);
    if let Some(git_committer_date) = git_committer_date {
        command.env("GIT_COMMITTER_DATE", git_committer_date);
    }
    let status = command.status()?;
    ensure!(status.success(), "command failed: {command:?}");

    Ok(())
}

// smoelius: Use the committer date rather than the author date. Rebasing normally changes the
// committer date but not the author date. If a version bump commit is rebased onto (say) a bug fix,
// we want the date of the rebase (the committer date), not the original date (the author date).
pub fn rev_short_date(rev: &str) -> Result<String> {
    let mut command = Command::new("git");
    command.args(["log", "-1", "--format=%cs", rev]);
    stdout_as_string(command)
}

fn rev_iso_date(rev: &str) -> Result<String> {
    let mut command = Command::new("git");
    command.args(["log", "-1", "--format=%cI", rev]);
    stdout_as_string(command)
}

fn stdout_as_string(mut command: Command) -> Result<String> {
    let output = command.output()?;
    ensure!(output.status.success(), "command failed: {command:?}");
    let stdout = str::from_utf8(&output.stdout)?;
    Ok(stdout.trim_end().to_owned())
}
