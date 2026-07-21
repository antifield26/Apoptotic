#!/bin/bash
# ═══════════════════════════════════════════════════════════════
# CI PGO Optimization Script (D6)
# ═══════════════════════════════════════════════════════════════
# Run PGO instrument → train → optimize cycle for CI environments.
# Designed to be called from GitHub Actions or locally.
#
# Usage:
#   ./scripts/ci-optimize.sh              # PGO only (CI-safe)
#   ./scripts/ci-optimize.sh --bolt       # PGO + BOLT (requires perf, Linux only)
#   ./scripts/ci-optimize.sh --check      # Verify PGO binary exists
#
# Output: target/release/mc-server (PGO-optimized)

set -euo pipefail
MODE="${1:-pgo}"
TARGET="${TARGET:-aarch64-unknown-linux-gnu}"
BINARY="./target/${TARGET}/release/mc-server"
PROFILE_DIR="/tmp/mc-pgo-ci-$$"
RUSTFLAGS_BASE="-Ctarget-cpu=cortex-a76 -Ctarget-feature=+neon,+lse,+rcpc"

echo "=== CI PGO Optimization ==="
echo "Target: ${TARGET}"
echo "Mode: ${MODE}"
echo ""

# ── Phase 1: Instrumented Build ──
echo "--- Phase 1: Instrumented build ---"
RUSTFLAGS="${RUSTFLAGS_BASE} -Cprofile-generate=${PROFILE_DIR}" \
  cargo build --release --target "${TARGET}" 2>&1 | tail -3

# ── Phase 2: Training (3 iterations, 20s each) ──
echo "--- Phase 2: Training (3 × 20s) ---"
for i in 1 2 3; do
  echo "  Iteration ${i}/3..."
  timeout 25 "${BINARY}" --config config/default.toml &
  PID=$!
  sleep 3
  # Simulate light load
  echo -ne '\x10\x00\xF0\x05\x09localhost\x63\xDD\x01' | timeout 3 nc 127.0.0.1 25565 2>/dev/null || true
  sleep 17
  kill -TERM "${PID}" 2>/dev/null || true
  wait "${PID}" 2>/dev/null || true
  sleep 1
done

# ── Phase 3: Merge ──
echo "--- Phase 3: Merge profiles ---"
llvm-profdata merge -o "${PROFILE_DIR}/merged.profdata" "${PROFILE_DIR}"

# ── Phase 4: Optimized Build ──
echo "--- Phase 4: PGO-optimized build ---"
RUSTFLAGS="${RUSTFLAGS_BASE} -Cprofile-use=${PROFILE_DIR}/merged.profdata" \
  cargo build --release --target "${TARGET}" 2>&1 | tail -3

# ── Results ──
echo ""
echo "=== PGO Complete ==="
ls -lh "${BINARY}" 2>/dev/null || echo "  (cross-compiled — binary not linked)"
echo "Profile data: ${PROFILE_DIR}"

# ── Phase 5: BOLT (optional, Linux x86_64 only) ──
if [ "${MODE}" = "--bolt" ] && [ -f "${BINARY}" ] && command -v perf &>/dev/null; then
  echo ""
  echo "--- Phase 5: BOLT optimization ---"
  PERF_DATA="/tmp/mc-bolt-$$.fdata"
  BOLT_OUT="${BINARY}.bolt"

  "${BINARY}" &
  BOLT_PID=$!
  sleep 2
  timeout 30 perf record -e cycles:u -j any,u -o "${PERF_DATA}" -p "${BOLT_PID}" 2>/dev/null || true
  kill -TERM "${BOLT_PID}" 2>/dev/null || true
  wait "${BOLT_PID}" 2>/dev/null || true

  if perf2bolt -p "${PERF_DATA}" -o /tmp/mc-bolt.fdata "${BINARY}" 2>/dev/null; then
    llvm-bolt "${BINARY}" -o "${BOLT_OUT}" \
      --data=/tmp/mc-bolt.fdata \
      --reorder-blocks=ext-tsp \
      --reorder-functions=hfsort \
      --split-functions \
      --split-all-cold \
      --hot-data 2>&1 | tail -1
    echo "BOLT binary: ${BOLT_OUT}"
    ls -lh "${BOLT_OUT}"
  else
    echo "  BOLT skipped (perf2bolt unavailable)"
  fi
  rm -f "${PERF_DATA}" /tmp/mc-bolt.fdata
fi

rm -rf "${PROFILE_DIR}"
echo "Done."
