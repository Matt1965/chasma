"""DIAGNOSTIC: cross-path comparison of one pack creature.

Compares, in equivalent spaces, the rest skeleton and sampled clip values across:
  * Unity prefab (independent reference, parsed from the .prefab YAML; LH Y-up -> RH via X mirror,
    compared through mirror-invariant quantities: joint distances, rotation-difference angles)
  * FBX2glTF direct outputs for SK_*.fbx and *@Clip.fbx (same converter, same space)
  * the shipped Chasma GLB (Blender merge path)

Writes a JSON report. Bufomorph bone names are the default; override with --bone.

Usage:
  py -3 compare_paths.py --pack "<pack monster folder>" --fbx2gltf-dir <dir with SK_X.glb and X@Clip.glb>
      --shipped ../../../assets/units/bufomorph.glb [--prefab Buformorph_Red.prefab]
      [--sk SK_Buformorph.glb] [--anim-prefix Buformorph] [--out compare_paths.json]
"""
from __future__ import annotations
import argparse, json, math, re, struct, sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from glb_sanitize_animations import load_glb

PACK = DIAG = SHIPPED = OUT = None
PREFAB = "Buformorph_Red.prefab"
SK_NAME = "SK_Buformorph.glb"
ANIM_PREFIX = "Buformorph"
BONES = ["root", "Bufomorph_", "Bufomorph_ Pelvis", "Bufomorph_ Spine", "Bufomorph_ Neck",
         "Bufomorph_ Head", "Bufomorph_JawUpper", "Bufomorph_JawLeft", "Bufomorph_JawRight", "Bufomorph_Tongue1", "Tongue1", "Tongue2"]

# ---------- quaternion helpers (x,y,z,w) ----------
def qmul(a, b):
    ax, ay, az, aw = a; bx, by, bz, bw = b
    return (aw*bx + ax*bw + ay*bz - az*by,
            aw*by - ax*bz + ay*bw + az*bx,
            aw*bz + ax*by - ay*bx + az*bw,
            aw*bw - ax*bx - ay*by - az*bz)
def qconj(q): return (-q[0], -q[1], -q[2], q[3])
def qangle_deg(a, b):
    d = qmul(qconj(a), b)
    w = max(-1.0, min(1.0, abs(d[3])))
    return math.degrees(2*math.acos(w))
def qrot(q, v):
    x, y, z, w = q; vx, vy, vz = v
    cx = y*vz - z*vy; cy = z*vx - x*vz; cz = x*vy - y*vx
    return (vx + 2*(w*cx + y*cz - z*cy), vy + 2*(w*cy + z*cx - x*cz), vz + 2*(w*cz + x*cy - y*cx))
def vlen(v): return math.sqrt(sum(c*c for c in v))

# ---------- glTF ----------
def acc(gltf, blob, i, fmt, n):
    a = gltf["accessors"][i]; bv = gltf["bufferViews"][a["bufferView"]]
    off = bv.get("byteOffset", 0) + a.get("byteOffset", 0)
    size = struct.calcsize(fmt); stride = bv.get("byteStride", size)
    return [struct.unpack_from(fmt, blob, off + k*stride) for k in range(a["count"])]

def nodes_by_name(gltf):
    return {n.get("name", f"#{i}"): i for i, n in enumerate(gltf["nodes"])}

def parent_map(gltf):
    pm = {}
    for i, n in enumerate(gltf["nodes"]):
        for c in n.get("children", []): pm[c] = i
    return pm

def local_trs(gltf, i):
    n = gltf["nodes"][i]
    return (tuple(n.get("translation", [0,0,0])), tuple(n.get("rotation", [0,0,0,1])), tuple(n.get("scale", [1,1,1])))

def sample(gltf, blob, anim, node, path, t):
    for ch in anim["channels"]:
        if ch["target"].get("node") == node and ch["target"]["path"] == path:
            s = anim["samplers"][ch["sampler"]]
            times = [v[0] for v in acc(gltf, blob, s["input"], "<f", 1)]
            fmt = "<fff" if path in ("translation", "scale") else "<ffff"
            vals = acc(gltf, blob, s["output"], fmt, 1)
            k = min(range(len(times)), key=lambda j: abs(times[j]-t))
            return vals[k], times[k]
    return None, None

def world_pos_rest(gltf, name_to_idx, pm):
    """World position of each node in rest TRS (full chain)."""
    cache = {}
    def wm(i):
        if i in cache: return cache[i]
        t, r, s = local_trs(gltf, i)
        if i in pm:
            pt, pr, ps = wm(pm[i])
            # compose: world = parent * local (scale assumed uniform-ish)
            wt = tuple(pt[k] + c for k, c in enumerate(qrot(pr, tuple(t[k]*ps[k] for k in range(3)))))
            wr = qmul(pr, r); ws = tuple(ps[k]*s[k] for k in range(3))
        else:
            wt, wr, ws = t, r, s
        cache[i] = (wt, wr, ws); return cache[i]
    return {n: wm(i) for n, i in name_to_idx.items()}

# ---------- Unity prefab ----------
def parse_prefab(path):
    text = path.read_text(encoding="utf-8", errors="replace")
    docs = re.split(r"^--- !u!(\d+) &(\d+)\s*$", text, flags=re.M)
    go_name = {}; transforms = {}
    for k in range(1, len(docs), 3):
        cls, fid, body = docs[k], docs[k+1], docs[k+2]
        if cls == "1":
            m = re.search(r"m_Name: (.*)", body); go_name[fid] = m.group(1).strip() if m else fid
        elif cls == "4":
            gm = re.search(r"m_GameObject: \{fileID: (\d+)\}", body)
            rot = re.search(r"m_LocalRotation: \{x: ([^,]+), y: ([^,]+), z: ([^,]+), w: ([^}]+)\}", body)
            pos = re.search(r"m_LocalPosition: \{x: ([^,]+), y: ([^,]+), z: ([^}]+)\}", body)
            fam = re.search(r"m_Father: \{fileID: (\d+)\}", body)
            if not (gm and rot and pos and fam):
                continue
            g = gm.group(1); fa = fam.group(1)
            transforms[fid] = dict(go=g, rot=tuple(float(v) for v in rot.groups()), pos=tuple(float(v) for v in pos.groups()), father=fa)
    out = {}
    for fid, tr in transforms.items():
        name = go_name.get(tr["go"], tr["go"])
        parent = go_name.get(transforms[tr["father"]]["go"]) if tr["father"] in transforms else None
        out[name] = dict(parent=parent, rot=tr["rot"], pos=tr["pos"])
    return out

def unity_world_rest(prefab):
    cache = {}
    def wm(name):
        if name in cache: return cache[name]
        n = prefab[name]
        if n["parent"] and n["parent"] in prefab:
            pt, pr = wm(n["parent"])
            wt = tuple(pt[k] + c for k, c in enumerate(qrot(pr, n["pos"])))
            wr = qmul(pr, n["rot"])
        else:
            wt, wr = n["pos"], n["rot"]
        cache[name] = (wt, wr); return cache[name]
    return {n: wm(n) for n in prefab}

def main():
    report = {}
    prefab = parse_prefab(PACK / "Prefabs" / PREFAB)
    uw = unity_world_rest(prefab)
    report["unity_prefab"] = {b: dict(parent=prefab[b]["parent"], local_pos=prefab[b]["pos"], local_rot=prefab[b]["rot"],
                                      world_pos=uw[b][0]) for b in BONES if b in prefab}
    report["unity_children_of_head"] = [n for n, v in prefab.items() if v["parent"] == "Bufomorph_ Head"]
    report["unity_children_of_jaw"] = [n for n, v in prefab.items() if v["parent"] == "Bufomorph_ JawUpper"]

    sk_g, sk_b = load_glb(DIAG / SK_NAME)
    sk_n = nodes_by_name(sk_g); sk_pm = parent_map(sk_g); sk_w = world_pos_rest(sk_g, sk_n, sk_pm)
    report["fbx2gltf_sk_rest"] = {b: dict(local=local_trs(sk_g, sk_n[b]), world_pos=sk_w[b][0],
                                          parent=sk_g["nodes"][sk_pm[sk_n[b]]].get("name") if sk_n[b] in sk_pm else None)
                                  for b in BONES if b in sk_n}
    report["fbx2gltf_sk_children_of_head"] = [sk_g["nodes"][c].get("name") for c in sk_g["nodes"][sk_n["Bufomorph_ Head"]].get("children", [])]

    sh_g, sh_b = load_glb(SHIPPED)
    sh_n = nodes_by_name(sh_g); sh_pm = parent_map(sh_g); sh_w = world_pos_rest(sh_g, sh_n, sh_pm)
    report["shipped_rest"] = {b: dict(local=local_trs(sh_g, sh_n[b]), world_pos=sh_w[b][0],
                                      parent=sh_g["nodes"][sh_pm[sh_n[b]]].get("name") if sh_n[b] in sh_pm else None)
                              for b in BONES if b in sh_n}
    report["shipped_children_of_head"] = [sh_g["nodes"][c].get("name") for c in sh_g["nodes"][sh_n["Bufomorph_ Head"]].get("children", [])]

    # joint-distance invariants (rest): Head->JawUpper, JawUpper->Tongue1, Pelvis->Head
    def dist(wp, a, b):
        return vlen(tuple(wp[a][0][k]-wp[b][0][k] for k in range(3))) if a in wp and b in wp else None
    pairs = [("Bufomorph_ Head","Bufomorph_JawUpper"), ("Bufomorph_JawUpper","Bufomorph_JawLeft"), ("Bufomorph_JawLeft","Bufomorph_JawRight"), ("Bufomorph_ Pelvis","Bufomorph_ Head"), ("Bufomorph_","Bufomorph_ Pelvis"), ("Bufomorph_","Bufomorph_ Head"), ("root","Bufomorph_")]
    # rest-pose reference mismatch: anim-FBX node defaults (FBX2glTF clip GLB) vs SK bind (FBX2glTF SK GLB)
    ref = {}
    for clip, fn in [("IdleLookAround",f"{ANIM_PREFIX}@IdleLookAround.glb"), ("BiteAttack",f"{ANIM_PREFIX}@BiteAttack.glb")]:
        g, b = load_glb(DIAG / fn); n = nodes_by_name(g)
        rows = {}
        for bone in BONES:
            if bone in n and bone in sk_n:
                ct, cr, _ = local_trs(g, n[bone]); st, sr, _ = local_trs(sk_g, sk_n[bone])
                rows[bone] = dict(anim_fbx_rest_rot=cr, sk_bind_rot=sr, angle_deg=qangle_deg(cr, sr),
                                  anim_fbx_rest_t=ct, sk_bind_t=st, t_delta=vlen(tuple(ct[k]-st[k] for k in range(3))))
        ref[clip] = rows
    report["anim_fbx_rest_vs_sk_bind"] = ref
    report["rest_joint_distances"] = {f"{a} <-> {b}": dict(unity=dist(uw,a,b), fbx2gltf_sk=dist(sk_w,a,b), shipped=dist(sh_w,a,b)) for a,b in pairs}

    # animated comparisons
    anims = {}
    for clip, fn in [("IdleLookAround",f"{ANIM_PREFIX}@IdleLookAround.glb"), ("BiteAttack",f"{ANIM_PREFIX}@BiteAttack.glb"), ("Run",f"{ANIM_PREFIX}@Run.glb")]:
        g, b = load_glb(DIAG / fn); n = nodes_by_name(g); a = g["animations"][0]
        sh_a = next(x for x in sh_g["animations"] if x["name"] == clip)
        rows = {}
        for bone in ["root","Bufomorph_","Bufomorph_ Pelvis","Bufomorph_ Head","Bufomorph_JawUpper","Bufomorph_JawLeft","Bufomorph_JawRight","Bufomorph_Tongue1","Tongue1"]:
            if bone not in n or bone not in sh_n: continue
            r = {}
            for t in (0.0, 0.35, 0.5):
                f_t, _ = sample(g, b, a, n[bone], "translation", t); f_r, _ = sample(g, b, a, n[bone], "rotation", t)
                s_t, _ = sample(sh_g, sh_b, sh_a, sh_n[bone], "translation", t); s_r, _ = sample(sh_g, sh_b, sh_a, sh_n[bone], "rotation", t)
                rest_r_f = local_trs(g, n[bone])[1]; rest_r_s = local_trs(sh_g, sh_n[bone])[1]
                rest_r_sk = local_trs(sk_g, sk_n[bone])[1] if bone in sk_n else None
                r[f"t={t}"] = dict(
                    fbx2gltf_clip_translation=f_t, shipped_translation=s_t,
                    fbx2gltf_clip_rotation=f_r, shipped_rotation=s_r,
                    angle_from_sk_bind_fbx2gltf=qangle_deg(rest_r_sk, f_r) if (f_r and rest_r_sk) else None,
                    angle_from_sk_bind_shipped=qangle_deg(rest_r_sk, s_r) if (s_r and rest_r_sk) else None,
                )
            # translation delta magnitude t0 -> t0.35
            ft0 = r["t=0.0"]["fbx2gltf_clip_translation"]; ft1 = r["t=0.35"]["fbx2gltf_clip_translation"]
            st0 = r["t=0.0"]["shipped_translation"]; st1 = r["t=0.35"]["shipped_translation"]
            r["delta_t0_to_t035"] = dict(fbx2gltf=vlen(tuple(ft1[k]-ft0[k] for k in range(3))) if ft0 and ft1 else None,
                                         shipped=vlen(tuple(st1[k]-st0[k] for k in range(3))) if st0 and st1 else None)
            rows[bone] = r
        anims[clip] = rows
        anims[clip]["_fbx2gltf_clip_node_count"] = len(g["nodes"]); anims[clip]["_fbx2gltf_has_skin"] = "skins" in g
    report["animated"] = anims
    out = OUT
    out.write_text(json.dumps(report, indent=1, default=str), encoding="utf-8")
    print(out)

def _cli():
    global PACK, DIAG, SHIPPED, PREFAB, SK_NAME, ANIM_PREFIX, OUT, BONES
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--pack", required=True, type=Path)
    ap.add_argument("--fbx2gltf-dir", required=True, type=Path)
    ap.add_argument("--shipped", required=True, type=Path)
    ap.add_argument("--prefab", default=PREFAB)
    ap.add_argument("--sk", default=SK_NAME)
    ap.add_argument("--anim-prefix", default=ANIM_PREFIX)
    ap.add_argument("--bone", action="append", help="override bone list")
    ap.add_argument("--out", type=Path, default=Path("compare_paths.json"))
    a = ap.parse_args()
    PACK, DIAG, SHIPPED, PREFAB, SK_NAME, ANIM_PREFIX, OUT = a.pack, a.fbx2gltf_dir, a.shipped, a.prefab, a.sk, a.anim_prefix, a.out
    if a.bone:
        BONES = a.bone
    main()


if __name__ == "__main__":
    _cli()
