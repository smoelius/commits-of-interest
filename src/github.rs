use crate::git::CommitInfo;
use serde_json::{Value, from_slice};
use std::fmt::Write;
use std::process::Command;

const BATCH_SIZE: usize = 50;

pub fn lookup_prs(commits: &mut [CommitInfo]) -> bool {
    let Some((owner, name)) = repo_owner_and_name() else {
        return false;
    };

    let mut success = false;
    for chunk_start in (0..commits.len()).step_by(BATCH_SIZE) {
        let chunk_end = (chunk_start + BATCH_SIZE).min(commits.len());
        if lookup_prs_batch(&mut commits[chunk_start..chunk_end], &owner, &name) {
            success = true;
        }
    }
    success
}

fn repo_owner_and_name() -> Option<(String, String)> {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let url = String::from_utf8(output.stdout).ok()?;
    parse_github_remote(url.trim())
}

fn parse_github_remote(url: &str) -> Option<(String, String)> {
    // git@github.com:owner/repo.git
    // https://github.com/owner/repo.git
    let path = url
        .strip_prefix("git@github.com:")
        .or_else(|| url.strip_prefix("https://github.com/"))?;
    let path = path.strip_suffix(".git").unwrap_or(path);
    let (owner, name) = path.split_once('/')?;
    Some((owner.to_owned(), name.to_owned()))
}

fn lookup_prs_batch(commits: &mut [CommitInfo], owner: &str, name: &str) -> bool {
    if commits.is_empty() {
        return false;
    }

    let query = build_graphql_query(commits, owner, name);

    let output = match Command::new("gh")
        .args(["api", "graphql", "-f", &format!("query={query}")])
        .output()
    {
        Ok(output) if output.status.success() => output.stdout,
        _ => return false,
    };

    let json: Value = match from_slice(&output) {
        Ok(v) => v,
        Err(_) => return false,
    };

    let Some(repo) = json.get("data").and_then(|data| data.get("repository")) else {
        return false;
    };

    for (i, commit) in commits.iter_mut().enumerate() {
        let alias = format!("c{i}");
        if let Some(pr_info) = extract_pr(repo, &alias) {
            commit.pr = Some(pr_info);
        }
    }
    true
}

fn build_graphql_query(commits: &[CommitInfo], owner: &str, name: &str) -> String {
    let mut query = format!("query {{\n  repository(owner: \"{owner}\", name: \"{name}\") {{\n");
    for (i, commit) in commits.iter().enumerate() {
        let oid = &commit.oid;
        writeln!(
            &mut query,
            "    c{i}: object(oid: \"{oid}\") {{
      ... on Commit {{
        associatedPullRequests(first: 1) {{
          nodes {{ number }}
        }}
      }}
    }}"
        )
        .unwrap();
    }
    query.push_str("  }\n}");
    query
}

fn extract_pr(repo: &Value, alias: &str) -> Option<u64> {
    let nodes = repo
        .get(alias)?
        .get("associatedPullRequests")?
        .get("nodes")?
        .as_array()?;
    let first = nodes.first()?;
    first.get("number")?.as_u64()
}
