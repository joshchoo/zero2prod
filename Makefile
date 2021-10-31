setup:
	cargo install sqlx-cli --version=0.5.7 --no-default-features --features postgres
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

db-migrate:
	SKIP_DOCKER=true ./scripts/init_db.sh

init-db:
	./scripts/init_db.sh

psql:
	psql -h localhost -p 5432 -U postgres newsletter

psql-tables:
	psql -h localhost -p 5432 -U postgres -c "\dt" newsletter

psql-columns:
	psql -h localhost -p 5432 -U postgres -c "\dS subscriptions" newsletter

run-trace:
	RUST_LOG=trace cargo run

sqlx-prepare:
	cargo sqlx prepare -- --lib

sqlx-prepare-check:
	cargo sqlx prepare --check -- --lib

test-trace:
	# we can prettify it by piping to `bunyan`, if installed
	TEST_LOG=true cargo test

docker-build:
	docker build --tag zero2prod --file Dockerfile .

docker-run:
	docker run -p 8000:8000 zero2prod
