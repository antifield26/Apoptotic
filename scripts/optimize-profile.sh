#!/bin/bash
# ═══════════════════════════════════════════════════════════════
# RPi 5 PGO (Profile-Guided Optimization) 脚本
# ═══════════════════════════════════════════════════════════════
# 通过训练运行收集执行 profile, 反馈给编译器实现 5-15% 性能提升
#
# 用法: ./scripts/optimize-profile.sh [iterations]
# 需要: llvm-profdata, cargo, rustup, netcat
#
# RPi 5 前置条件:
#   rustup target add aarch64-unknown-linux-gnu
#   sudo apt install gcc-aarch64-linux-gnu llvm-tools netcat-openbsd
#   # 或 macOS 交叉编译: brew install llvm
#   export PATH="/opt/homebrew/opt/llvm/bin:$PATH"

set -euo pipefail
ITERATIONS=${1:-3}
TARGET="${TARGET:-aarch64-unknown-linux-gnu}"
PROFILE_DIR="/tmp/mc-pgo-$$"
BINARY="./target/${TARGET}/release/mc-server"
CONFIG_DIR="./config"

trap 'rm -rf "${PROFILE_DIR}"' EXIT

echo "=== PGO: Instrumented Build (${TARGET}) ==="
RUSTFLAGS="-Cprofile-generate=${PROFILE_DIR} -Ctarget-cpu=cortex-a76" \
  cargo build --release --target "${TARGET}"

echo "=== PGO: Training (${ITERATIONS} iterations) ==="
for i in $(seq 1 "${ITERATIONS}"); do
  echo "--- Iteration ${i}/${ITERATIONS} ---"
  "${BINARY}" --config "${CONFIG_DIR}/default.toml" &
  PID=$!
  sleep 5
  # Simulate: handshake + status ping (generates chunk/network profiles)
  echo -ne '\x10\x00\xF0\x05\x09localhost\x63\xDD\x01' | nc -w2 127.0.0.1 25565 || true
  sleep 15
  kill -TERM "${PID}" 2>/dev/null || true
  wait "${PID}" 2>/dev/null || true
  sleep 1
done

echo "=== PGO: Merge Profile Data ==="
llvm-profdata merge -o "${PROFILE_DIR}/merged.profdata" "${PROFILE_DIR}"

echo "=== PGO: Optimized Build ==="
RUSTFLAGS="-Cprofile-use=${PROFILE_DIR}/merged.profdata -Ctarget-cpu=cortex-a76" \
  cargo build --release --target "${TARGET}"

echo "=== PGO: Result ==="
ls -lh "${BINARY}"
file "${BINARY}"
echo "Done! Optimized binary: ${BINARY}"
