#!/usr/bin/env python3
"""Compare `pw_abi_check` output with the constants in record/pw.rs."""
import re
import subprocess
import sys
from pathlib import Path

root = Path(__file__).resolve().parents[1]
rust = (root / "crates/atbswp-core/src/record/pw.rs").read_text()
consts = {m.group(1): int(m.group(2).replace("_", ""), 0)
          for m in re.finditer(r"pub const (\w+): (?:u32|usize) = (0x[0-9A-Fa-f_]+|[0-9_]+);", rust)}
expected_layout = {
    "offsetof_pw_stream_events_param_changed": 40,
    "offsetof_pw_stream_events_process": 64,
    "offsetof_spa_buffer_metas": 8,
    "offsetof_spa_meta_data": 8,
    "offsetof_spa_meta_cursor_position": 8,
    "sizeof_pw_buffer": 40,
}
out = subprocess.run([sys.argv[1]], capture_output=True, text=True, check=True).stdout
bad = 0
for line in out.splitlines():
    name, val = line.split()
    val = int(val)
    want = consts.get(name, expected_layout.get(name))
    if want is None:
        print(f"?? {name} not in pw.rs")
        bad += 1
    elif want != val:
        print(f"MISMATCH {name}: rust {want}, headers {val}")
        bad += 1
print("pw-abi: OK" if not bad else f"pw-abi: {bad} problems")
sys.exit(1 if bad else 0)
