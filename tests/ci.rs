use anyhow::{Result, ensure};
use assert_cmd::assert::OutputAssertExt;
use std::{
    ffi::OsStr,
    fs::{read_to_string, write},
    process::{Command, Stdio},
};
use toml_edit::{DocumentMut, Item};

const VERSION_BUMP_SUBJECT: &str = "Bump version";

#[test]
fn clippy() {
    Command::new("cargo")
        .args([
            "+nightly",
            "clippy",
            "--all-targets",
            "--offline",
            "--",
            "--deny=warnings",
        ])
        .assert()
        .success();
}

#[test]
fn dylint() {
    Command::new("cargo")
        .args(["dylint", "--all", "--", "--all-targets"])
        .env("DYLINT_RUSTFLAGS", "--deny=warnings")
        .assert()
        .success();
}

#[test]
fn fmt() {
    let mut command = Command::new("cargo");
    command.args(["+nightly", "fmt", "--check"]);
    command.assert().success();
}

// smoelius: The name of the next test must match what is in scripts/update_version.sh.
#[cfg_attr(dylint_lib = "general", allow(non_thread_safe_call_in_test))]
#[test]
fn version_prerelease_is_date_of_version_bump_or_latest_tag() {
    let latest_commit = latest_commit().unwrap();
    let latest_commit_subject = commit_subject(&latest_commit).unwrap();
    let version_bump_date = if latest_commit_subject == VERSION_BUMP_SUBJECT {
        rev_short_date(&latest_commit).map(Some).unwrap()
    } else {
        None
    };

    let contents = read_to_string("Cargo.toml").unwrap();
    let mut document = contents.parse::<DocumentMut>().unwrap();
    let version = document
        .get_mut("package")
        .and_then(Item::as_table_mut)
        .and_then(|table| table.get_mut("version"))
        .and_then(Item::as_value_mut)
        .unwrap();
    let version_as_str = version.as_str().unwrap();
    let (base, prerelease) = version_as_str
        .split_once('-')
        .unwrap_or((version_as_str, ""));

    if enabled("BLESS") {
        if version_bump_date.is_none() {
            panic!(
                "`BLESS` was set but latest commit subject is not {VERSION_BUMP_SUBJECT:?}: \
                 {latest_commit_subject}"
            );
        };
        if !repository_is_clean().unwrap() {
            panic!("`BLESS` was set but repository is dirty");
        }
        // smoelius: Ensure the latest commit uses the current date.
        amend_latest_commit::<_, &OsStr>([]).unwrap();
        let amended_date = rev_short_date("HEAD").unwrap();
        *version = format!("{base}-{amended_date}").into();
        write("Cargo.toml", document.to_string()).unwrap();
        update_lockfile().unwrap();
        amend_latest_commit(["Cargo.toml", "Cargo.lock"]).unwrap();
        return;
    }

    let date = version_bump_date.unwrap_or_else(|| {
        latest_tag_date().unwrap_or_else(|error| {
            panic!(
                "latest commit is not a version bump ({latest_commit_subject:?} != \
                 {VERSION_BUMP_SUBJECT:?}) and latest tag date could not be determined: {error:?}"
            );
        })
    });

    assert_eq!(date, prerelease);
}

fn latest_commit() -> Result<String> {
    let mut command = Command::new("git");
    command.args(["rev-parse", "HEAD"]);
    stdout_as_string(command)
}

fn commit_subject(rev: &str) -> Result<String> {
    let mut command = Command::new("git");
    command.args(["log", "-1", "--format=%s", rev]);
    stdout_as_string(command)
}

fn latest_tag_date() -> Result<String> {
    let tag = latest_tag()?;
    rev_short_date(&tag)
}

fn latest_tag() -> Result<String> {
    let mut command = Command::new("git");
    command.args(["describe", "--tags", "--abbrev=0"]);
    stdout_as_string(command)
}

fn repository_is_clean() -> Result<bool> {
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

fn amend_latest_commit<I, S>(paths: I) -> Result<()>
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
fn rev_short_date(rev: &str) -> Result<String> {
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

fn update_lockfile() -> Result<()> {
    let mut command = Command::new("cargo");
    command.arg("check");
    let status = command.status()?;
    ensure!(status.success(), "command failed: {command:?}");
    Ok(())
}

fn enabled(key: &str) -> bool {
    std::env::var(key).is_ok_and(|value| value != "0")
}
