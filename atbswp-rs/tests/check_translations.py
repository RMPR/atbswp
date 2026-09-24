#!/usr/bin/env python3
"""Every @tr("...") literal in the GUI must have a translation in every
bundled .po file (English may leave msgstr empty).  Run from anywhere."""
import re
import sys
from pathlib import Path

root = Path(__file__).resolve().parents[1] / "crates/atbswp-gui"
slint = (root / "ui/app.slint").read_text()
ids = set(re.findall(r'@tr\("((?:[^"\\]|\\.)*)"', slint))
bad = 0
for po in sorted(root.glob("lang/*/LC_MESSAGES/atbswp-gui.po")):
    lang = po.parts[-3]
    entries = dict(re.findall(r'^msgid "((?:[^"\\]|\\.)*)"\nmsgstr "((?:[^"\\]|\\.)*)"', po.read_text(), re.M))
    entries.pop("", None)
    missing = ids - set(entries)
    untranslated = {k for k, v in entries.items() if not v} if lang != "en" else set()
    stale = set(entries) - ids
    for what, items in (("missing", missing), ("untranslated", untranslated), ("stale", stale)):
        if items:
            bad += len(items)
            print(f"{lang}: {what}: {sorted(items)}")
print(f"translations: {len(ids)} strings x {len(list(root.glob('lang/*')))} languages, {'OK' if not bad else f'{bad} problems'}")
sys.exit(1 if bad else 0)
