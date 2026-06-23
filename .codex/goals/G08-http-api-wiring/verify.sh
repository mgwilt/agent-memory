#!/usr/bin/env sh
set -eu

cargo test -p actr-api
cargo run -p actr-api >/tmp/actr-api-route-manifest.txt
test -s /tmp/actr-api-route-manifest.txt
