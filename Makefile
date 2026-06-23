.PHONY: fmt check clippy test e2e-agentic verify

fmt:
	cargo fmt --all

check:
	cargo check --workspace --all-targets

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

test:
	cargo test --workspace

e2e-agentic:
	pnpm e2e:agentic

verify: fmt check clippy test
