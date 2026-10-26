//! The node tree of binary FBX.
//!
//! oracle: the files in `tests/data` are written by `scripts/fbx_fixture.py`, an independent writer
//! of the record format; the tree it writes is spelled out again here by hand. Version 7400 uses
//! 32-bit record words, 7500 64-bit ones; both hold the same tree.

use alice_fbx::{read_tree, tree, FbxError, Node, Prop};

fn n(name: &str, props: Vec<Prop>, children: Vec<Node>) -> Node {
    Node {
        name: name.to_owned(),
        props,
        children,
    }
}

fn expected() -> Vec<Node> {
    vec![
        n(
            "Header",
            vec![Prop::I32(1003), Prop::Str(b"tree fixture".to_vec())],
            vec![
                n("Version", vec![Prop::I32(7400)], vec![]),
                n("Empty", vec![], vec![]),
            ],
        ),
        n(
            "Scalars",
            vec![
                Prop::I16(-2),
                Prop::Bool(true),
                Prop::Bool(false),
                Prop::I32(-70000),
                Prop::F32(1.5),
                Prop::F64(-0.125),
                Prop::I64(1 << 40),
            ],
            vec![],
        ),
        n(
            "Arrays",
            vec![
                Prop::F32s(vec![0.5, -1.0, 2.25]),
                Prop::F64s(vec![1.0, 2.0, 3.0, 4.0]),
                Prop::I64s(vec![-1, 1 << 33]),
                Prop::I32s((-5..45).collect()),
                Prop::Bools(vec![true, false, true]),
            ],
            vec![],
        ),
        n(
            "Strings",
            vec![
                Prop::Str(b"Name\x00\x01Model".to_vec()),
                Prop::Raw(vec![0x00, 0xff, 0x10]),
            ],
            vec![],
        ),
        n(
            "Outer",
            vec![Prop::I64(7)],
            vec![n(
                "Middle",
                vec![],
                vec![n("Inner", vec![Prop::F64(9.75)], vec![])],
            )],
        ),
    ]
}

const TREE_7400: &[u8] = include_bytes!("data/tree_7400.fbx");
const TREE_7500: &[u8] = include_bytes!("data/tree_7500.fbx");

#[test]
fn both_word_sizes_read_the_written_tree() {
    for (bytes, version) in [(TREE_7400, 7400), (TREE_7500, 7500)] {
        let doc = read_tree(bytes).unwrap();
        assert_eq!(doc.version, version);
        assert_eq!(doc.nodes, expected(), "version {version}");
    }
    let doc = read_tree(TREE_7400).unwrap();
    assert_eq!(
        doc.node("Outer")
            .unwrap()
            .child("Middle")
            .unwrap()
            .children
            .len(),
        1
    );
    assert!(doc.node("Missing").is_none());
}

#[test]
fn every_truncation_is_refused_without_a_panic() {
    for bytes in [TREE_7400, TREE_7500] {
        // the footer is 16 bytes of id, padding to 16, the version, 120 zero bytes and 16 bytes of
        // magic; the tree (ending with its null record) is everything before it. Every list must
        // end with a null record, so every cut before the tree's end is refused, even one that
        // falls between two records
        let id = b"\xfa\xbc\xab\x09\xd0\xc8\xd4\x66\xb1\x76\xfb\x83\x1c\xf7\x26\x7e";
        let tree_end = bytes
            .windows(16)
            .position(|w| w == id)
            .expect("the footer id");
        let refused = (0..tree_end)
            .filter(|&len| read_tree(&bytes[..len]).is_err())
            .count();
        assert_eq!(refused, tree_end, "cuts refused");
        assert!(read_tree(&bytes[..tree_end]).is_ok());
    }
}

#[test]
fn a_changed_byte_never_panics() {
    for bytes in [TREE_7400, TREE_7500] {
        for at in 27..bytes.len() {
            for x in [0x00u8, 0xff, 0x7f] {
                let mut b = bytes.to_vec();
                b[at] = x;
                let _ = read_tree(&b);
            }
        }
    }
}

#[test]
fn the_magic_and_the_version_are_checked() {
    assert_eq!(read_tree(b"Kaydara FBX Binary"), Err(FbxError::NotFbx));
    let mut b = TREE_7400.to_vec();
    b[0] = b'k';
    assert_eq!(read_tree(&b), Err(FbxError::NotFbx));
    for v in [7099u32, 7701, 6100] {
        let mut b = TREE_7400.to_vec();
        b[23..27].copy_from_slice(&v.to_le_bytes());
        assert_eq!(read_tree(&b), Err(FbxError::UnsupportedVersion(v)));
    }
    let mut b = TREE_7400.to_vec();
    b[23..27].copy_from_slice(&7100u32.to_le_bytes());
    assert_eq!(read_tree(&b).unwrap().nodes, expected());
}

#[test]
fn nesting_past_the_limit_is_refused() {
    assert_eq!(tree::MAX_DEPTH, 64);
    let ok = read_tree(include_bytes!("data/deep_ok_7400.fbx")).unwrap();
    let mut depth = 0;
    let mut at = &ok.nodes[0];
    while let Some(c) = at.children.first() {
        depth += 1;
        at = c;
    }
    assert_eq!((depth, at.name.as_str()), (63, "Leaf"));
    assert!(matches!(
        read_tree(include_bytes!("data/deep_7400.fbx")),
        Err(FbxError::TooDeep { .. })
    ));
}

#[test]
fn an_array_must_hold_exactly_its_declared_length() {
    for (file, why) in [
        (
            &include_bytes!("data/bomb_7400.fbx")[..],
            "zlib data that does not inflate within the declared length",
        ),
        (
            &include_bytes!("data/short_7400.fbx")[..],
            "the stored length does not match the element count",
        ),
        (
            &include_bytes!("data/encoding_7400.fbx")[..],
            "an unknown encoding",
        ),
        // 2^27 doubles declared: 1 GiB, past the 256 MiB limit
        (
            &include_bytes!("data/huge_7400.fbx")[..],
            "longer than the array limit",
        ),
        // the stream inflates to 12 bytes, 4 floats are declared
        (
            &include_bytes!("data/inflate_short_7400.fbx")[..],
            "inflated length does not match the element count",
        ),
    ] {
        assert!(
            matches!(read_tree(file), Err(FbxError::BadArray { why: w, .. }) if w == why),
            "{why}: {:?}",
            read_tree(file)
        );
    }
}

#[test]
fn a_record_must_fill_exactly_its_declared_lengths() {
    // one 5-byte property in a property list declared 9 bytes long
    assert!(matches!(
        read_tree(include_bytes!("data/slack_7400.fbx")),
        Err(FbxError::BadProperty { why, .. }) if why == "the property list length does not match"
    ));
    // a record whose end offset is 4 bytes past the null record that ends its children
    assert!(matches!(
        read_tree(include_bytes!("data/trail_7400.fbx")),
        Err(FbxError::BadOffset { .. })
    ));
}

#[test]
fn a_child_may_not_end_past_its_parent() {
    // A (at 27) claims to end 1 byte before its child B (at 41) does: B is refused where it
    // starts, not read through to a cut further on
    assert_eq!(
        read_tree(include_bytes!("data/overrun_7400.fbx")),
        Err(FbxError::BadOffset { at: 41 })
    );
}

#[test]
fn any_non_zero_bool_byte_is_true() {
    let doc = read_tree(include_bytes!("data/bools_7400.fbx")).unwrap();
    assert_eq!(
        doc.nodes[0].props,
        vec![Prop::Bools(vec![true, false, true])]
    );
}
