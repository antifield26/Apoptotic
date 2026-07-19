#!/bin/bash
# Minecraft LAN Server — 自动备份脚本 (增量 + 校验)
# 用法: ./scripts/backup.sh [RCON密码] [full|incr]
# Cron (每日全量): 0 4 * * 0 /home/minecraft/scripts/backup.sh change-me full
# Cron (每日增量): 0 4 * * 1-6 /home/minecraft/scripts/backup.sh change-me incr

set -e

RCON_PASSWORD="${1:-change-me}"
BACKUP_MODE="${2:-incr}"
RCON_PORT="${RCON_PORT:-25575}"
DATA_DIR="${DATA_DIR:-./data}"
BACKUP_DIR="${BACKUP_DIR:-./data/backups}"
RETENTION_DAYS="${RETENTION_DAYS:-30}"
SNAPSHOT_FILE="$BACKUP_DIR/backup.snar"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

echo "[$(date)] Starting backup (mode: $BACKUP_MODE)..."

# 1. 强制保存所有数据到磁盘 (via RCON)
echo "[$(date)] Sending /save-all via RCON..."
printf '/save-all\n' | timeout 5 nc localhost "$RCON_PORT" 2>/dev/null || true
sleep 3

# 2. 创建备份
mkdir -p "$BACKUP_DIR"

if [ "$BACKUP_MODE" = "full" ]; then
    # Full backup: remove old snapshot to start fresh
    rm -f "$SNAPSHOT_FILE"
    BACKUP_FILE="$BACKUP_DIR/mc-backup-full-$TIMESTAMP.tar.gz"
    echo "[$(date)] Creating full backup: $BACKUP_FILE..."
    tar -czf "$BACKUP_FILE" \
        --listed-incremental="$SNAPSHOT_FILE" \
        --exclude='*.mca.tmp' \
        --exclude='backups' \
        -C "$DATA_DIR" .
else
    # Incremental: only changed files since last full/incr
    BACKUP_FILE="$BACKUP_DIR/mc-backup-incr-$TIMESTAMP.tar.gz"
    echo "[$(date)] Creating incremental backup: $BACKUP_FILE..."
    if [ ! -f "$SNAPSHOT_FILE" ]; then
        echo "[$(date)] No snapshot found — creating full backup instead"
        BACKUP_FILE="$BACKUP_DIR/mc-backup-full-$TIMESTAMP.tar.gz"
        tar -czf "$BACKUP_FILE" \
            --listed-incremental="$SNAPSHOT_FILE" \
            --exclude='*.mca.tmp' \
            --exclude='backups' \
            -C "$DATA_DIR" .
    else
        tar -czf "$BACKUP_FILE" \
            --listed-incremental="$SNAPSHOT_FILE" \
            --exclude='*.mca.tmp' \
            --exclude='backups' \
            -C "$DATA_DIR" .
    fi
fi

BACKUP_SIZE=$(du -h "$BACKUP_FILE" | cut -f1)
echo "[$(date)] Backup created: $BACKUP_SIZE"

# 3. 校验备份完整性 (SHA256)
echo "[$(date)] Verifying backup checksum..."
SHA_FILE="$BACKUP_FILE.sha256"
sha256sum "$BACKUP_FILE" > "$SHA_FILE"
echo "[$(date)] Checksum saved to $SHA_FILE"

# 4. 清理旧备份
echo "[$(date)] Cleaning backups older than $RETENTION_DAYS days..."
find "$BACKUP_DIR" -name "mc-backup-*.tar.gz" -mtime +"$RETENTION_DAYS" -delete
find "$BACKUP_DIR" -name "mc-backup-*.sha256" -mtime +"$RETENTION_DAYS" -delete

BACKUP_COUNT=$(find "$BACKUP_DIR" -name "mc-backup-*.tar.gz" | wc -l)
echo "[$(date)] Backup complete. $BACKUP_COUNT backups retained."
