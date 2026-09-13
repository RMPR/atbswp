#!/bin/sh
# End-to-end through the XDG RemoteDesktop *portal* code path (libdbus):
# a fake portal on a private session bus hands the player an EIS socket;
# the EIS sink asserts the events.  Run twice to prove the restore token
# round-trips through $XDG_STATE_HOME.
#
# Needs: player/build/player.com, player/build/eis_sink, cargo,
#        python3-dbus, python3-gi, dbus-run-session.
set -eu
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"

if [ -z "${INSIDE_DBUS_SESSION:-}" ]; then
	exec env INSIDE_DBUS_SESSION=1 dbus-run-session -- sh "$0" "$@"
fi

TMP=$(mktemp -d)
PIDS=""
cleanup() { for p in $PIDS; do kill "$p" 2>/dev/null || true; done; rm -rf "$TMP"; }
trap cleanup EXIT

cargo build -q -p atbswp-cli
target/debug/atbswp export tests/golden.txt -o "$TMP/macro.com"

run_ape() { "$@" 2>>"$TMP/player.log" || sh "$@" 2>>"$TMP/player.log"; }

for round in 1 2; do
	echo "== round $round"
	: > "$TMP/portal.log"
	SOCK="$TMP/eis-$round.sock"
	player/build/eis_sink "$SOCK" --timeout 30 > "$TMP/sink-$round.log" 2>&1 &
	SINK=$!
	python3 tests/fake_portal.py "$SOCK" "$TMP/portal.log" > /dev/null 2>&1 &
	PORTAL=$!
	PIDS="$SINK $PORTAL"
	for _ in $(seq 50); do grep -q "fake portal ready" "$TMP/portal.log" 2>/dev/null && break; sleep 0.1; done
	grep -q "fake portal ready" "$TMP/portal.log" || { echo "fake portal did not start"; exit 1; }

	if ! XDG_STATE_HOME="$TMP/state" ATBSWP_VERBOSE=1 run_ape "$TMP/macro.com" --speed 400; then
		echo "player failed:"; cat "$TMP/player.log" "$TMP/portal.log"; exit 1
	fi
	wait "$SINK" || { echo "sink failed:"; cat "$TMP/sink-$round.log" "$TMP/player.log"; exit 1; }
	kill "$PORTAL" 2>/dev/null || true
	wait "$PORTAL" 2>/dev/null || true
	PIDS=""

	expect() { grep -q "$1" "$2" || { echo "missing in $2: $1"; cat "$TMP"/*.log; exit 1; }; }
	expect "CreateSession token=atbswp" "$TMP/portal.log"
	expect "types=3 persist_mode=2" "$TMP/portal.log"
	expect "Start session=/org/freedesktop/portal/desktop/session/fake/s1 parent=''" "$TMP/portal.log"
	expect "ConnectToEIS" "$TMP/portal.log"
	expect "^absolute 375 421" "$TMP/sink-$round.log"   # 200x300 scaled from 1024x768 to the sink's 1920x1080
	expect "^button 272 1" "$TMP/sink-$round.log"
	expect "^key 30 1" "$TMP/sink-$round.log"
	expect "^key 97 0" "$TMP/sink-$round.log"
	expect "^key 105 1" "$TMP/sink-$round.log"
	expect "^scroll_discrete 0 120" "$TMP/sink-$round.log"
	expect "^client disconnect" "$TMP/sink-$round.log"
	if [ "$round" = 1 ]; then
		expect "restore_token=$" "$TMP/portal.log"
		test "$(cat "$TMP/state/atbswp-portal-token")" = "fake-restore-token-42"
	else
		expect "restore_token=fake-restore-token-42" "$TMP/portal.log"
	fi
done
echo "e2e-portal: OK"
