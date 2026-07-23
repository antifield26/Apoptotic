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

echo "=== PGO: Pre-generate chunks for training ==="
"${BINARY}" pregenerate --radius 100 --threads 2 2>/dev/null || true

echo "=== PGO: Training (${ITERATIONS} iterations) ==="
for i in $(seq 1 "${ITERATIONS}"); do
  echo "--- Iteration ${i}/${ITERATIONS} ---"
  "${BINARY}" --config "${CONFIG_DIR}/default.toml" &
  PID=$!
  sleep 3

  # Phase 1: Status ping (exercises handshake + status state machine)
  echo -ne '\x10\x00\xF0\x05\x09localhost\x63\xDD\x01' | nc -w2 127.0.0.1 25565 || true

  # Phase 2: Login + play simulation via Python (exercises full protocol)
  python3 -c "
import socket, struct, time, uuid as _uuid

def varint(v):
    buf = []
    while v >= 0x80:
        buf.append((v & 0x7F) | 0x80)
        v >>= 7
    buf.append(v & 0x7F)
    return bytes(buf)

def pack_string(s):
    b = s.encode('utf-8')
    return varint(len(b)) + b

def make_packet(packet_id, data=b''):
    payload = varint(packet_id) + data
    return varint(len(payload)) + payload

try:
    s = socket.socket()
    s.settimeout(5)
    s.connect(('127.0.0.1', 25565))

    # Handshake
    host = pack_string('localhost')
    handshake = struct.pack('>i', 776) + host + struct.pack('>H', 25565) + varint(2)
    s.send(make_packet(0x00, handshake))

    # Login Start
    name = pack_string('PGO_Bot')
    login = struct.pack('>16s', _uuid.uuid4().bytes) + name
    s.send(make_packet(0x00, login))

    # Read responses for 10s (exercises compression, chunk decoding, entity spawns)
    deadline = time.time() + 10
    while time.time() < deadline:
        try:
            s.recv(65536)
        except:
            break
    s.close()
except Exception as e:
    pass
" 2>/dev/null || true

  sleep 10

  # Phase 3: Multiple status pings in parallel (exercises multi-connection handling)
  for _ in $(seq 1 3); do
    echo -ne '\x10\x00\xF0\x05\x09localhost\x63\xDD\x01' | nc -w1 127.0.0.1 25565 2>/dev/null || true &
  done
  wait

  sleep 5
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
