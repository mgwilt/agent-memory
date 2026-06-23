#!/usr/bin/env sh
set -eu

cargo test -p actr-ops
test -f config/prometheus/prometheus.yml
