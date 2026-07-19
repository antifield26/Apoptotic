#!/bin/bash
# ═══════════════════════════════════════════════════════
# 部署脚本 — 构建并部署到 Raspberry Pi
# ═══════════════════════════════════════════════════════
set -euo pipefail

GREEN='\033[0;32m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[DEPLOY]${NC} $*"; }

PI_HOST="${PI_HOST:-raspberrypi}"
PI_USER="${PI_USER:-pi}"
PI_PATH="${PI_PATH:-/opt/mc-server}"
BUILD_MODE="${1:-native}"  # native | cross

case "$BUILD_MODE" in
    cross)
        # ── Cross-compile on local machine, deploy binary ──
        log_info "Cross-compiling for ARM64..."
        rustup target add aarch64-unknown-linux-gnu 2>/dev/null || true
        cargo build --release --target aarch64-unknown-linux-gnu

        log_info "Deploying to ${PI_USER}@${PI_HOST}:${PI_PATH}..."
        ssh "${PI_USER}@${PI_HOST}" "mkdir -p ${PI_PATH}/{bin,config,data}"
        scp "target/aarch64-unknown-linux-gnu/release/mc-server" \
            "${PI_USER}@${PI_HOST}:${PI_PATH}/bin/mc-server"
        scp config/default.toml "${PI_USER}@${PI_HOST}:${PI_PATH}/config/"
        scp config/rpi5.toml "${PI_USER}@${PI_HOST}:${PI_PATH}/config/"
        scp scripts/mc-server.service "${PI_USER}@${PI_HOST}:/tmp/"

        log_info "Installing systemd service..."
        ssh "${PI_USER}@${PI_HOST}" "
            sudo mv /tmp/mc-server.service /etc/systemd/system/ &&
            sudo systemctl daemon-reload &&
            sudo systemctl enable mc-server &&
            sudo systemctl restart mc-server
        "
        ;;

    native|*)
        # ── Build on the Pi itself via SSH ──
        log_info "Syncing source to ${PI_USER}@${PI_HOST}:${PI_PATH}..."
        rsync -avz --delete \
            --exclude='target/' \
            --exclude='.git/' \
            --exclude='data/' \
            ./ "${PI_USER}@${PI_HOST}:${PI_PATH}/"

        log_info "Building on Pi..."
        ssh "${PI_USER}@${PI_HOST}" "
            cd ${PI_PATH} &&
            docker compose -f docker/docker-compose.yml build &&
            docker compose -f docker/docker-compose.yml up -d
        "

        log_info "Deployment complete — server is running on ${PI_HOST}:25565"
        ;;
esac

log_info "Done!"
