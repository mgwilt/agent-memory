.PHONY: fmt check clippy test verify

fmt:
	cargo fmt --all

check:
	cargo check --workspace --all-targets

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

test:
	cargo test --workspace

verify: fmt check clippy test
