#!/bin/sh
# The pkexec helper (`atbswp record --stdout-raw`) must stream raw events
# and stop when its stdin is closed, which is how the GUI's Stop button
# reaches a root process it cannot signal.  A FIFO stands in for a device.
set -eu
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"
cargo build -q -p atbswp-cli
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
mkfifo "$TMP/event0" "$TMP/stdin"

ATBSWP_INPUT_DIR="$TMP" target/debug/atbswp record --stdout-raw --no-elevate < "$TMP/stdin" > "$TMP/out" 2> "$TMP/err" &
REC=$!
exec 3> "$TMP/stdin"          # hold the helper's stdin open
# one key press + release for KEY_A (30) and a SYN, as 24-byte input_events
python3 - "$TMP/event0" <<'PY'
import struct, sys, time
with open(sys.argv[1], "wb") as f:
    for code, val in ((30, 1), (0, 0), (30, 0), (0, 0)):
        typ = 1 if code else 0
        f.write(struct.pack("<qqHHi", 1, 0, typ, code, val))
    f.flush()
    time.sleep(0.3)
PY
sleep 0.3
exec 3>&-                     # close stdin: the helper must stop now
for _ in $(seq 50); do kill -0 $REC 2>/dev/null || break; sleep 0.1; done
if kill -0 $REC 2>/dev/null; then echo "helper did not stop when stdin closed"; kill $REC; cat "$TMP/err"; exit 1; fi
wait $REC || { echo "helper failed"; cat "$TMP/err"; exit 1; }
cat "$TMP/out"
grep -q "^K 1000000 30 1$" "$TMP/out"
grep -q "^K 1000000 30 0$" "$TMP/out"
grep -qx "END" "$TMP/out"
echo "e2e-helper-stop: OK"
