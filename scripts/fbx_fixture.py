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


def node(name, props=(), children=None):
    return (name, list(props), children)


def write(nodes, version):
    wide = version >= 7500
    head = "<QQQ" if wide else "<III"
    null = b"\x00" * (25 if wide else 13)
    out = bytearray(MAGIC + struct.pack("<I", version))

    def emit(n):
        name, props, children = n
        body = b"".join(prop(k, v) for k, v in props)
        start = len(out)
        out.extend(b"\x00" * struct.calcsize(head))  # end offset, count, list length: patched below
        out.extend(struct.pack("<B", len(name)) + name.encode())
        out.extend(body)
        if children is not None:
            for c in children:
                emit(c)
            out.extend(null)
        struct.pack_into(head, out, start, len(out), len(props), len(body))

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


HOSTILE = {
    # 66 nested records: the deepest list is at depth 65, past the limit of 64
    "deep_7400.fbx": nested(66),
    # 64 nested records: the deepest list is at depth 63, read
    "deep_ok_7400.fbx": nested(64),
    # 3 floats declared, a zlib stream that inflates to 1 MiB
    "bomb_7400.fbx": [node("A", [raw_array("f", 3, 1, zlib.compress(b"\x00" * (1 << 20)))])],
    # 5 floats declared, 4 stored (raw)
    "short_7400.fbx": [node("A", [raw_array("f", 5, 0, struct.pack("<4f", 1, 2, 3, 4))])],
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
    print("wrote", ", ".join([f"tree_{v}.fbx" for v in (7400, 7500)] + list(HOSTILE)))


if __name__ == "__main__":
    main()
