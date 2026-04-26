# syntax=docker/dockerfile:1.6

# ---------- builder ----------
FROM rust:1.80-slim AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Cache deps layer
COPY Cargo.toml ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src target/release/deps/portfolio_tracker*

# Real build
COPY src ./src
COPY migrations ./migrations
COPY templates ./templates
RUN cargo build --release

# ---------- runtime ----------
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/nworth-web /usr/local/bin/nworth-web
COPY templates ./templates
COPY static ./static
COPY migrations ./migrations

RUN mkdir -p /app/data
VOLUME ["/app/data"]

ENV BIND_ADDR=0.0.0.0:8080 \
    DATABASE_URL=sqlite:///app/data/portfolio.db?mode=rwc \
    RUST_LOG=info

EXPOSE 8080
CMD ["nworth-web"]
