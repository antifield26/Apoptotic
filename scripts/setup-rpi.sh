#!/bin/bash
# Raspberry Pi 5 (8GB) — Minecraft Server Setup
# Applies jemalloc tuning and kernel optimizations for RPi 5.

set -euo pipefail

echo "== RPi 5 Minecraft Server Setup =="

# ── jemalloc tuning for 8GB RPi 5 ──
export MALLOC_CONF="background_thread:true,dirty_decay_ms:5000,muzzy_decay_ms:5000,narenas:4,lg_tcache_max:16,metadata_thp:always"
echo "MALLOC_CONF=$MALLOC_CONF"

# ── Kernel tuning ──
# Enable transparent hugepages (reduces TLB misses)
if [ -w /sys/kernel/mm/transparent_hugepage/enabled ]; then
    echo madvise | sudo tee /sys/kernel/mm/transparent_hugepage/enabled > /dev/null 2>&1 || true
    echo "THP: enabled (madvise)"
fi

# Increase max file descriptors
ulimit -n 65536 2>/dev/null || true

# ── Launch server ──
echo "Starting Minecraft server..."
exec ./mc-server "$@"
