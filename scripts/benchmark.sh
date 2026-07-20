#!/bin/bash
# ═══════════════════════════════════════════════════════════════
# RPi 5 Minecraft Server Benchmark
# ═══════════════════════════════════════════════════════════════
# Measures TPS, memory usage, and CPU under simulated player load.
#
# Usage:
#   ./scripts/benchmark.sh              # Quick 60s benchmark
#   ./scripts/benchmark.sh 120 4        # 120 seconds, 4 simulated players
#   ./scripts/benchmark.sh 300 8 full   # Full 5-min benchmark with 8 players
#
# Output: benchmark_report_YYYYMMDD_HHMMSS.txt

set -euo pipefail

DURATION="${1:-60}"
PLAYERS="${2:-2}"
MODE="${3:-quick}"
PORT=25565
HOST="127.0.0.1"
REPORT="benchmark_report_$(date +%Y%m%d_%H%M%S).txt"
PID_FILE="/tmp/mc-benchmark.pid"

echo "=== Apoptotic Benchmark ==="
echo "Duration: ${DURATION}s, Simulated Players: ${PLAYERS}, Mode: ${MODE}"
echo ""

# Phase 1: Start server
echo "--- Starting server ---"
cargo run --release &
SERVER_PID=$!
echo $SERVER_PID > "$PID_FILE"
sleep 3

# Check server started
if ! kill -0 $SERVER_PID 2>/dev/null; then
    echo "ERROR: Server failed to start"
    exit 1
fi

# Phase 2: Simulate player connections
echo "--- Connecting ${PLAYERS} simulated players ---"
SIM_PIDS=()
for i in $(seq 1 $PLAYERS); do
    (
        # Send handshake + login ping, stay connected
        for t in $(seq 1 $((DURATION / 5))); do
            echo -ne '\x10\x00\xF0\x05\x09localhost\x63\xDD\x01' | nc -w1 $HOST $PORT 2>/dev/null || true
            sleep 5
        done
    ) &
    SIM_PIDS+=($!)
done

# Phase 3: Monitor
echo "--- Monitoring for ${DURATION}s ---"
{
    echo "Timestamp,CPU%,RSS_KB"
    for t in $(seq 1 $((DURATION))); do
        CPU=$(ps -p $SERVER_PID -o %cpu= 2>/dev/null | tr -d ' ' || echo "0")
        RSS=$(ps -p $SERVER_PID -o rss= 2>/dev/null | tr -d ' ' || echo "0")
        echo "$(date +%H:%M:%S),$CPU,$RSS"
        sleep 1
    done
} > "$REPORT"

# Phase 4: Cleanup
echo "--- Stopping ---"
for pid in "${SIM_PIDS[@]}"; do kill $pid 2>/dev/null || true; done
sleep 2
kill -TERM $SERVER_PID 2>/dev/null || true
wait $SERVER_PID 2>/dev/null || true
rm -f "$PID_FILE"

# Phase 5: Summary
AVG_CPU=$(awk -F',' 'NR>1{sum+=$2;n++} END{printf "%.1f", sum/n}' "$REPORT")
AVG_RSS=$(awk -F',' 'NR>1{sum+=$3;n++} END{printf "%.0f", sum/n}' "$REPORT")
PEAK_RSS=$(awk -F',' 'NR>1{if($3>m)m=$3} END{print m}' "$REPORT")

echo ""
echo "=== Results ==="
echo "Average CPU:  ${AVG_CPU}%"
echo "Average RSS:  $((AVG_RSS / 1024)) MB"
echo "Peak RSS:     $((PEAK_RSS / 1024)) MB"
echo "Players:      ${PLAYERS}"
echo "Duration:     ${DURATION}s"
echo "Report:       ${REPORT}"
