# ═══════════════════════════════════════════════════════
# Stage 1: Build — Cross-compile for ARM64 from x86_64
# ═══════════════════════════════════════════════════════
FROM rust:1.85-slim-bookworm AS builder

# Install cross-compilation toolchain
RUN dpkg --add-architecture arm64 && \
    apt-get update && \
    apt-get install -y --no-install-recommends \
        gcc-aarch64-linux-gnu \
        libc6-dev-arm64-cross \
        pkg-config \
        && rm -rf /var/lib/apt/lists/*

# Add ARM64 target
RUN rustup target add aarch64-unknown-linux-gnu

# Configure cargo for cross-compilation
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
    CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc \
    CXX_aarch64_unknown_linux_gnu=aarch64-linux-gnu-g++

WORKDIR /build

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
COPY crates/mc-core/Cargo.toml crates/mc-core/
COPY crates/mc-protocol/Cargo.toml crates/mc-protocol/
COPY crates/mc-network/Cargo.toml crates/mc-network/
COPY crates/mc-world/Cargo.toml crates/mc-world/
COPY crates/mc-entity/Cargo.toml crates/mc-entity/
COPY crates/mc-player/Cargo.toml crates/mc-player/
COPY crates/mc-persistence/Cargo.toml crates/mc-persistence/
COPY crates/mc-command/Cargo.toml crates/mc-command/
COPY crates/mc-admin/Cargo.toml crates/mc-admin/
COPY crates/mc-server/Cargo.toml crates/mc-server/

# Create dummy lib.rs files for dependency caching
RUN mkdir -p crates/mc-core/src && echo "pub mod auth; pub mod block; pub mod position; pub mod types;" > crates/mc-core/src/lib.rs
RUN mkdir -p crates/mc-core/src && touch crates/mc-core/src/auth.rs crates/mc-core/src/block.rs crates/mc-core/src/position.rs crates/mc-core/src/types.rs
RUN mkdir -p crates/mc-protocol/src && echo "pub mod codec; pub mod packets; pub mod state; pub mod varint;" > crates/mc-protocol/src/lib.rs
RUN mkdir -p crates/mc-network/src && echo "pub mod connection; pub mod encryption; pub mod lan_broadcast; pub mod listener; pub mod packet_io;" > crates/mc-network/src/lib.rs
RUN mkdir -p crates/mc-world/src && echo "pub mod chunk; pub mod chunk_store; pub mod generator; pub mod lighting; pub mod paletted; pub mod world;" > crates/mc-world/src/lib.rs
RUN mkdir -p crates/mc-entity/src && echo "pub mod bundles; pub mod components; pub mod resources; pub mod systems;" > crates/mc-entity/src/lib.rs
RUN mkdir -p crates/mc-player/src && echo "pub mod gamemode; pub mod inventory; pub mod player; pub mod session;" > crates/mc-player/src/lib.rs
RUN mkdir -p crates/mc-persistence/src && echo "pub mod player_data; pub mod save_manager; pub mod world_save;" > crates/mc-persistence/src/lib.rs
RUN mkdir -p crates/mc-command/src && echo "pub mod commands; pub mod dispatcher; pub mod parser;" > crates/mc-command/src/lib.rs
RUN mkdir -p crates/mc-admin/src && echo "pub mod console; pub mod metrics; pub mod rcon;" > crates/mc-admin/src/lib.rs
RUN mkdir -p crates/mc-server/src && echo "fn main() {}" > crates/mc-server/src/main.rs

# Build dependencies (will be cached)
RUN cargo build --release --target aarch64-unknown-linux-gnu 2>/dev/null || true

# Copy actual source
COPY . .

# Build release binary
RUN cargo build --release --target aarch64-unknown-linux-gnu

# ═══════════════════════════════════════════════════════
# Stage 2: Runtime — Minimal Debian image
# ═══════════════════════════════════════════════════════
FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates \
        libsqlite3-0 \
        && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary
COPY --from=builder /build/target/aarch64-unknown-linux-gnu/release/mc-server /app/mc-server

# Copy config
COPY config/ /app/config/

# Create data directory
RUN mkdir -p /app/data

# Expose Minecraft port + optional metrics port
EXPOSE 25565 9090

# Volume for persistent data
VOLUME ["/app/data", "/app/config"]

ENV MCS_CONFIG=/app/config/default.toml

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["/app/mc-server", "--healthcheck"] 2>/dev/null || exit 0

ENTRYPOINT ["/app/mc-server"]
