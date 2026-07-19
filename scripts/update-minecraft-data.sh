#!/bin/bash
# ═══════════════════════════════════════════════════════════════
# Minecraft Data Updater — 从 PrismarineJS 同步官方数据
# ═══════════════════════════════════════════════════════════════
# 从 PrismarineJS/minecraft-data 拉取最新官方方块/物品/配方数据,
# 生成 Rust item.rs 注册代码和差异报告。
#
# 用法:
#   ./scripts/update-minecraft-data.sh 26.2
#   ./scripts/update-minecraft-data.sh 26.2 --apply   # 自动覆盖 item.rs
#   ./scripts/update-minecraft-data.sh latest         # 使用最新版本
#
# 依赖: node, npm, python3

set -euo pipefail

VERSION="${1:-26.2}"
APPLY="${2:-}"
DATA_DIR="./data/minecraft-data"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
ITEM_RS="${PROJECT_ROOT}/crates/mc-core/src/item.rs"
EXTRACTOR="${SCRIPT_DIR}/extract_items.py"

echo "=== Minecraft Data Updater ==="
echo "Version: ${VERSION}"
echo "Data dir: ${DATA_DIR}"

# Phase 1: Ensure minecraft-data is available
mkdir -p "${DATA_DIR}"
if [ ! -f "${DATA_DIR}/package.json" ]; then
    echo "--- Installing minecraft-data ---"
    cd "${DATA_DIR}"
    npm init -y > /dev/null 2>&1
    npm install minecraft-data@latest > /dev/null 2>&1
    cd "${PROJECT_ROOT}"
    echo "minecraft-data installed"
fi

# Phase 2: Extract data via Node.js
echo "--- Extracting data for version ${VERSION} ---"
node -e "
const mcData = require('${DATA_DIR}/node_modules/minecraft-data')('${VERSION}');
const fs = require('fs');
if (!mcData || !mcData.blocks) {
    console.error('Version ${VERSION} not found in minecraft-data');
    console.error('Available versions:', Object.keys(require('${DATA_DIR}/node_modules/minecraft-data').supportedVersions));
    process.exit(1);
}
fs.writeFileSync('${DATA_DIR}/blocks_${VERSION}.json', JSON.stringify(mcData.blocks, null, 2));
fs.writeFileSync('${DATA_DIR}/items_${VERSION}.json', JSON.stringify(mcData.items, null, 2));
console.log('Blocks: ' + mcData.blocks.length + ' entries');
console.log('Items: ' + mcData.items.length + ' entries');
if (mcData.biomes) {
    fs.writeFileSync('${DATA_DIR}/biomes_${VERSION}.json', JSON.stringify(mcData.biomes, null, 2));
    console.log('Biomes: ' + mcData.biomes.length + ' entries');
}
if (mcData.entities) {
    fs.writeFileSync('${DATA_DIR}/entities_${VERSION}.json', JSON.stringify(mcData.entities, null, 2));
    console.log('Entity types: ' + mcData.entities.length + ' entries');
}
if (mcData.effects) {
    fs.writeFileSync('${DATA_DIR}/effects_${VERSION}.json', JSON.stringify(mcData.effects, null, 2));
    console.log('Effects: ' + mcData.effects.length + ' entries');
}
if (mcData.foods) {
    fs.writeFileSync('${DATA_DIR}/foods_${VERSION}.json', JSON.stringify(mcData.foods, null, 2));
    console.log('Foods: ' + mcData.foods.length + ' entries');
}
console.log('Data extracted to ${DATA_DIR}/');
"

echo ""

# Phase 3: Generate comparison report
echo "--- Comparing with existing item.rs ---"
python3 "${EXTRACTOR}" \
    --blocks "${DATA_DIR}/blocks_${VERSION}.json" \
    --items "${DATA_DIR}/items_${VERSION}.json" \
    --version "${VERSION}" \
    --compare "${ITEM_RS}" \
    --output "${DATA_DIR}/diff_report_${VERSION}.txt" 2>&1 || true

# Phase 4: Optionally generate Rust code
if [ "${APPLY}" = "--apply" ]; then
    echo ""
    echo "--- Generating item_gen.rs ---"
    python3 "${EXTRACTOR}" \
        --blocks "${DATA_DIR}/blocks_${VERSION}.json" \
        --items "${DATA_DIR}/items_${VERSION}.json" \
        --version "${VERSION}" \
        --output "${PROJECT_ROOT}/crates/mc-core/src/item_gen.rs"
    echo ""
    echo "Generated: crates/mc-core/src/item_gen.rs"
    echo ""
    echo "=== NEXT STEPS ==="
    echo "1. Review item_gen.rs and merge into item.rs"
    echo "2. Update item.rs to use the generated ITEM_REGISTRY"
    echo "3. Run: cargo test test_recipe_result_items_exist"
    echo "4. Fix any remaining recipe result ID gaps"
else
    echo ""
    echo "=== NEXT STEPS ==="
    echo "Review the diff report: cat ${DATA_DIR}/diff_report_${VERSION}.txt"
    echo "To apply changes: ./scripts/update-minecraft-data.sh ${VERSION} --apply"
fi

echo ""
echo "=== Done! ==="
