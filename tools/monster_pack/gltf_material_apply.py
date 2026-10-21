"""Bind Unity-resolved textures onto a merged GLB (glTF 2.0 PBR)."""

from __future__ import annotations

import io
from pathlib import Path
from typing import Any

from glb_sanitize_animations import load_glb, write_glb
from unity_material_resolve import ResolvedMaterial

try:
    from PIL import Image
except ImportError:
    Image = None  # type: ignore


def _append_image_bytes(gltf: dict, blob: bytearray, data: bytes, mime: str) -> int:
    while len(blob) % 4:
        blob.append(0)
    off = len(blob)
    blob.extend(data)
    gltf["bufferViews"].append({"buffer": 0, "byteOffset": off, "byteLength": len(data)})
    image_index = len(gltf.get("images", []))
    gltf.setdefault("images", []).append({"bufferView": len(gltf["bufferViews"]) - 1, "mimeType": mime})
    return image_index


def _add_texture(gltf: dict, image_index: int) -> int:
    gltf.setdefault("textures", []).append({"source": image_index})
    return len(gltf["textures"]) - 1


def _metallic_roughness_from_unity(ms_path: Path) -> tuple[bytes, str]:
    if Image is None:
        raise RuntimeError("Pillow required for MetallicSmoothness repack (pip install Pillow)")
    im = Image.open(ms_path).convert("RGBA")
    out = Image.new("RGBA", im.size)
    px = im.load()
    opx = out.load()
    for y in range(im.height):
        for x in range(im.width):
            r, g, b, a = px[x, y]
            metallic = r / 255.0
            roughness = 1.0 - (a / 255.0)
            opx[x, y] = (0, int(roughness * 255), int(metallic * 255), 255)
    buf = io.BytesIO()
    out.save(buf, format="PNG")
    return buf.getvalue(), "image/png"


def _apply_one_material_slot(gltf: dict, blob: bytearray, mat_index: int, material: ResolvedMaterial) -> dict[str, Any]:
    if material.base_color is None:
        raise ValueError(f"material `{material.material_name}` has no BaseColor texture")
    report: dict[str, Any] = {"material": material.material_name, "textures": {}}
    base_data = material.base_color.read_bytes()
    mime = "image/png" if material.base_color.suffix.lower() == ".png" else "image/jpeg"
    base_img = _append_image_bytes(gltf, blob, base_data, mime)
    base_tex = _add_texture(gltf, base_img)
    report["textures"]["baseColor"] = str(material.base_color)

    normal_tex = None
    if material.normal and material.normal.is_file():
        nd = material.normal.read_bytes()
        nmime = "image/png" if material.normal.suffix.lower() == ".png" else "image/jpeg"
        ni = _append_image_bytes(gltf, blob, nd, nmime)
        normal_tex = _add_texture(gltf, ni)
        report["textures"]["normal"] = str(material.normal)

    mr_tex = None
    if material.metallic_smoothness and material.metallic_smoothness.is_file():
        mr_bytes, mr_mime = _metallic_roughness_from_unity(material.metallic_smoothness)
        mi = _append_image_bytes(gltf, blob, mr_bytes, mr_mime)
        mr_tex = _add_texture(gltf, mi)
        report["textures"]["metallicRoughness"] = str(material.metallic_smoothness)

    pbr: dict[str, Any] = {"baseColorTexture": {"index": base_tex}}
    if mr_tex is not None:
        pbr["metallicRoughnessTexture"] = {"index": mr_tex}
    else:
        pbr.setdefault("metallicFactor", 0.0)
        pbr.setdefault("roughnessFactor", 0.45)

    if mat_index >= len(gltf.setdefault("materials", [])):
        gltf["materials"].append({"name": material.material_name, "pbrMetallicRoughness": pbr, "doubleSided": True})
    else:
        mat = gltf["materials"][mat_index]
        mat["name"] = material.material_name
        mat["pbrMetallicRoughness"] = {**mat.get("pbrMetallicRoughness", {}), **pbr}
        mat["doubleSided"] = True
        if normal_tex is not None:
            mat["normalTexture"] = {"index": normal_tex}
    report["warnings"] = material.warnings
    report["unsupported_shader_guid"] = material.unsupported_shader
    return report


def apply_materials(glb_path: Path, materials: list[ResolvedMaterial], out_path: Path | None = None) -> dict[str, Any]:
    gltf, blob = load_glb(glb_path)
    blob = bytearray(blob)
    by_name = {m.material_name: m for m in materials}
    reports = []
    existing = gltf.get("materials", [])
    if existing:
        for i, slot in enumerate(existing):
            name = slot.get("name")
            if name in by_name:
                reports.append(_apply_one_material_slot(gltf, blob, i, by_name[name]))
        missing_names = [m.material_name for m in materials if m.material_name not in {s.get("name") for s in existing}]
        if missing_names and len(existing) == 1 and len(materials) == 1 and not reports:
            # Unity variant materials (e.g. M_Buformorph_Red) often differ from the FBX slot name (M_Buformorph).
            reports.append(_apply_one_material_slot(gltf, blob, 0, materials[0]))
            missing_names = []
        if missing_names:
            raise ValueError(f"glTF has no material slots for: {missing_names}")
    else:
        if len(materials) != 1:
            raise ValueError("multi-material bind requires existing material slots from FBX2glTF SK")
        reports.append(_apply_one_material_slot(gltf, blob, 0, materials[0]))
        for mesh in gltf.get("meshes", []):
            for prim in mesh.get("primitives", []):
                prim["material"] = 0

    gltf["buffers"][0]["byteLength"] = len(blob)
    target = out_path or glb_path
    write_glb(target, gltf, bytes(blob))
    return {"slots": reports}


def apply_material(glb_path: Path, material: ResolvedMaterial, out_path: Path | None = None) -> dict[str, Any]:
    return apply_materials(glb_path, [material], out_path)
