#!/usr/bin/env python3
"""
Blender batch export: Unity monster FBX pack -> single skinned GLB.

Run headless:
  blender --background --python blender_export_monster.py -- \\
    --source-dir "<monster folder>" \\
    --monster-name Gorosaurus \\
    --output "C:/path/gorosaurus.glb"
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

_SCRIPT_DIR = Path(__file__).resolve().parent
if str(_SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(_SCRIPT_DIR))

from monster_export_core import export_monster_glb  # noqa: E402


def parse_args() -> argparse.Namespace:
    argv = sys.argv
    if "--" in argv:
        argv = argv[argv.index("--") + 1 :]
    else:
        argv = []
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-dir", type=Path, required=True)
    parser.add_argument("--monster-name", required=True)
    parser.add_argument("--anim-prefix", default="")
    parser.add_argument("--sk-fbx", type=Path, default=None)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--include-rm-clips", action="store_true", default=True)
    parser.add_argument("--no-rm-clips", action="store_true")
    parser.add_argument(
        "--checkpoint-dir",
        type=Path,
        default=None,
        help="Write export checkpoints (see run_bufomorph_diagnostics.ps1).",
    )
    return parser.parse_args(argv)


def main() -> None:
    args = parse_args()
    fbx_dir = args.source_dir / "FBX Files"
    if not fbx_dir.is_dir():
        raise SystemExit(f"Missing FBX Files: {fbx_dir}")

    if args.sk_fbx:
        sk_path = args.sk_fbx
    else:
        sk_candidates = sorted(fbx_dir.glob(f"SK_{args.monster_name}.FBX"))
        sk_candidates += sorted(fbx_dir.glob(f"SK_{args.monster_name}.fbx"))
        if not sk_candidates:
            sk_candidates = sorted(fbx_dir.glob("SK_*.FBX"))
        if not sk_candidates:
            raise SystemExit(f"No SK_ mesh for {args.monster_name}")
        sk_path = sk_candidates[0]

    anim_prefix = args.anim_prefix or args.monster_name
    include_rm = args.include_rm_clips and not args.no_rm_clips

    merged, removed, kept, rebased = export_monster_glb(
        source_dir=args.source_dir,
        monster_name=args.monster_name,
        output=args.output,
        anim_prefix=anim_prefix,
        sk_fbx=sk_path,
        include_rm_clips=include_rm,
        checkpoint_dir=args.checkpoint_dir,
    )
    print(
        f"Exported {args.output}: {merged} clips "
        f"(stripped {removed} anim channels, kept {kept}, rebased {rebased} clips)"
    )


if __name__ == "__main__":
    main()
