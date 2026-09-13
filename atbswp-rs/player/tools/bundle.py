#!/usr/bin/env python3
"""Append (or replace) the native Intel-macOS helper behind an APE player.

Layout: player | helper | u64 LE helper_len | b"ATBSWPH1"
The macro payload footer ("ATBSWPM1") is appended later by `atbswp export`.
"""
import struct
import sys

MAGIC = b"ATBSWPH1"


def strip_existing(data: bytes) -> bytes:
    if len(data) >= 16 and data[-8:] == MAGIC:
        (n,) = struct.unpack("<Q", data[-16:-8])
        return data[: len(data) - 16 - n]
    return data


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: bundle.py PLAYER.COM HELPER", file=sys.stderr)
        return 64
    player_path, helper_path = sys.argv[1:]
    with open(player_path, "rb") as f:
        player = strip_existing(f.read())
    with open(helper_path, "rb") as f:
        helper = f.read()
    if not helper:
        print("helper is empty", file=sys.stderr)
        return 1
    with open(player_path, "wb") as f:
        f.write(player + helper + struct.pack("<Q", len(helper)) + MAGIC)
    print(f"bundled {len(helper)} byte helper into {player_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
