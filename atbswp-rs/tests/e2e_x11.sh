#!/bin/sh
# End-to-end on X11: play the golden macro through XTest inside Xvfb while
# `xinput test-xi2 --root` records the raw events the server delivered, then
# check the pointer landed where the macro said.
#
# usage: e2e_x11.sh MACRO.COM     (needs xvfb-run, xinput, xdotool)
set -eu
MACRO=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

xvfb-run -a -s "-screen 0 1024x768x24" sh -eu -c '
	TMP=$1; MACRO=$2
	xinput test-xi2 --root > "$TMP/xi2.log" 2>&1 &
	XI=$!
	sleep 0.5
	if ! ATBSWP_BACKEND=xtest ATBSWP_VERBOSE=1 "$MACRO" --speed 400 2> "$TMP/player.log"; then
		ATBSWP_BACKEND=xtest ATBSWP_VERBOSE=1 sh "$MACRO" --speed 400 2>> "$TMP/player.log"
	fi
	xdotool getmouselocation > "$TMP/mouse.log"
	sleep 0.3
	kill $XI 2>/dev/null || true
' sh "$TMP" "$MACRO"

# "EVENT type 13 (RawKeyPress)" ... "detail: 38"  ->  "RawKeyPress 38"
awk '/^EVENT type/ { t = $4; gsub(/[()]/, "", t) } /^ *detail:/ { print t, $2 }' "$TMP/xi2.log" > "$TMP/events"

expect() {
	grep -qx "$1" "$2" || { echo "missing: $1"; echo "--- events"; cat "$TMP/events"; echo "--- player"; cat "$TMP/player.log"; exit 1; }
}
grep -q "using XTest fallback" "$TMP/player.log" || { echo "XTest backend not used"; cat "$TMP/player.log"; exit 1; }
# X keycode = evdev + 8
expect "RawButtonPress 1" "$TMP/events"
expect "RawButtonRelease 1" "$TMP/events"
expect "RawKeyPress 38" "$TMP/events"       # KEY_A
expect "RawKeyRelease 38" "$TMP/events"
expect "RawKeyPress 105" "$TMP/events"      # KEY_RIGHTCTRL
expect "RawKeyPress 113" "$TMP/events"      # KEY_LEFT
expect "RawButtonPress 5" "$TMP/events"     # wheel down
grep -q "x:200 y:300" "$TMP/mouse.log" || { echo "pointer not at 200,300:"; cat "$TMP/mouse.log"; exit 1; }
echo "e2e-x11: OK"
