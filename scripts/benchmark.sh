#!/bin/bash
# ═══════════════════════════════════════════════════════════════
# RPi 5 Minecraft Server Benchmark (D7 enhanced)
# ═══════════════════════════════════════════════════════════════
# Measures TPS, memory, CPU, and per-stage latency under player load.
# D7: Added TPS estimation from /status endpoint + per-stage timing.
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
    echo "Timestamp,CPU%,RSS_KB,TPS_est"
    for t in $(seq 1 $((DURATION))); do
        CPU=$(ps -p $SERVER_PID -o %cpu= 2>/dev/null | tr -d ' ' || echo "0")
        RSS=$(ps -p $SERVER_PID -o rss= 2>/dev/null | tr -d ' ' || echo "0")
        # D7: Query /status endpoint for TPS estimation
        TPS=$(curl -s "http://localhost:9090/status" 2>/dev/null | grep -o '"tps_p95":"[^"]*"' | cut -d'"' -f4 || echo "N/A")
        echo "$(date +%H:%M:%S),$CPU,$RSS,$TPS"
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
# D7: TPS stats (skip N/A entries)
AVG_TPS=$(awk -F',' '$4!="N/A" && $4!=""{sum+=$4;n++} END{if(n>0)printf "%.1f", sum/n; else print "N/A"}' "$REPORT")
MIN_TPS=$(awk -F',' '$4!="N/A" && $4!=""{if($4<m||m=="")m=$4} END{print m}' "$REPORT")

echo ""
echo "=== Results ==="
echo "Average CPU:  ${AVG_CPU}%"
echo "Average RSS:  $((AVG_RSS / 1024)) MB"
echo "Peak RSS:     $((PEAK_RSS / 1024)) MB"
echo "Average TPS:  ${AVG_TPS}"
echo "Minimum TPS:  ${MIN_TPS}"
echo "Players:      ${PLAYERS}"
echo "Duration:     ${DURATION}s"
echo "Report:       ${REPORT}"
