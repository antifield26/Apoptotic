#!/bin/bash
# Raspberry Pi 5 (8GB) — Minecraft Server Setup
# Applies jemalloc tuning, CPU governor, and kernel optimizations for RPi 5.
#
# Usage:
#   source scripts/setup-rpi.sh    # export env vars
#   ./scripts/setup-rpi.sh         # launch server directly

set -euo pipefail

echo "== RPi 5 Minecraft Server Setup =="

# ── D3: CPU governor — set performance for consistent tick latency ──
for cpu in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do
    if [ -w "$cpu" ]; then
        echo performance | sudo tee "$cpu" > /dev/null 2>&1 || true
    fi
done
# Check current governor
CURRENT_GOV=$(cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor 2>/dev/null || echo "unknown")
echo "CPU governor: $CURRENT_GOV"

# ── jemalloc tuning for 8GB RPi 5 ──
export MALLOC_CONF="background_thread:true,dirty_decay_ms:5000,muzzy_decay_ms:5000,narenas:4,lg_tcache_max:16,metadata_thp:always"
echo "MALLOC_CONF=$MALLOC_CONF"

# ── Kernel tuning ──
# Enable transparent hugepages (reduces TLB misses for chunk store)
if [ -w /sys/kernel/mm/transparent_hugepage/enabled ]; then
    echo madvise | sudo tee /sys/kernel/mm/transparent_hugepage/enabled > /dev/null 2>&1 || true
    echo "THP: enabled (madvise)"
fi

# Increase max memory-mapped areas (needed by jemalloc)
if [ -w /proc/sys/vm/max_map_count ]; then
    sudo sysctl -w vm.max_map_count=262144 > /dev/null 2>&1 || true
    echo "vm.max_map_count: $(cat /proc/sys/vm/max_map_count)"
fi

# Increase max file descriptors
ulimit -n 65536 2>/dev/null || true

# ── D4: SD card optimization note ──
echo ""
echo "SD card optimization tips:"
echo "  - Set chunk_compression = 'zstd' in config for less write amplification"
echo "  - Set save_interval_ticks = 12000 to reduce write frequency"
echo "  - Use A2-class SD card or external SSD for world storage"
echo ""

# ── Launch server ──
echo "Starting Minecraft server..."
exec ./mc-server "$@"
