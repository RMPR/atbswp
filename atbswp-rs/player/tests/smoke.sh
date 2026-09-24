#!/bin/sh
# Display-free checks for the player: payload parsing, dump, dry-run timing.
# usage: smoke.sh PLAYER   (PLAYER may be player.com or player-native)
set -eu
PLAYER=${1:-build/player.com}
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

# Build a payload by hand (python3 is available on every CI runner) so this
# test does not depend on the Rust side.
python3 - "$TMP/payload" "$PLAYER" "$TMP/macro.com" <<'PY'
import struct, sys
ev = [(1, 0, 100, 200, 0), (3, 0x110, 0, 0, 20000), (4, 0x110, 0, 0, 20000),
      (5, 30, 0, 0, 10000), (6, 30, 0, 0, 10000), (7, 0, 0, 240, 10000), (2, 0, -5, 7, 10000)]
hdr = struct.pack('<HHIIIIIII', 1, 0, len(ev), 1920, 1080, 1, 100, 0, 0)
body = b''.join(struct.pack('<HHiiI', *e) for e in ev)
open(sys.argv[1], 'wb').write(hdr + body)
import shutil, zipfile
shutil.copyfile(sys.argv[2], sys.argv[3])
with zipfile.ZipFile(sys.argv[3], 'a', zipfile.ZIP_STORED) as z:
    z.writestr('macro.bin', hdr + body)
PY
chmod +x "$TMP/macro.com"

run() { # run an APE even when the kernel lacks an APE binfmt handler
	"$@" 2>/dev/null || sh "$@"
}

echo "== version"
run "$PLAYER" --version | grep -q "atbswp player"

echo "== bare player refuses to play"
if run "$PLAYER" >/dev/null 2>&1; then echo "expected failure"; exit 1; fi

echo "== dump from --payload"
run "$PLAYER" --payload "$TMP/payload" --dump > "$TMP/dump1"
grep -qx "move 100 200" "$TMP/dump1"
grep -qx "buttondown 272" "$TMP/dump1"
grep -qx "scroll 0 240" "$TMP/dump1"
grep -qx "moverel -5 7" "$TMP/dump1"
test "$(grep -c '^wait' "$TMP/dump1")" -eq 6

echo "== dump from embedded payload"
run "$TMP/macro.com" --dump > "$TMP/dump2"
cmp "$TMP/dump1" "$TMP/dump2"

echo "== dry run emits every event in order"
run "$TMP/macro.com" --dry-run --speed 1000 > "$TMP/dry"
test "$(wc -l < "$TMP/dry")" -eq 7
grep -v '^wait' "$TMP/dump1" | grep -v '^#' | cmp - "$TMP/dry"

echo "== repeat is honoured"
run "$TMP/macro.com" --dry-run --speed 1000 --repeat 3 > "$TMP/dry3"
test "$(wc -l < "$TMP/dry3")" -eq 21

echo "== timing: 80ms of waits at 100% takes >= 80ms"
start=$(date +%s%N); run "$TMP/macro.com" --dry-run >/dev/null; end=$(date +%s%N)
elapsed=$(( (end - start) / 1000000 ))
test "$elapsed" -ge 80 || { echo "too fast: ${elapsed}ms"; exit 1; }
echo "   took ${elapsed}ms"

echo "smoke: OK"
