//! Parity with an independent parser: fbxcel (a separate implementation of the binary format)
//! reads the same bytes, and its tree, converted to this crate's types, must equal ours.
//!
//! `ALICE_FBX_PARITY=<file.fbx>` with `--ignored` runs the same comparison on any local file.

use alice_fbx::{read_tree, Node, Prop};
use fbxcel::low::v7400::AttributeValue;
use fbxcel::tree::any::AnyTree;
use fbxcel::tree::v7400::NodeHandle;

fn convert(h: NodeHandle<'_>) -> Node {
    Node {
        name: h.name().to_owned(),
        props: h
            .attributes()
            .iter()
            .map(|a| match a {
                AttributeValue::Bool(v) => Prop::Bool(*v),
                AttributeValue::I16(v) => Prop::I16(*v),
                AttributeValue::I32(v) => Prop::I32(*v),
                AttributeValue::I64(v) => Prop::I64(*v),
                AttributeValue::F32(v) => Prop::F32(*v),
                AttributeValue::F64(v) => Prop::F64(*v),
                AttributeValue::ArrBool(v) => Prop::Bools(v.clone()),
                AttributeValue::ArrI32(v) => Prop::I32s(v.clone()),
                AttributeValue::ArrI64(v) => Prop::I64s(v.clone()),
                AttributeValue::ArrF32(v) => Prop::F32s(v.clone()),
                AttributeValue::ArrF64(v) => Prop::F64s(v.clone()),
                AttributeValue::String(v) => Prop::Str(v.as_bytes().to_vec()),
                AttributeValue::Binary(v) => Prop::Raw(v.clone()),
            })
            .collect(),
        children: h.children().map(convert).collect(),
    }
}

fn theirs(bytes: &[u8]) -> Vec<Node> {
    match AnyTree::from_seekable_reader(std::io::Cursor::new(bytes)).expect("fbxcel reads it") {
        AnyTree::V7400(_, tree, _) => tree.root().children().map(convert).collect(),
        _ => panic!("an FBX version fbxcel does not tree"),
    }
}

fn compare(bytes: &[u8]) -> usize {
    let ours = read_tree(bytes).expect("we read it").nodes;
    let theirs = theirs(bytes);
    assert_eq!(ours.len(), theirs.len(), "top-level records");
    for (a, b) in ours.iter().zip(&theirs) {
        assert_eq!(a, b, "record {}", a.name);
    }
    count(&ours)
}

fn count(n: &[Node]) -> usize {
    n.iter().map(|x| 1 + count(&x.children)).sum()
}

#[test]
fn the_fixtures_read_the_same_as_an_independent_parser() {
    for bytes in [
        &include_bytes!("data/tree_7400.fbx")[..],
        &include_bytes!("data/tree_7500.fbx")[..],
        &include_bytes!("data/deep_ok_7400.fbx")[..],
    ] {
        assert!(compare(bytes) > 0, "nothing compared");
    }
}

#[test]
#[ignore = "reads the local file named by ALICE_FBX_PARITY"]
fn a_local_file_reads_the_same_as_an_independent_parser() {
    let path = std::env::var("ALICE_FBX_PARITY").expect("ALICE_FBX_PARITY names a file");
    let n = compare(&std::fs::read(&path).unwrap());
    println!("parity: {n} records equal");
    assert!(n > 0);
}
