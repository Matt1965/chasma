#!/usr/bin/env python3
"""Author Slice 7.2 equipment presentation tuning into Chasma Design.xlsx."""

from __future__ import annotations

import math
import openpyxl
from pathlib import Path

WORKBOOK = Path(__file__).resolve().parents[1] / "Chasma Design.xlsx"

# Bevy XYZW quaternions derived from asset pivot inspection (grip at mesh ymin, blade along +Y).
SWORD_GRIP_QUAT = "0,-0.5,-0.5,0.5"  # X-90 then Z+90
BOW_GRIP_QUAT = "0.5,0.5,0.5,0.5"  # vertical bow held in right hand

# Backpack: asset centered at origin with front facing +Z; flip to back and offset from spine_02.
BACKPACK_QUAT = "0,1,0,0"
BACKPACK_TRANS = "0,0.10,-0.16"

RIGID_WEAPON_TUNING: dict[str, dict[str, str]] = {
    "iron_sword": {
        "Local Translation": "0,0.183,0.02",
        "Local Rotation": SWORD_GRIP_QUAT,
    },
    "iron_dagger": {
        "Local Translation": "0,0.100,0.01",
        "Local Rotation": SWORD_GRIP_QUAT,
    },
    "iron_hand_axe": {
        "Local Translation": "0,0.271,0.02",
        "Local Rotation": SWORD_GRIP_QUAT,
    },
    "iron_greatsword": {
        "Local Translation": "0,0.083,0.03",
        "Local Rotation": SWORD_GRIP_QUAT,
    },
    "iron_greataxe": {
        "Local Translation": "0,0.370,0.03",
        "Local Rotation": SWORD_GRIP_QUAT,
    },
    "iron_warhammer": {
        "Local Translation": "0,0.409,0.02",
        "Local Rotation": SWORD_GRIP_QUAT,
    },
    "wooden_bow": {
        "Local Translation": "0,0.675,0.02",
        "Local Rotation": BOW_GRIP_QUAT,
    },
    "leather_backpack": {
        "Local Translation": BACKPACK_TRANS,
        "Local Rotation": BACKPACK_QUAT,
    },
}


def header_map(ws) -> dict[str, int]:
    headers = [cell.value for cell in next(ws.iter_rows(max_row=1))]
    return {str(h).strip(): idx for idx, h in enumerate(headers) if h}


def main() -> None:
    wb = openpyxl.load_workbook(WORKBOOK)
    ws = wb["Equipment Visuals"]
    mapping = header_map(ws)
    item_col = mapping["Item ID"]
    trans_col = mapping["Local Translation"]
    rot_col = mapping["Local Rotation"]

    updated = 0
    for row in range(2, ws.max_row + 1):
        item_id = ws.cell(row=row, column=item_col + 1).value
        if item_id not in RIGID_WEAPON_TUNING:
            continue
        tuning = RIGID_WEAPON_TUNING[item_id]
        ws.cell(row=row, column=trans_col + 1, value=tuning["Local Translation"])
        ws.cell(row=row, column=rot_col + 1, value=tuning["Local Rotation"])
        updated += 1

    wb.save(WORKBOOK)
    print(f"Updated {updated} Equipment Visual rows in {WORKBOOK}")


if __name__ == "__main__":
    main()
