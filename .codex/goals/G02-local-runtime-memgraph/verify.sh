#!/usr/bin/env sh
set -eu

test -f docker-compose.yml
test -f config/prometheus/prometheus.yml
test -f crates/actr-store/migrations/001_actr_memory_schema.cypher
test -x scripts/wait-for-memgraph.sh
test -x scripts/bootstrap-memgraph.sh
