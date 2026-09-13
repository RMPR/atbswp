#!/usr/bin/env python3
"""Store (or replace) the native Intel-macOS helper in an APE's zip section.

An APE is a valid zip archive; the player reads /zip/player-macos-x86_64.
`atbswp export` later adds /zip/macro.bin the same way.
"""
import sys
import zipfile

ENTRY = "player-macos-x86_64"


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: bundle.py PLAYER.COM HELPER", file=sys.stderr)
        return 64
    player_path, helper_path = sys.argv[1:]
    with open(helper_path, "rb") as f:
        helper = f.read()
    if not helper:
        print("helper is empty", file=sys.stderr)
        return 1
    # Rebuild the archive without a previous helper entry, then append.
    try:
        with zipfile.ZipFile(player_path) as z:
            names = z.namelist()
    except zipfile.BadZipFile:
        names = []
    if ENTRY in names:
        print(f"{player_path} already carries {ENTRY}; replacing is not supported, "
              "rebuild the player first", file=sys.stderr)
        return 1
    with zipfile.ZipFile(player_path, "a", zipfile.ZIP_STORED) as z:
        z.writestr(ENTRY, helper)
    print(f"stored {len(helper)} byte helper as /zip/{ENTRY} in {player_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
