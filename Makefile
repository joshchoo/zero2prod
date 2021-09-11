setup:
	cargo install cargo-tarpaulin

lint:
	cargo clippy

format:
	cargo fmt

test-coverage:
	cargo tarpaulin --ignore-tests

ci-audit:
	cargo audit

ci-lint:
	cargo clippy -- -D warnings

ci-format:
	cargo fmt -- --check