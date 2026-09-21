#! /bin/bash

# set -x
set -euo pipefail

if [[ $# -ne 0 ]]; then
    echo "$0: expect no arguments" >&2
    exit 1
fi

if [[ "$(git branch --show-current)" = 'main' ]]; then
    echo "$0: should not be run on the main branch" >&2
    exit 1
fi

# smoelius: The name of the next test must match what is in tests/ci.rs.
BLESS=1 cargo test --test ci version_prerelease_is_date_of_version_bump_or_latest_tag -- --exact
