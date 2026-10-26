# ALICE-FBX

A bounds-checked reader of binary FBX files.

It is for loading models into ALICE programs from the format most modelling tools export. It is
not a writer, it does not read ASCII FBX, and it does not evaluate animation curves or
textures.

## Install

```sh
cargo add alice-fbx --git https://github.com/ext-sakamoro/ALICE-FBX
```

## Example

```rust
let bytes = std::fs::read("model.fbx")?;
let doc = alice_fbx::read_tree(&bytes)?;
for node in &doc.nodes {
    println!("{} ({} properties, {} children)", node.name, node.props.len(), node.children.len());
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

## What it reads

| Layer | Status |
|---|---|
| Node tree: records, every property type, raw and zlib arrays, versions 7100-7700 (32- and 64-bit record words) | read |
| Scene (`read_scene`): models with their transforms and hierarchy, meshes with per-corner normals and UVs and per-polygon materials, skin clusters, blend shape channels, materials, axes and unit | read |
| ASCII FBX | not read |

## What it refuses

A file is read only if every offset, length and array fits it. Otherwise `read_tree` returns an
`FbxError` that names the offset where the file stopped fitting. It never reads out of bounds
and never panics.

- a missing magic, or a version outside 7100-7700
- a record whose end offset or lengths leave the file or its parent record
- a record list without its closing null record (a cut file)
- an array whose stored or inflated length differs from its declared element count, or that
  would hold more than 256 MiB
- records nested deeper than 64
- in the scene: an index outside what it names (a point, a UV, a material slot), arrays of
  mismatched lengths, an open polygon or one of fewer than 3 corners, a non-finite number, a
  layer mapping or rotation order it does not read, a connection to an object that does not
  exist, a model that is its own ancestor

## How it is checked

- `scripts/fbx_fixture.py` is an independent writer of the binary format. It writes the
  fixtures in `tests/data`, and the tests spell the same tree out by hand.
- The fixtures are also read with an independent parser (fbxcel, a test-only dependency), and the
  two trees must be equal. `ALICE_FBX_PARITY=<file> cargo test --test parity -- --ignored` runs
  the same comparison on any local file.
- Every truncation of the fixtures is refused, and changing any byte never panics.
- Three fuzz targets run in CI: any bytes, bytes behind a valid header, and bytes read as a scene.

## MSRV

Rust 1.85 (`rust-version` in Cargo.toml; CI builds the library with it).

## Build and test

```sh
cargo test
python3 scripts/fbx_fixture.py   # rewrites tests/data (CI checks they are unchanged)
scripts/preflight.sh --quick     # the CI steps, without fuzz and the network checks
```

## Licence

`LicenseRef-Proprietary`: see [LICENSE-Proprietary](LICENSE-Proprietary) and
[TRADEMARK_NOTICE](TRADEMARK_NOTICE).
