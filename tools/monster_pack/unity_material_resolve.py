"""Resolve Unity prefab renderer material -> .mat -> texture paths."""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from pathlib import Path


@dataclass
class ResolvedMaterial:
    material_name: str
    material_path: Path
    base_color: Path | None = None
    normal: Path | None = None
    metallic_smoothness: Path | None = None
    emissive: Path | None = None
    unsupported_shader: str | None = None
    warnings: list[str] = field(default_factory=list)


@dataclass
class MaterialResolution:
    prefab_path: Path
    renderer_material_guids: list[str]
    resolved: ResolvedMaterial | None = None
    resolved_materials: list[ResolvedMaterial] = field(default_factory=list)
    ambiguous: bool = False
    ambiguity_detail: str = ""


def _guid_index(source_dir: Path) -> dict[str, Path]:
    index: dict[str, Path] = {}
    for meta in source_dir.rglob("*.meta"):
        try:
            text = meta.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        m = re.search(r"^guid:\s*([0-9a-f]+)", text, re.M)
        if m:
            asset = meta.with_suffix("")
            if asset.suffix.lower() in {".mat", ".png", ".tga", ".jpg"}:
                index[m.group(1)] = asset
    return index


def _prefab_material_guids(prefab_path: Path) -> list[str]:
    text = prefab_path.read_text(encoding="utf-8", errors="replace")
    guids = re.findall(r"m_Materials:\s*\n\s*-\s*\{fileID: \d+, guid: ([0-9a-f]+), type: 2\}", text)
    if not guids:
        guids = re.findall(r"guid: ([0-9a-f]+), type: 2\}", text)
    return guids


def _mat_texture_guids(mat_path: Path) -> dict[str, str]:
    text = mat_path.read_text(encoding="utf-8", errors="replace")
    shader = ""
    sm = re.search(r"m_Shader:.*guid: ([0-9a-f]+)", text)
    if sm:
        shader = sm.group(1)
    out: dict[str, str] = {}
    for prop in ("_MainTex", "_BaseMap", "_BumpMap", "_MetallicGlossMap", "_MetallicSmoothness", "_EmissionMap"):
        m = re.search(
            rf"- {prop}:\s*\n\s*m_Texture: \{{fileID: \d+, guid: ([0-9a-f]+), type: 3\}}",
            text,
        )
        if m:
            out[prop] = m.group(1)
    return {"shader_guid": shader, "textures": out}


def _texture_by_suffix(textures_dir: Path, suffix: str) -> Path | None:
    if not textures_dir.is_dir():
        return None
    for pattern in (f"*{suffix}.png", f"*{suffix}.tga", f"*{suffix}.PNG"):
        matches = sorted(textures_dir.glob(pattern))
        if matches:
            return matches[0]
    return None


def resolve_prefab_material(source_dir: Path, prefab_path: Path | None = None) -> MaterialResolution:
    prefab_dir = source_dir / "Prefabs"
    if prefab_path is None:
        prefabs = sorted(prefab_dir.glob("*.prefab"))
        if len(prefabs) == 0:
            raise FileNotFoundError(f"No prefabs under {prefab_dir}")
        if len(prefabs) > 1:
            return MaterialResolution(
                prefab_path=prefab_dir,
                renderer_material_guids=[],
                ambiguous=True,
                ambiguity_detail=f"multiple prefabs ({len(prefabs)}): {[p.name for p in prefabs]} — pass --prefab",
            )
        prefab_path = prefabs[0]
    guid_map = _guid_index(source_dir)
    guids = _prefab_material_guids(prefab_path)
    unique_guids = []
    for g in guids:
        if g not in unique_guids:
            unique_guids.append(g)
    if not unique_guids:
        return MaterialResolution(
            prefab_path=prefab_path,
            renderer_material_guids=[],
            ambiguous=True,
            ambiguity_detail="no SkinnedMeshRenderer materials found in prefab",
        )
    resolved_list: list[ResolvedMaterial] = []
    textures_dir = source_dir / "Textures"
    for guid in unique_guids:
        mat_path = guid_map.get(guid)
        if mat_path is None or not mat_path.is_file():
            return MaterialResolution(
                prefab_path=prefab_path,
                renderer_material_guids=unique_guids,
                ambiguous=True,
                ambiguity_detail=f"guid {guid} not resolved to .mat under source_dir",
            )
        mat_info = _mat_texture_guids(mat_path)
        tex = mat_info["textures"]
        base = guid_map.get(tex.get("_MainTex", tex.get("_BaseMap", "")))
        normal = guid_map.get(tex.get("_BumpMap", ""))
        ms = guid_map.get(tex.get("_MetallicGlossMap", tex.get("_MetallicSmoothness", "")))
        emissive = guid_map.get(tex.get("_EmissionMap", ""))
        resolved = ResolvedMaterial(
            material_name=mat_path.stem,
            material_path=mat_path,
            base_color=base if base and base.is_file() else _texture_by_suffix(textures_dir, "_BaseColor"),
            normal=normal if normal and normal.is_file() else _texture_by_suffix(textures_dir, "_Normal"),
            metallic_smoothness=ms if ms and ms.is_file() else _texture_by_suffix(textures_dir, "_MetallicSmoothness"),
            emissive=emissive if emissive and emissive.is_file() else _texture_by_suffix(textures_dir, "_Emissive"),
            unsupported_shader=mat_info["shader_guid"] or None,
        )
        if resolved.base_color is None:
            resolved.warnings.append("no BaseColor texture resolved from .mat or Textures/*_BaseColor")
        if resolved.metallic_smoothness is None:
            resolved.warnings.append("no MetallicSmoothness texture — metallic/roughness will use defaults")
        resolved_list.append(resolved)
    primary = resolved_list[0]
    return MaterialResolution(
        prefab_path=prefab_path,
        renderer_material_guids=unique_guids,
        resolved=primary,
        resolved_materials=resolved_list,
    )
