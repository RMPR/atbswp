#!/bin/sh
# End-to-end: text script -> export with the Rust CLI -> standalone APE
# replays through libei into a real EIS server (no compositor required).
#
# Needs: player/build/player.com, player/build/eis_sink, cargo.
set -eu
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"
TMP=$(mktemp -d)
SINK_PID=
cleanup() { rm -rf "$TMP"; [ -n "$SINK_PID" ] && kill "$SINK_PID" 2>/dev/null || true; }
trap cleanup EXIT

cargo build -q -p atbswp-cli
ATBSWP=target/debug/atbswp

cat > "$TMP/macro.txt" <<'M'
screen 1920x1080
move 640 360
wait 20ms
click left
wait 20ms
keydown leftctrl
key a
keyup leftctrl
wait 10ms
scroll down 1
moverel 10 -10
M

$ATBSWP export "$TMP/macro.txt" -o "$TMP/macro.com"
test -s "$TMP/macro.com"

# The socket path must be absolute or relative to XDG_RUNTIME_DIR.
SOCK="$TMP/eis.sock"
player/build/eis_sink "$SOCK" --timeout 20 > "$TMP/sink.log" 2>&1 &
SINK_PID=$!
sleep 0.3

if ! LIBEI_SOCKET="$SOCK" ATBSWP_VERBOSE=1 "$TMP/macro.com" --speed 400 2> "$TMP/player.log"; then
	LIBEI_SOCKET="$SOCK" ATBSWP_VERBOSE=1 sh "$TMP/macro.com" --speed 400 2>> "$TMP/player.log"
fi
wait "$SINK_PID" || { echo "sink failed:"; cat "$TMP/sink.log" "$TMP/player.log"; exit 1; }
SINK_PID=

expect() {
	grep -qx "$1" "$TMP/sink.log" || { echo "missing in sink log: $1"; cat "$TMP/sink.log" "$TMP/player.log"; exit 1; }
}
expect "absolute 640 360"
expect "button 272 1"
expect "button 272 0"
expect "key 29 1"
expect "key 30 1"
expect "key 30 0"
expect "key 29 0"
expect "scroll_discrete 0 120"
expect "motion 10 -10"
expect "client disconnect"
test "$(grep -c '^frame$' "$TMP/sink.log")" -eq 9

# Playing back through `atbswp play` must produce the same events.
$ATBSWP play "$TMP/macro.txt" --dry-run --speed 1000 > "$TMP/dry"
test "$(wc -l < "$TMP/dry")" -eq 9

echo "e2e: OK"
