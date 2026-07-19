#!/usr/bin/env python3
"""
Apply item.rs deduplication: remove first-occurrence duplicate m.insert() lines,
keeping only the LAST occurrence of each item name (matching HashMap behavior).

Also outputs an old_id→new_id mapping for recipe migration.
Usage: python3 scripts/dedup_apply.py [--dry-run]
"""
import re, sys

ITEM_RS = 'crates/mc-core/src/item.rs'
MAP_FILE = 'scripts/dup_id_map.txt'

def main():
    dry_run = '--dry-run' in sys.argv

    with open(ITEM_RS, 'r', encoding='utf-8') as f:
        lines = f.readlines()

    # Pass 1: find all m.insert calls in reverse to determine which to keep
    # Process in reverse: first time we see a name → keep (last in original)
    # Subsequent times → remove (earlier duplicates)
    keep_line = set()  # line indices to keep
    seen_names = {}    # name → (line_idx, id_val) — last occurrence in original
    remove_line = set()  # line indices to remove
    id_mapping = {}     # old_id → new_id

    insert_pattern = re.compile(r'm\.insert\("([^"]+)", (\d+)\)')

    # First pass: find last occurrences (scan forward, overwrite)
    for i, line in enumerate(lines):
        m = insert_pattern.search(line)
        if m:
            name = m.group(1)
            id_val = int(m.group(2))
            if name in seen_names:
                # This is a newer occurrence — old one should be removed
                old_idx, old_id = seen_names[name]
                remove_line.add(old_idx)
                id_mapping[old_id] = id_val
            seen_names[name] = (i, id_val)

    # All last-occurrence lines are kept by default
    for idx, _ in seen_names.values():
        keep_line.add(idx)

    print(f"Total m.insert lines: {len(seen_names) + len(remove_line)}")
    print(f"Unique names: {len(seen_names)}")
    print(f"Duplicate lines to remove: {len(remove_line)}")
    print(f"Old→new ID mappings for recipe migration: {len(id_mapping)}")

    if dry_run:
        print("\n--- Lines to remove (first 20) ---")
        for idx in sorted(remove_line)[:20]:
            print(f"  L{idx+1}: {lines[idx].rstrip()}")
        print("\n--- ID Mappings (first 20) ---")
        for old_id, new_id in list(sorted(id_mapping.items()))[:20]:
            print(f"  {old_id} → {new_id}")
        return

    # Write cleaned file
    with open(ITEM_RS, 'w', encoding='utf-8') as f:
        for i, line in enumerate(lines):
            if i not in remove_line:
                f.write(line)

    # Write ID mapping for recipe migration
    with open(MAP_FILE, 'w', encoding='utf-8') as f:
        for old_id, new_id in sorted(id_mapping.items()):
            f.write(f"{old_id} {new_id}\n")

    print(f"\nCleaned item.rs written ({len(remove_line)} duplicates removed)")
    print(f"ID mapping written to {MAP_FILE}")

if __name__ == '__main__':
    main()
