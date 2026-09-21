#!/usr/bin/env python3
"""Remove erroneous appearance: None insertions from struct-literal script."""

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "src"

TRAILING_BLOCK = re.compile(
    r"\n\s*appearance: None,\s*\n\s*\}(\s*\n)",
    re.MULTILINE,
)

for path in ROOT.rglob("*.rs"):
    text = path.read_text(encoding="utf-8")
    orig = text

    # Move appearance into UnitRecord struct literals when it was appended outside.
    text = re.sub(
        r"(work_skills: Default::default\(\),)\s*\n\s*\}\s*\n\s*appearance: None,\s*\n\s*\}",
        r"\1\n            appearance: None,\n        }",
        text,
    )

    # Function returning new_test(...) accidentally got an extra block.
    text = re.sub(
        r"(UnitRecord::new_test\([\s\S]*?\))\s*\n\s*appearance: None,\s*\n\s*\}",
        r"\1\n    }",
        text,
    )

    # create_unit_with_inventory(...).unwrap() accidentally got an extra block.
    text = re.sub(
        r"(create_unit_with_inventory\([\s\S]*?\)\s*\n\s*\.unwrap\(\))\s*\n\s*appearance: None,\s*\n\s*\}",
        r"\1\n}",
        text,
    )

    # Generic trailing garbage block after closing paren of a call.
    text = re.sub(
        r"(\))\s*\n\s*appearance: None,\s*\n\s*\}(\s*\n\s*(?:fn |#\[test\]|pub fn ))",
        r")\1\2",
        text,
    )

    if text != orig:
        path.write_text(text, encoding="utf-8")
        print(f"fixed {path.relative_to(ROOT.parent)}")
