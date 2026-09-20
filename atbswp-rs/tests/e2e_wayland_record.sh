#!/bin/sh
# Wayland recorder end-to-end without a compositor: a fake ScreenCast portal
# hands `atbswp record` a PipeWire node whose buffers carry scripted cursor
# metadata, while raw key/button events are fed through a FIFO in the
# pkexec helper's line format.  The result must match the golden macro.
#
# Needs: cargo, a PipeWire daemon (started here if PIPEWIRE_RUNTIME_DIR is
# unset and none is running), tests/pw_cursor_src built (libpipewire-0.3-dev),
# python3-dbus, python3-gi, dbus-run-session.
set -eu
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"
if [ -z "${INSIDE_DBUS_SESSION:-}" ]; then
	exec env INSIDE_DBUS_SESSION=1 dbus-run-session -- sh "$0" "$@"
fi
SRC=${PW_CURSOR_SRC:-player/build/pw_cursor_src}
test -x "$SRC" || { echo "build $SRC first (make -C player build/pw_cursor_src)"; exit 1; }

TMP=$(mktemp -d)
PIDS=""
cleanup() { for p in $PIDS; do kill "$p" 2>/dev/null || true; done; rm -rf "$TMP"; }
trap cleanup EXIT

# A PipeWire daemon: use the running one, else start a private one.
if [ ! -S "${PIPEWIRE_RUNTIME_DIR:-${XDG_RUNTIME_DIR:-/nonexistent}}/${PIPEWIRE_REMOTE:-pipewire-0}" ]; then
	export PIPEWIRE_RUNTIME_DIR="$TMP/pw"
	mkdir -p "$PIPEWIRE_RUNTIME_DIR"
	pipewire > "$TMP/pipewire.log" 2>&1 &
	PIDS="$PIDS $!"
	for _ in $(seq 50); do [ -S "$PIPEWIRE_RUNTIME_DIR/pipewire-0" ] && break; sleep 0.1; done
	[ -S "$PIPEWIRE_RUNTIME_DIR/pipewire-0" ] || { echo "pipewire did not start"; cat "$TMP/pipewire.log"; exit 1; }
fi

cargo build -q -p atbswp-cli

# Cursor path: 100,100 for the first frames, then 200,300 and hold (20 fps).
"$SRC" 100,100 100,100 100,100 150,200 200,300 200,300 > "$TMP/src.log" 2>&1 &
PIDS="$PIDS $!"
for _ in $(seq 100); do grep -q "^node " "$TMP/src.log" 2>/dev/null && break; sleep 0.1; done
NODE=$(grep "^node " "$TMP/src.log" | awk '{print $2}')
[ -n "$NODE" ] || { echo "cursor source did not register"; cat "$TMP/src.log"; exit 1; }
echo "cursor source node $NODE"

python3 tests/fake_portal.py /nonexistent "$TMP/portal.log" "$NODE" > "$TMP/portal.out" 2>&1 &
PIDS="$PIDS $!"
for _ in $(seq 50); do grep -q "fake portal ready" "$TMP/portal.log" 2>/dev/null && break; sleep 0.1; done
grep -q "fake portal ready" "$TMP/portal.log" || { echo "fake portal did not start:"; cat "$TMP/portal.out"; exit 1; }

mkfifo "$TMP/raw"
XDG_STATE_HOME="$TMP/state" target/debug/atbswp record --raw-from "$TMP/raw" -o "$TMP/rec.txt" 2> "$TMP/rec.log" &
REC=$!
PIDS="$PIDS $REC"

# Raw events on CLOCK_MONOTONIC, paced so the cursor has settled at 200,300.
python3 - "$TMP/raw" <<'PY'
import sys, time
f = open(sys.argv[1], "w")
def ev(kind, a, b):
    f.write(f"{kind} {int(time.monotonic() * 1e6)} {a} {b}\n"); f.flush()
time.sleep(1.5)                 # cursor path done, stream negotiated
ev("B", 0x110, 1); time.sleep(0.02); ev("B", 0x110, 0)
time.sleep(0.05)
ev("K", 30, 1); time.sleep(0.01); ev("K", 30, 0)      # a
ev("K", 97, 1); time.sleep(0.01); ev("K", 97, 0)      # rightctrl
ev("K", 105, 1); time.sleep(0.01); ev("K", 105, 0)    # left
time.sleep(0.05)
ev("S", 0, 120)
time.sleep(0.05)
f.write("END\n"); f.close()
PY
wait "$REC" || { echo "recorder failed:"; cat "$TMP/rec.log" "$TMP/portal.log"; exit 1; }
PIDS=$(echo "$PIDS" | sed "s/ $REC//")
cat "$TMP/rec.log"
cat "$TMP/rec.txt"
grep -q "screen 1024x768" "$TMP/rec.txt" || { echo "stream size not recorded"; exit 1; }
grep -q "SC SelectSources types=1 cursor_mode=4 persist_mode=2" "$TMP/portal.log"
test "$(cat "$TMP/state/atbswp-screencast-token")" = "fake-sc-token"
python3 tests/check_recording.py "$TMP/rec.txt"
echo "e2e-wayland-record: OK"
