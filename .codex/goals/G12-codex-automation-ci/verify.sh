#!/usr/bin/env sh
set -eu

test -d .codex/goals
test -f .github/workflows/ci.yml
find .codex/goals -name goal.yaml | grep -q G12-codex-automation-ci
