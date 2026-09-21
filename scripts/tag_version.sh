#! /bin/bash

# set -x
set -euo pipefail

if [[ $# -ne 0 ]]; then
    echo "$0: expect no arguments" >&2
    exit 1
fi

if [[ "$(git branch --show-current)" != 'main' ]]; then
    echo "$0: should be run on the main branch" >&2
    exit 1
fi

TAG="$(cat Cargo.toml | sed -n 's/^version = "\([^"]*\)"$/\1/;T;p')"

set +e
git tag v"$TAG"
STATUS=$?
set -e

if [[ $STATUS -ne 0 ]]; then
    echo "If tag 'v$TAG' exists in error:" >&2
    echo "    git tag -d v$TAG" >&2
    exit $STATUS
fi
