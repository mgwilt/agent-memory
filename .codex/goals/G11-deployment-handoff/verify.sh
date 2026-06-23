#!/usr/bin/env sh
set -eu

cargo check --workspace --all-targets
test -f Dockerfile
test -f docker-compose.yml
