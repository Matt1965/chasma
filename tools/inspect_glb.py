import struct
import json
import sys

path = sys.argv[1]
with open(path, "rb") as f:
    f.read(4)  # magic
    f.read(4)  # version
    f.read(4)  # length
    chunk0_len = struct.unpack("<I", f.read(4))[0]
    f.read(4)  # type
    json_data = f.read(chunk0_len)
    g = json.loads(json_data)

nodes = g.get("nodes", [])
scenes = g.get("scenes", [])
print("scene roots:", scenes[0]["nodes"] if scenes else [])
for i, n in enumerate(nodes):
    t = n.get("translation", [0, 0, 0])
    name = n.get("name", f"node{i}")
    children = n.get("children", [])
    print(f"node {i} {name!r}: t={t}, children={children}")

accessors = g.get("accessors", [])
mins, maxs = [], []
for mesh in g.get("meshes", []):
    for prim in mesh.get("primitives", []):
        pos_idx = prim["attributes"].get("POSITION")
        if pos_idx is None:
            continue
        acc = accessors[pos_idx]
        if "min" in acc and "max" in acc:
            mins.append(acc["min"])
            maxs.append(acc["max"])
if mins:
    mn = [min(c[i] for c in mins) for i in range(3)]
    mx = [max(c[i] for c in maxs) for i in range(3)]
    print("overall mesh bounds min", mn, "max", mx)
    print("center", [(mn[i] + mx[i]) / 2 for i in range(3)])
    print("size", [mx[i] - mn[i] for i in range(3)])
