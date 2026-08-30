# syntax=docker/dockerfile:1.7

# ── builder ──────────────────────────────────────────────────────────────
FROM rust:1.93.1-slim-bookworm AS builder
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY src ./src

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release --locked --bin bandao-api \
    && cp target/release/bandao-api /usr/local/bin/bandao-api

# ── runtime ──────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

ENV DEBIAN_FRONTEND=noninteractive

# ca-certificates only: the api makes outbound TLS connections (Mongo,
# Resend, customers' MSSQL) and nothing else here needs a package.
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/local/bin/bandao-api /usr/local/bin/bandao-api

EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/bandao-api"]
