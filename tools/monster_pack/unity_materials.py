"""Assign Unity pack textures to imported FBX meshes before glTF export."""

from __future__ import annotations

from pathlib import Path

import bpy


def _find_texture(textures_dir: Path, suffix: str) -> Path | None:
    if not textures_dir.is_dir():
        return None
    for pattern in (f"*{suffix}.png", f"*{suffix}.tga", f"*{suffix}.PNG", f"*{suffix}.TGA"):
        matches = sorted(textures_dir.glob(pattern))
        if matches:
            return matches[0]
    return None


def discover_texture_set(source_dir: Path) -> dict[str, Path]:
    textures_dir = source_dir / "Textures"
    keys = {
        "base": "_BaseColor",
        "normal": "_Normal",
        "metallic_smoothness": "_MetallicSmoothness",
        "emissive": "_Emissive",
    }
    out: dict[str, Path] = {}
    for name, suffix in keys.items():
        path = _find_texture(textures_dir, suffix)
        if path:
            out[name] = path
    return out


def _load_image(path: Path) -> bpy.types.Image:
    image = bpy.data.images.load(str(path), check_existing=True)
    image.colorspace_settings.name = (
        "Non-Color" if "Normal" in path.name or "Metallic" in path.name else "sRGB"
    )
    return image


def _principled_input(bsdf: bpy.types.Node, *names: str) -> bpy.types.NodeSocket:
    for name in names:
        if name in bsdf.inputs:
            return bsdf.inputs[name]
    raise KeyError(names[0])


def build_unity_material(name: str, textures: dict[str, Path]) -> bpy.types.Material:
    mat = bpy.data.materials.new(name=name)
    mat.use_nodes = True
    tree = mat.node_tree
    nodes = tree.nodes
    links = tree.links
    nodes.clear()

    output = nodes.new("ShaderNodeOutputMaterial")
    bsdf = nodes.new("ShaderNodeBsdfPrincipled")
    links.new(bsdf.outputs["BSDF"], output.inputs["Surface"])

    if "base" in textures:
        tex = nodes.new("ShaderNodeTexImage")
        tex.image = _load_image(textures["base"])
        links.new(tex.outputs["Color"], _principled_input(bsdf, "Base Color"))

    if "normal" in textures:
        tex = nodes.new("ShaderNodeTexImage")
        tex.image = _load_image(textures["normal"])
        tex.image.colorspace_settings.name = "Non-Color"
        normal = nodes.new("ShaderNodeNormalMap")
        links.new(tex.outputs["Color"], normal.inputs["Color"])
        links.new(normal.outputs["Normal"], _principled_input(bsdf, "Normal"))

    if "metallic_smoothness" in textures:
        tex = nodes.new("ShaderNodeTexImage")
        tex.image = _load_image(textures["metallic_smoothness"])
        tex.image.colorspace_settings.name = "Non-Color"
        sep = nodes.new("ShaderNodeSeparateColor")
        sep.mode = "RGB"
        links.new(tex.outputs["Color"], sep.inputs["Color"])
        links.new(sep.outputs["Red"], _principled_input(bsdf, "Metallic"))
        _principled_input(bsdf, "Roughness").default_value = 0.45

    if "emissive" in textures:
        tex = nodes.new("ShaderNodeTexImage")
        tex.image = _load_image(textures["emissive"])
        links.new(tex.outputs["Color"], _principled_input(bsdf, "Emission Color"))

    return mat


def apply_textures_to_meshes(source_dir: Path, material_name: str) -> None:
    textures = discover_texture_set(source_dir)
    if "base" not in textures:
        print(f"WARN: no BaseColor texture under {source_dir / 'Textures'}")
        return
    mat = build_unity_material(material_name, textures)
    for obj in bpy.data.objects:
        if obj.type != "MESH":
            continue
        if obj.data.materials:
            obj.data.materials[0] = mat
        else:
            obj.data.materials.append(mat)
