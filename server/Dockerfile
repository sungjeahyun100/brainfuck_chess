# ── Build stage ──────────────────────────────────────────────────────────────
FROM rust:1.93-slim AS builder

WORKDIR /app

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
COPY engine/Cargo.toml engine/
COPY server/Cargo.toml server/

RUN mkdir -p engine/src server/src && \
    echo "pub fn main() {}" > engine/src/lib.rs && \
    echo "fn main() {}" > server/src/main.rs && \
    cargo build --release -p brainfuck-chess-server && \
    rm -rf engine/src server/src

# Build actual source
COPY engine/ engine/
COPY server/ server/

RUN touch engine/src/lib.rs server/src/main.rs && \
    cargo build --release -p brainfuck-chess-server

# ── Runtime stage ─────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/brainfuck-chess-server /usr/local/bin/server

ENV PORT=8080
EXPOSE 8080

CMD ["server"]
