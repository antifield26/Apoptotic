# Multi-arch Minecraft LAN Server — Raspberry Pi 5 (ARM64) + x86_64
# Build: docker build --platform linux/arm64 -t mc-server .
#   or:  docker build --platform linux/amd64 -t mc-server .

FROM --platform=$BUILDPLATFORM rust:1.85-slim-bookworm AS builder

ARG TARGETPLATFORM
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Cross-compilation targets
RUN case "${TARGETPLATFORM}" in \
      "linux/arm64") rustup target add aarch64-unknown-linux-gnu ;; \
      "linux/amd64") rustup target add x86_64-unknown-linux-gnu ;; \
    esac

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY .cargo/ .cargo/

# Build for target platform
RUN case "${TARGETPLATFORM}" in \
      "linux/arm64") \
        CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
        apt-get install -y gcc-aarch64-linux-gnu && \
        cargo build --release --target aarch64-unknown-linux-gnu && \
        cp target/aarch64-unknown-linux-gnu/release/mc-server /mc-server ;; \
      "linux/amd64") \
        cargo build --release --target x86_64-unknown-linux-gnu && \
        cp target/x86_64-unknown-linux-gnu/release/mc-server /mc-server ;; \
    esac

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*

RUN useradd -m -u 1000 minecraft
USER minecraft
WORKDIR /home/minecraft

COPY --from=builder /mc-server /home/minecraft/mc-server
RUN mkdir -p /home/minecraft/config /home/minecraft/data/world/region

COPY config/default.toml /home/minecraft/config/default.toml

EXPOSE 25565 25575 9100
VOLUME ["/home/minecraft/config", "/home/minecraft/data"]

HEALTHCHECK --interval=30s --timeout=3s --retries=3 \
  CMD curl -f http://localhost:9100/metrics || exit 1

ENV RUST_LOG=info
ENTRYPOINT ["./mc-server"]
