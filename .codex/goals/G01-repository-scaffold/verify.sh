#!/usr/bin/env sh
set -eu

cargo check --workspace --all-targets
test -f AGENTS.md
test -d .codex/goals/template
