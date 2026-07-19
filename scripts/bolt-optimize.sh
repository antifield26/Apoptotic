#!/bin/bash
# RPi 5 BOLT (Binary Optimization and Layout Tool) 脚本
# BOLT 优化二进制布局，减少 I-cache miss，提升 5-15%
# 需要: llvm-bolt, perf, cargo

set -euo pipefail
TARGET="aarch64-unknown-linux-gnu"
BINARY="./target/${TARGET}/release/mc-server"
OUTPUT="${BINARY}.bolt"
PERF_DATA="/tmp/mc-bolt-perf.fdata"

echo "=== Phase 1: Record Profile ==="
# 启动服务器并用 perf 采样
"${BINARY}" &
PID=$!
sleep 2
perf record -e cycles:u -j any,u -o "${PERF_DATA}" -p "${PID}" -- sleep 60 || true
kill -TERM "${PID}" 2>/dev/null || true
wait "${PID}" 2>/dev/null || true

echo "=== Phase 2: Convert Profile ==="
perf2bolt -p "${PERF_DATA}" -o /tmp/mc-bolt.fdata "${BINARY}" || {
  echo "WARNING: perf2bolt failed — skipping BOLT optimization"
  exit 0
}

echo "=== Phase 3: Apply BOLT Optimization ==="
llvm-bolt "${BINARY}" -o "${OUTPUT}" \
  --data=/tmp/mc-bolt.fdata \
  --reorder-blocks=ext-tsp \
  --reorder-functions=hfsort \
  --split-functions \
  --split-all-cold \
  --dyno-stats \
  --hot-data

echo "=== Phase 4: Result ==="
ls -lh "${BINARY}" "${OUTPUT}"
echo "BOLT optimized binary: ${OUTPUT}"
