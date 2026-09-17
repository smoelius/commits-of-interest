#! /bin/bash

# set -x
set -euo pipefail

if [[ $# -ne 0 ]]; then
    echo "$0: expect no arguments" >&2
    exit 1
fi

# smoelius: The name of the next test must match what is in tests/ci.rs.
BLESS=1 cargo test --test ci version_prerelease_is_date_of_version_bump_or_latest_tag -- --exact

TAG="$(cat Cargo.toml | sed -n 's/^version = "\([^"]*\)"$/\1/;T;p')"

# smoelius: Update lockfile.
cargo check

git add Cargo.toml Cargo.lock

# smoelius: If the test above succeeds with `BLESS=1`, the latest commit's committer date becomes
# the current date. Preserve that in the next `git commit --amend`.
GIT_COMMITTER_DATE="$(git log -1 --format=%cI)" git commit --amend --no-edit

set +e
git tag v"$TAG"
STATUS=$?
set -e

if [[ $STATUS -ne 0 ]]; then
    echo "If tag 'v$TAG' exists in error:" >&2
    echo "    git tag -d v$TAG" >&2
    exit $STATUS
fi
