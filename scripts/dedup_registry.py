#!/usr/bin/env python3
"""Find and report duplicate entries in item.rs registry."""
import re
import sys

path = sys.argv[1] if len(sys.argv) > 1 else 'crates/mc-core/src/item.rs'
with open(path, 'r') as f:
    content = f.read()

pattern = r'm\.insert\(\"([^\"]+)\", (\d+)\)'
matches = re.findall(pattern, content)
seen = {}
dupes = []
for name, id_val in matches:
    if name in seen:
        dupes.append((name, seen[name], id_val))
    seen[name] = id_val

print(f'Total entries: {len(matches)}')
print(f'Unique names: {len(seen)}')
print(f'Duplicate names: {len(dupes)}')
print()
for name, old_id, new_id in dupes:
    print(f'  {name}: {old_id} → {new_id}')

# Also find ID collisions (different names, same ID)
id_to_names = {}
for name, id_val in matches:
    if id_val not in id_to_names:
        id_to_names[id_val] = []
    id_to_names[id_val].append(name)

collisions = {k: v for k, v in id_to_names.items() if len(v) > 1}
print(f'\nID collisions (same ID, different names): {len(collisions)}')
# Show first 20
for id_val, names in list(sorted(collisions.items()))[:20]:
    print(f'  ID {id_val}: {names}')
if len(collisions) > 20:
    print(f'  ... and {len(collisions) - 20} more')
