#!/usr/bin/env python3
"""Assert that a recording of tests/golden.txt (as dumped text) contains the
golden events in order.  Motion is coalesced and timing jitters, so only the
event sequence and the final pointer position are compared.

usage: check_recording.py DUMP.txt [--scale W H]   (default: the dump's `screen` line)
"""
import re
import sys

dump = open(sys.argv[1]).read()
scale = None
if len(sys.argv) >= 5 and sys.argv[2] == "--scale":
    scale = (int(sys.argv[3]), int(sys.argv[4]))

lines = [l.split("#")[0].strip() for l in dump.splitlines()]
events = [l for l in lines if l and not l.startswith(("screen", "repeat", "speed", "wait"))]
# the golden macro was recorded for 1024x768; a recorder on another screen
# sees the player's scaled coordinates, so scale the expectation the same way
for l in lines:
    if l.startswith("screen ") and scale is None:
        w, h = l.split()[1].split("x")
        scale = (int(w), int(h))

# expected sequence; motion may be split into several "move" lines, only the
# last one before the click matters.
expected = [
    ("move", None),
    ("buttondown left", None),
    ("buttonup left", None),
    ("keydown KEY_A", None),
    ("keyup KEY_A", None),
    ("keydown KEY_RIGHTCTRL", None),
    ("keyup KEY_RIGHTCTRL", None),
    ("keydown KEY_LEFT", None),
    ("keyup KEY_LEFT", None),
    ("scroll 0 120", None),
]

pos = 0
last_move = None
for want, _ in expected:
    while pos < len(events) and events[pos].startswith("move ") and want != "move":
        pos += 1  # stray motion between events is fine
    if want == "move":
        # consume all moves, remember the last
        while pos < len(events) and events[pos].startswith("move "):
            last_move = events[pos]
            pos += 1
        if last_move is None:
            sys.exit(f"no absolute move recorded before the click; events: {events}")
        continue
    if pos >= len(events) or events[pos] != want:
        sys.exit(f"expected `{want}` at event {pos}, got {events[pos:pos+1]}; all events: {events}")
    pos += 1

x, y = map(int, last_move.split()[1:3])
ex, ey = 200, 300
if scale:
    ex, ey = 200 * scale[0] // 1024, 300 * scale[1] // 768
if abs(x - ex) > 1 or abs(y - ey) > 1:
    sys.exit(f"pointer recorded at {x},{y}, expected {ex},{ey}")
print(f"recording OK: {len(events)} events, pointer {x},{y}")
