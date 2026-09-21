#!/usr/bin/env python3
"""Add appearance: None to UnitRecord / SceneUnitRecord struct literals missing it."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "src"

MARKERS = [
    ("UnitRecord {", "appearance: None,"),
    ("SceneUnitRecord {", "appearance: None,"),
]


def process_file(path: Path) -> bool:
    text = path.read_text(encoding="utf-8")
    if "appearance:" in text:
        # still may miss some literals; continue per marker
        pass
    changed = False
    for struct_name, field_line in MARKERS:
        idx = 0
        while True:
            start = text.find(struct_name, idx)
            if start == -1:
                break
            brace = text.find("{", start)
            depth = 0
            end = None
            for pos in range(brace, len(text)):
                ch = text[pos]
                if ch == "{":
                    depth += 1
                elif ch == "}":
                    depth -= 1
                    if depth == 0:
                        end = pos
                        break
            if end is None:
                break
            block = text[brace : end + 1]
            if "appearance:" in block:
                idx = end + 1
                continue
            insertion = f"\n            {field_line}\n        "
            text = text[:end] + insertion + text[end:]
            changed = True
            idx = end + len(insertion) + 1
    if changed:
        path.write_text(text, encoding="utf-8")
    return changed


for path in ROOT.rglob("*.rs"):
    if process_file(path):
        print(f"updated {path.relative_to(ROOT.parent)}")
