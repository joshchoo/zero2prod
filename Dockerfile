FROM rust:1.55.0

WORKDIR /app

COPY configuration configuration
COPY migrations migrations
COPY src src
COPY Cargo.lock Cargo.lock
COPY Cargo.toml Cargo.toml
COPY sqlx-data.json sqlx-data.json

ENV SQLX_OFFLINE true
RUN cargo build --release

ENV APP_ENVIRONMENT production
ENTRYPOINT ["./target/release/zero2prod"]