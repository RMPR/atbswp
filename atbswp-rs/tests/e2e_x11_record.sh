#!/bin/sh
# Record while the golden macro plays through XTest inside Xvfb; the
# recorder must reproduce the golden sequence.  The macro ends with F12,
# which is also the recorder's stop key.
#   e2e_x11_record.sh ATBSWP GOLDEN.COM
set -eu
ATBSWP=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
MACRO=$(cd "$(dirname "$2")" && pwd)/$(basename "$2")
HERE=$(cd "$(dirname "$0")" && pwd)
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

xvfb-run -a -s "-screen 0 1024x768x24" sh -eu -c '
	TMP=$1; ATBSWP=$2; MACRO=$3
	ATBSWP_RECORDER=x11 "$ATBSWP" record -o "$TMP/rec.txt" 2> "$TMP/rec.log" &
	REC=$!
	sleep 1.5
	if ! ATBSWP_BACKEND=xtest "$MACRO" --speed 400 2> "$TMP/player.log"; then
		ATBSWP_BACKEND=xtest sh "$MACRO" --speed 400 2>> "$TMP/player.log"
	fi
	# the macro ends by pressing F12, which stops the recorder
	for _ in $(seq 100); do kill -0 $REC 2>/dev/null || break; sleep 0.1; done
	if kill -0 $REC 2>/dev/null; then echo "recorder did not stop on F12"; kill $REC; cat "$TMP/rec.log"; exit 1; fi
	wait $REC || { echo "recorder failed"; cat "$TMP/rec.log" "$TMP/player.log"; exit 1; }
' sh "$TMP" "$ATBSWP" "$MACRO"

cat "$TMP/rec.txt"
python3 "$HERE/check_recording.py" "$TMP/rec.txt"
echo "e2e-x11-record: OK"
