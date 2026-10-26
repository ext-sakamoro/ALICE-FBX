#!/usr/bin/env python3
"""Writes the binary FBX fixtures of tests/data from a description in this file.

An independent writer of the binary FBX record format (header, node records with 32-bit
offsets for version < 7500 and 64-bit from 7500, the null record that ends a list of children,
every property type, arrays raw or zlib-compressed), so the reader is checked against a file it
did not produce. The tree written here is spelled out again by hand in tests/tree.rs.

usage: python3 scripts/fbx_fixture.py [out_dir]   (default tests/data)
"""

import struct
import sys
import zlib
from pathlib import Path

MAGIC = b"Kaydara FBX Binary  \x00\x1a\x00"


def prop(kind, value):
    """One property record: its type code and its bytes."""
    if kind == "Y":
        return b"Y" + struct.pack("<h", value)
    if kind == "C":
        return b"C" + struct.pack("<B", 1 if value else 0)
    if kind == "I":
        return b"I" + struct.pack("<i", value)
    if kind == "F":
        return b"F" + struct.pack("<f", value)
    if kind == "D":
        return b"D" + struct.pack("<d", value)
    if kind == "L":
        return b"L" + struct.pack("<q", value)
    if kind in ("S", "R"):
        return kind.encode() + struct.pack("<I", len(value)) + value
    if kind[0] in "fdlib":
        code, compress = kind[0], kind.endswith("z")
        fmt = {"f": "f", "d": "d", "l": "q", "i": "i", "b": "B"}[code]
        raw = b"".join(struct.pack("<" + fmt, v) for v in value)
        data = zlib.compress(raw) if compress else raw
        return code.encode() + struct.pack("<III", len(value), 1 if compress else 0, len(data)) + data
    raise ValueError(kind)


def node(name, props=(), children=None, slack=b"", trail=b"", overrun=0):
    """A record; `slack` is written inside its property list after the properties (counted in
    the list length) and `trail` after the null record that ends its children (counted in its
    end offset), both to describe records that do not fit; `overrun` is added to the end offset
    written (a record that claims to end past where it does)."""
    return (name, list(props), children, slack, trail, overrun)


def write(nodes, version):
    wide = version >= 7500
    head = "<QQQ" if wide else "<III"
    null = b"\x00" * (25 if wide else 13)
    out = bytearray(MAGIC + struct.pack("<I", version))

    def emit(n):
        name, props, children, slack, trail, overrun = n
        body = b"".join(prop(k, v) for k, v in props) + slack
        start = len(out)
        out.extend(b"\x00" * struct.calcsize(head))  # end offset, count, list length: patched below
        out.extend(struct.pack("<B", len(name)) + name.encode())
        out.extend(body)
        if children is not None:
            for c in children:
                emit(c)
            out.extend(null)
        out.extend(trail)
        struct.pack_into(head, out, start, len(out) + overrun, len(props), len(body))

    for n in nodes:
        emit(n)
    out.extend(null)
    # a footer: 16 bytes of id, padding to 16, the version, 120 zero bytes, the footer magic
    out.extend(b"\xfa\xbc\xab\x09\xd0\xc8\xd4\x66\xb1\x76\xfb\x83\x1c\xf7\x26\x7e")
    out.extend(b"\x00" * ((16 - len(out) % 16) % 16))
    out.extend(struct.pack("<I", version) + b"\x00" * 120)
    out.extend(b"\xf8\x5a\x8c\x6a\xde\xf5\xd9\x7e\xec\xe9\x0c\xe3\x75\x8f\x29\x0b")
    return bytes(out)


# every property type, both array encodings, nesting three deep, an empty-children node
TREE = [
    node("Header", [("I", 1003), ("S", b"tree fixture")], [
        node("Version", [("I", 7400)]),
        node("Empty", [], []),
    ]),
    node("Scalars", [("Y", -2), ("C", True), ("C", False), ("I", -70000), ("F", 1.5),
                     ("D", -0.125), ("L", 1 << 40)]),
    node("Arrays", [("f", [0.5, -1.0, 2.25]), ("dz", [1.0, 2.0, 3.0, 4.0]),
                    ("l", [-1, 1 << 33]), ("iz", list(range(-5, 45))), ("b", [1, 0, 1])]),
    node("Strings", [("S", b"Name\x00\x01Model"), ("R", b"\x00\xff\x10")]),
    node("Outer", [("L", 7)], [
        node("Middle", [], [
            node("Inner", [("D", 9.75)]),
        ]),
    ]),
]


def nested(depth):
    """A chain of `depth` records, each the only child of the one before."""
    n = node("Leaf", [("I", depth)])
    for _ in range(depth - 1):
        n = node("Level", [], [n])
    return [n]


def raw_array(code, count, encoding, data):
    """An array property written as given (to describe one that does not fit)."""
    return (code + "!", (count, encoding, data))


_prop = prop


def prop(kind, value):  # noqa: F811  (adds the hand-made arrays of the hostile fixtures)
    if kind.endswith("!"):
        count, encoding, data = value
        return kind[0].encode() + struct.pack("<III", count, encoding, len(data)) + data
    return _prop(kind, value)


def p70(*entries):
    """A Properties70 record: each entry is (name, type, flags, values...)."""
    out = []
    for name, typ, flags, *vals in entries:
        props = [("S", name.encode()), ("S", typ.encode()), ("S", b""), ("S", flags.encode())]
        for v in vals:
            props.append(("D", float(v)) if isinstance(v, float) else ("I", v) if isinstance(v, int) else ("S", v.encode()))
        out.append(node("P", props))
    return node("Properties70", [], out)


def obj(kind, oid, name, cls, sub, children):
    return node(kind, [("L", oid), ("S", name.encode() + b"\x00\x01" + cls.encode()), ("S", sub.encode())], children)


def conn(kind, child, parent, prop=None):
    props = [("S", kind.encode()), ("L", child), ("L", parent)]
    if prop is not None:
        props.append(("S", prop.encode()))
    return node("C", props)


IDENTITY = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]

# A small scene: two bones (Hips -> Spine), a mesh model with a quad and a pentagon, normals
# per polygon vertex (direct), UVs per polygon vertex (index to direct), two materials per polygon,
# a skin of two clusters, and one blend shape channel with one target.
SCENE = [
    node("FBXHeaderExtension", [], [node("FBXVersion", [("I", 7400)])]),
    node("GlobalSettings", [], [p70(
        ("UpAxis", "int", "", 1), ("UpAxisSign", "int", "", 1),
        ("FrontAxis", "int", "", 2), ("FrontAxisSign", "int", "", 1),
        ("CoordAxis", "int", "", 0), ("CoordAxisSign", "int", "", 1),
        ("UnitScaleFactor", "double", "", 100.0),
    )]),
    node("Objects", [], [
        obj("Model", 100, "Hips", "Model", "LimbNode", [p70(
            ("Lcl Translation", "Lcl Translation", "A", 0.0, 1.0, 0.0),
            ("Lcl Rotation", "Lcl Rotation", "A", 0.0, 90.0, 0.0),
            ("Lcl Scaling", "Lcl Scaling", "A", 2.0, 2.0, 2.0),
        )]),
        obj("Model", 101, "Spine", "Model", "LimbNode", [p70(
            ("Lcl Translation", "Lcl Translation", "A", 0.0, 0.5, 0.0),
            ("PreRotation", "Vector3D", "Vector", 0.0, 0.0, 90.0),
            ("RotationOrder", "enum", "", 4),
        )]),
        obj("Model", 200, "Body", "Model", "Mesh", []),
        obj("Geometry", 300, "Body", "Geometry", "Mesh", [
            node("Vertices", [("dz", [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 0.0,
                                      2.0, 0.0, 0.0, 2.0, 1.0, 0.0, 1.5, 1.5, 0.0])]),
            # quad 0 1 2 3, pentagon 1 4 5 6 2 (the last index of a polygon is ~i = -i - 1)
            node("PolygonVertexIndex", [("i", [0, 1, 2, -4, 1, 4, 5, 6, -3])]),
            node("LayerElementNormal", [("I", 0)], [
                node("MappingInformationType", [("S", b"ByPolygonVertex")]),
                node("ReferenceInformationType", [("S", b"Direct")]),
                node("Normals", [("d", [0.0, 0.0, 1.0] * 9)]),
            ]),
            node("LayerElementUV", [("I", 0)], [
                node("Name", [("S", b"map1")]),
                node("MappingInformationType", [("S", b"ByPolygonVertex")]),
                node("ReferenceInformationType", [("S", b"IndexToDirect")]),
                node("UV", [("d", [0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0])]),
                node("UVIndex", [("i", [0, 1, 2, 3, 0, 1, 2, 3, 0])]),
            ]),
            node("LayerElementMaterial", [("I", 0)], [
                node("MappingInformationType", [("S", b"ByPolygon")]),
                node("ReferenceInformationType", [("S", b"IndexToDirect")]),
                node("Materials", [("i", [0, 1])]),
            ]),
        ]),
        obj("Material", 400, "Skin", "Material", "", [p70(("DiffuseColor", "Color", "", 1.0, 0.8, 0.7))]),
        obj("Material", 401, "Cloth", "Material", "", [p70(("DiffuseColor", "Color", "", 0.2, 0.3, 0.9))]),
        obj("Deformer", 500, "Skin", "Deformer", "Skin", []),
        obj("Deformer", 501, "Hips", "SubDeformer", "Cluster", [
            node("Indexes", [("i", [0, 1, 3])]),
            node("Weights", [("d", [1.0, 0.75, 1.0])]),
            node("Transform", [("d", IDENTITY)]),
            node("TransformLink", [("d", IDENTITY[:12] + [0.0, 1.0, 0.0, 1.0])]),
        ]),
        obj("Deformer", 502, "Spine", "SubDeformer", "Cluster", [
            node("Indexes", [("i", [1, 2, 4, 5, 6])]),
            node("Weights", [("d", [0.25, 1.0, 1.0, 1.0, 1.0])]),
            node("Transform", [("d", IDENTITY)]),
            node("TransformLink", [("d", IDENTITY[:12] + [0.0, 1.5, 0.0, 1.0])]),
        ]),
        obj("Deformer", 600, "Body", "Deformer", "BlendShape", []),
        obj("Deformer", 601, "smile", "SubDeformer", "BlendShapeChannel", [
            node("DeformPercent", [("D", 25.0)]),
            node("FullWeights", [("d", [80.0])]),
        ]),
        obj("Geometry", 602, "smile", "Geometry", "Shape", [
            node("Indexes", [("i", [2, 6])]),
            node("Vertices", [("d", [0.0, 0.25, 0.0, 0.0, 0.5, 0.0])]),
        ]),
    ]),
    node("Connections", [], [
        conn("OO", 100, 0), conn("OO", 101, 100), conn("OO", 200, 0),
        conn("OO", 300, 200), conn("OO", 400, 200), conn("OO", 401, 200),
        conn("OO", 500, 300), conn("OO", 501, 500), conn("OO", 502, 500),
        conn("OO", 100, 501), conn("OO", 101, 502),
        conn("OO", 600, 300), conn("OO", 601, 600), conn("OO", 602, 601),
    ]),
]

HOSTILE = {
    # 66 nested records: the deepest list is at depth 65, past the limit of 64
    "deep_7400.fbx": nested(66),
    # 64 nested records: the deepest list is at depth 63, read
    "deep_ok_7400.fbx": nested(64),
    # 3 floats declared, a zlib stream that inflates to 1 MiB
    "bomb_7400.fbx": [node("A", [raw_array("f", 3, 1, zlib.compress(b"\x00" * (1 << 20)))])],
    # 5 floats declared, 4 stored (raw)
    "short_7400.fbx": [node("A", [raw_array("f", 5, 0, struct.pack("<4f", 1, 2, 3, 4))])],
    # 4 floats declared, a zlib stream that inflates to 3 (fewer than declared)
    "inflate_short_7400.fbx": [node("A", [raw_array("f", 4, 1, zlib.compress(struct.pack("<3f", 1, 2, 3)))])],
    # a property list declared 4 bytes longer than its one property
    "slack_7400.fbx": [node("A", [("I", 7)], slack=b"\x00\x00\x00\x00")],
    # 4 bytes after the null record that ends a record's children, inside the record
    "trail_7400.fbx": [node("A", [], [node("B", [("I", 1)])], trail=b"\x00\x00\x00\x00")],
    # a parent that claims to end 1 byte before its child does: A (at 27) holds B (at 41), which
    # holds C and ends consistently; A's end offset is written 14 bytes short (before its own
    # 13-byte null record and the last byte of B)
    "overrun_7400.fbx": [node("A", [], [node("B", [], [node("C", [("I", 1)])])], overrun=-14), node("D", [("I", 2)])],
    # 2^27 doubles (1 GiB) declared, past the array limit
    "huge_7400.fbx": [node("A", [raw_array("d", 1 << 27, 0, b"")])],
    # bools stored as 2, 0 and 255: any non-zero byte is true
    "bools_7400.fbx": [node("A", [raw_array("b", 3, 0, bytes([2, 0, 255]))])],
    # 4 floats declared and stored, encoding 2 (unknown)
    "encoding_7400.fbx": [node("A", [raw_array("f", 4, 2, struct.pack("<4f", 1, 2, 3, 4))])],
}


def main():
    out = Path(sys.argv[1] if len(sys.argv) > 1 else "tests/data")
    out.mkdir(parents=True, exist_ok=True)
    for version in (7400, 7500):
        (out / f"tree_{version}.fbx").write_bytes(write(TREE, version))
    for name, nodes in HOSTILE.items():
        (out / name).write_bytes(write(nodes, 7400))
    (out / "scene_7400.fbx").write_bytes(write(SCENE, 7400))
    print("wrote", ", ".join([f"tree_{v}.fbx" for v in (7400, 7500)] + list(HOSTILE) + ["scene_7400.fbx"]))


if __name__ == "__main__":
    main()
