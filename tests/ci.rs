use anyhow::{Result, bail, ensure};
use assert_cmd::assert::OutputAssertExt;
use std::{
    ffi::OsStr,
    fs::{read_to_string, write},
    process::Command,
};
use toml_edit::{DocumentMut, Item, Value};

use crate::git::{
    amend_latest_commit, commit_subject, latest_commit, latest_tag_date, repository_is_clean,
    rev_short_date,
};

mod git;

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
        update_version(document).unwrap();
        return;
    }

    let version = package_version(&mut document).unwrap();
    let (_, prerelease) = split_version(version).unwrap();

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

fn update_version(mut document: DocumentMut) -> Result<()> {
    let Some(version) = package_version(&mut document) else {
        bail!("failed to get package version");
    };
    let Some((base, _)) = split_version(version) else {
        bail!("version is not a string");
    };

    // smoelius: Ensure the latest commit uses the current date.
    amend_latest_commit::<_, &OsStr>([])?;

    let amended_date = rev_short_date("HEAD")?;

    *version = format!("{base}-{amended_date}").into();

    write("Cargo.toml", document.to_string())?;

    update_lockfile()?;

    amend_latest_commit(["Cargo.toml", "Cargo.lock"])?;

    Ok(())
}

fn package_version(document: &mut DocumentMut) -> Option<&mut Value> {
    document
        .get_mut("package")
        .and_then(Item::as_table_mut)
        .and_then(|table| table.get_mut("version"))
        .and_then(Item::as_value_mut)
}

fn split_version(version: &Value) -> Option<(&str, &str)> {
    let s = version.as_str()?;
    Some(s.split_once('+').unwrap_or((s, "")))
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
