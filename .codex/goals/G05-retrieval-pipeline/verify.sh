#!/usr/bin/env sh
set -eu

cargo test retrieval_pipeline -- --nocapture
