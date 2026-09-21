#!/usr/bin/env python3
"""Deterministic CG9 geometry regeneration pipeline.

Order:
1. Human body morph targets (from current body GLB bind pose)
2. Equipment morph transfer (fit_scale already baked offline)
3. Local clearance correction
4. Validation report
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
SCRIPTS = REPO / "scripts"


def run(script: str, *args: str) -> None:
    cmd = [sys.executable, str(SCRIPTS / script), *args]
    print("+", " ".join(cmd))
    subprocess.run(cmd, check=True, cwd=REPO)


def main() -> None:
    run("author_human_morph_targets.py")
    run("author_equipment_morph_targets.py")
    run("author_equipment_local_fit.py")
    result = subprocess.run(
        [sys.executable, str(SCRIPTS / "validate_equipment_clearance.py")],
        cwd=REPO,
    )
    if result.returncode != 0:
        raise SystemExit(result.returncode)


if __name__ == "__main__":
    main()
