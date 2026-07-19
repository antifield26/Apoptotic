#!/usr/bin/env python3
"""
Minecraft Item Registry Generator
==================================
Extracts block/item IDs from PrismarineJS minecraft-data and generates
Rust code for crates/mc-core/src/item.rs.

Usage:
    # Download data for specific version
    npm install minecraft-data@latest
    node -e "const md=require('minecraft-data')('26.2');require('fs').writeFileSync('blocks.json',JSON.stringify(md.blocks))"

    # Or fetch directly from PrismarineJS CDN:
    curl -s https://unpkg.com/minecraft-data@3.111.0/data/pc/26.2/blocks.json > data/blocks.json
    curl -s https://unpkg.com/minecraft-data@3.111.0/data/pc/26.2/items.json > data/items.json

    # Generate Rust code:
    python3 scripts/extract_items.py --blocks data/blocks.json --items data/items.json \
        --version 26.2 --output crates/mc-core/src/item_generated.rs

    # Compare against existing registry:
    python3 scripts/extract_items.py --blocks data/blocks.json --items data/items.json \
        --version 26.2 --compare crates/mc-core/src/item.rs --output report.txt

Version: Minecraft 26.2 (protocol 776, data version 4903)
"""

import json
import argparse
import sys
import os
from collections import OrderedDict
from typing import Dict, List, Tuple

def load_json(path: str) -> dict:
    with open(path, 'r', encoding='utf-8') as f:
        return json.load(f)

def extract_block_ids(blocks_json: list) -> Dict[str, int]:
    """Extract name→ID mapping from PrismarineJS blocks.json array."""
    result = OrderedDict()
    for block in blocks_json:
        name = block['name'].replace('minecraft:', '')
        result[name] = block['id']
    return result

def extract_item_ids(items_json: list) -> Dict[str, int]:
    """Extract name→ID mapping from PrismarineJS items.json array."""
    result = OrderedDict()
    for item in items_json:
        name = item['name'].replace('minecraft:', '')
        result[name] = item['id']
    return result

def parse_existing_registry(path: str) -> Dict[str, int]:
    """Parse m.insert() calls from existing item.rs."""
    result = OrderedDict()
    with open(path, 'r', encoding='utf-8') as f:
        for line in f:
            line = line.strip()
            if line.startswith('m.insert('):
                # Extract: m.insert("name", id);
                try:
                    parts = line.split('"')
                    if len(parts) >= 3:
                        name = parts[1]
                        id_str = parts[2].split(')')[0].strip().strip(',')
                        id_val = int(id_str)
                        result[name] = id_val
                except (ValueError, IndexError):
                    pass
    return result

def generate_rust_code(blocks: Dict[str, int], items: Dict[str, int],
                       version: str) -> str:
    """Generate Rust code for ITEM_REGISTRY HashMap."""
    lines = []
    lines.append(f"//! 物品名称注册表 — 自动生成自 PrismarineJS minecraft-data {version}")
    lines.append(f"//!")
    lines.append(f"//! 生成时间: auto-generated")
    lines.append(f"//! 方块: {len(blocks)} entries, 物品: {len(items)} entries")
    lines.append("")
    lines.append("use std::collections::HashMap;")
    lines.append("use std::sync::LazyLock;")
    lines.append("")
    lines.append(f"/// Minecraft {version} 物品名 → protocol ID 映射")
    lines.append(f"/// 数据源: PrismarineJS/minecraft-data data/pc/{version}/")
    lines.append(f"static ITEM_REGISTRY: LazyLock<HashMap<&'static str, u32>> = LazyLock::new(|| {{")
    total = len(blocks) + len(items)
    lines.append(f"    let mut m = HashMap::with_capacity({total});")
    lines.append("")

    # Write blocks section
    lines.append("    // === BLOCKS ===")
    for name, block_id in blocks.items():
        lines.append(f'    m.insert("{name}", {block_id});')

    # Write items section (only items not already in blocks)
    lines.append("")
    lines.append("    // === ITEMS ===")
    for name, item_id in items.items():
        if name not in blocks:
            lines.append(f'    m.insert("{name}", {item_id});')

    lines.append("")
    lines.append("    m")
    lines.append("});")
    lines.append("")
    lines.append("// Generated helper functions...")

    return '\n'.join(lines)

def compare_registries(existing: Dict[str, int], official: Dict[str, int],
                        label: str) -> List[str]:
    """Compare existing registry against official data, return report lines."""
    report = []
    report.append(f"\n### {label}")
    report.append(f"Official entries: {len(official)}")
    report.append(f"Existing entries: {len(existing)}")

    # IDs that exist but with wrong names (ID collision check)
    existing_by_id: Dict[int, str] = {}
    for name, id_val in existing.items():
        if id_val in existing_by_id:
            report.append(f"  COLLISION: '{existing_by_id[id_val]}' and '{name}' both use ID {id_val}")
        existing_by_id[id_val] = name

    # Missing entries
    missing = []
    for name, official_id in official.items():
        if name not in existing:
            missing.append(name)

    # ID mismatches (same name, different ID)
    mismatches = []
    for name, existing_id in existing.items():
        if name in official and existing_id != official[name]:
            mismatches.append((name, existing_id, official[name]))

    if missing:
        report.append(f"Missing ({len(missing)}):")
        # Show first 30 and last 5
        if len(missing) <= 35:
            for name in missing:
                report.append(f"  - {name} (official ID: {official[name]})")
        else:
            for name in missing[:30]:
                report.append(f"  - {name} (official ID: {official[name]})")
            report.append(f"  ... and {len(missing) - 35} more ...")
            for name in missing[-5:]:
                report.append(f"  - {name} (official ID: {official[name]})")

    if mismatches:
        report.append(f"ID mismatches ({len(mismatches)}):")
        for name, exist_id, off_id in mismatches:
            report.append(f"  {name}: existing={exist_id}, official={off_id}")

    if not missing and not mismatches:
        report.append("✓ All entries match official data")

    return report

def main():
    parser = argparse.ArgumentParser(
        description='Generate Minecraft item registry from PrismarineJS data')
    parser.add_argument('--blocks', help='Path to blocks.json')
    parser.add_argument('--items', help='Path to items.json')
    parser.add_argument('--version', default='26.2', help='Minecraft version')
    parser.add_argument('--output', help='Output path for generated Rust code')
    parser.add_argument('--compare', help='Path to existing item.rs for comparison')
    args = parser.parse_args()

    if not args.blocks or not args.items:
        print("Error: --blocks and --items are required")
        print("Download data first:")
        print("  npm install minecraft-data@latest")
        print("  node -e \"const md=require('minecraft-data')('26.2');"
              "require('fs').writeFileSync('blocks.json',JSON.stringify(md.blocks))\"")
        sys.exit(1)

    if not os.path.exists(args.blocks):
        print(f"Error: {args.blocks} not found")
        sys.exit(1)
    if not os.path.exists(args.items):
        print(f"Error: {args.items} not found")
        sys.exit(1)

    blocks_data = load_json(args.blocks)
    items_data = load_json(args.items)

    if isinstance(blocks_data, list):
        blocks = extract_block_ids(blocks_data)
    else:
        print("Error: blocks.json should be a JSON array")
        sys.exit(1)

    if isinstance(items_data, list):
        items = extract_item_ids(items_data)
    else:
        print("Error: items.json should be a JSON array")
        sys.exit(1)

    print(f"Loaded {len(blocks)} blocks and {len(items)} items from PrismarineJS data")

    if args.compare:
        existing = parse_existing_registry(args.compare)
        print(f"Parsed {len(existing)} entries from {args.compare}")

        # Merge blocks+items for comparison
        official = OrderedDict()
        for name, id_val in blocks.items():
            official[name] = id_val
        for name, id_val in items.items():
            if name not in official:
                official[name] = id_val

        report = compare_registries(existing, official, "Registry Comparison")
        report_text = '\n'.join(report)
        print(report_text)

        if args.output:
            with open(args.output, 'w', encoding='utf-8') as f:
                f.write(report_text)
            print(f"\nReport saved to {args.output}")

    if args.output and not args.compare:
        code = generate_rust_code(blocks, items, args.version)
        with open(args.output, 'w', encoding='utf-8') as f:
            f.write(code)
        print(f"Generated {len(blocks) + len(items)} entries → {args.output}")

if __name__ == '__main__':
    main()
