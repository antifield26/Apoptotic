#!/bin/bash
# RPi 5 PGO (Profile-Guided Optimization) 脚本
# 用法: ./scripts/optimize-profile.sh [iterations]
# 需要: llvm-profdata, cargo, rustup

set -euo pipefail
ITERATIONS=${1:-3}
TARGET="aarch64-unknown-linux-gnu"
PROFILE_DIR="/tmp/mc-pgo"
BINARY="./target/${TARGET}/release/mc-server"

echo "=== Phase 1: Instrumented Build ==="
RUSTFLAGS="-Cprofile-generate=${PROFILE_DIR}" \
  cargo build --release --target "${TARGET}"

echo "=== Phase 2: Training (${ITERATIONS} iterations) ==="
for i in $(seq 1 "${ITERATIONS}"); do
  echo "--- Iteration ${i}/${ITERATIONS} ---"
  # Start server
  "${BINARY}" &
  PID=$!
  sleep 3
  # Simulate activity: join + move + chat + disconnect (via netcat)
  echo -ne '\x10\x00\xF0\x05\x09localhost\x63\xDD\x01' | nc -w1 127.0.0.1 25565 || true
  sleep 10
  # Graceful shutdown
  kill -TERM "${PID}" 2>/dev/null || true
  wait "${PID}" 2>/dev/null || true
  sleep 1
done

echo "=== Phase 3: Merge Profile Data ==="
llvm-profdata merge -o "${PROFILE_DIR}/merged.profdata" "${PROFILE_DIR}"

echo "=== Phase 4: Optimized Build ==="
RUSTFLAGS="-Cprofile-use=${PROFILE_DIR}/merged.profdata" \
  cargo build --release --target "${TARGET}"

echo "=== Phase 5: Binary Size & Stats ==="
ls -lh "${BINARY}"
file "${BINARY}"

echo "=== Done! Optimized binary at ${BINARY} ==="
