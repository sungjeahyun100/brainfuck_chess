# ── Frontend build stage ──────────────────────────────────────────────────────
FROM node:22-slim AS frontend-builder

WORKDIR /app/frontend

COPY frontend/package.json frontend/package-lock.json* ./
RUN npm ci

COPY frontend/ ./
# Skip type checking in Docker for faster builds (types are checked in CI)
RUN npx vite build

# ── Rust build stage ──────────────────────────────────────────────────────────
# Using rust:slim (tracks latest stable) for maximum Docker Hub compatibility.
# For reproducible builds pin to a specific tag e.g. rust:1.93-slim.
FROM rust:slim AS builder

# Install build dependencies (needed for linking on Debian slim)
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# ── Dependency caching layer ──────────────────────────────────────────────────
# Copy workspace manifest + lock file first.
COPY Cargo.toml Cargo.lock ./
COPY engine/Cargo.toml engine/
COPY server/Cargo.toml server/

# Create stub source files so `cargo build` can resolve and compile all deps.
# engine is a library crate; server is a binary crate.
RUN mkdir -p engine/src server/src && \
    echo "" > engine/src/lib.rs && \
    echo "fn main() {}" > server/src/main.rs && \
    cargo build --release -p brainfuck-chess-server && \
    # Remove ALL artifacts for our crates (lib prefix + no prefix) so real source triggers full recompile
    find target/release/deps -name 'brainfuck*' -delete && \
    find target/release/deps -name 'libbrainfuck*' -delete && \
    rm -f target/release/brainfuck-chess-server

# ── Real build ────────────────────────────────────────────────────────────────
COPY engine/ engine/
COPY server/ server/

RUN cargo build --release -p brainfuck-chess-server

# ── Runtime stage ─────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

# Copy built frontend static files
COPY --from=frontend-builder /app/frontend/dist /app/dist

COPY --from=builder /app/target/release/brainfuck-chess-server /usr/local/bin/server

# Cloud Run injects PORT env variable; default 8080 as fallback.
ENV PORT=8080
ENV STATIC_DIR=/app/dist
EXPOSE 8080

CMD ["server"]
