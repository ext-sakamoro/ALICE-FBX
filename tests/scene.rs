//! The scene of a binary FBX file.
//!
//! oracle: `scripts/fbx_fixture.py` writes `tests/data/scene_7400.fbx` (its `SCENE`); every value
//! below is that description worked by hand:
//! - models Hips (bone, root, T (0, 1, 0), R (0, 90, 0), S 2), Spine (bone, child of Hips,
//!   T (0, 0.5, 0), pre-rotation (0, 0, 90), rotation order 4 = ZXY) and Body (mesh holder);
//! - Hips' local matrix is T * Ry(90) * S: Ry(90) takes x to -z and z to x, so its columns are
//!   (0, 0, -2), (0, 2, 0), (2, 0, 0) and the translation (0, 1, 0);
//! - Spine's local matrix is T * Rz(90): Rz(90) takes x to y and y to -x; its world matrix is
//!   Hips' times it, so its origin is Hips' image of (0, 0.5, 0) = (0, 1, 0) + 2 (0, 0.5, 0) =
//!   (0, 2, 0);
//! - the mesh: 7 points; a quad 0 1 2 3 and a pentagon 1 4 5 6 2 (9 corners, polygons start at 0
//!   and 4); fan triangles (0 1 2) (0 2 3) (4 5 6) (4 6 7) (4 7 8); normals (0, 0, 1) on every
//!   corner; UVs by index 0 1 2 3 0 1 2 3 0 into (0,0) (1,0) (1,1) (0,1); material slots 0 and 1
//!   per polygon, the slots being Skin and Cloth in connection order;
//! - the skin: cluster Hips on points 0 1 3 (weights 1, 0.75, 1) and Spine on 1 2 4 5 6
//!   (0.25, 1, 1, 1, 1), bind translations (0, 1, 0) and (0, 1.5, 0);
//! - one blend channel `smile` (25 %) with one target at 80 % moving point 2 by (0, 0.25, 0) and
//!   point 6 by (0, 0.5, 0);
//! - y up, z front, x coord (all +), 100 cm per unit.

// exact float comparisons on purpose: every expected value is exactly representable and is
// what the fixture writer wrote
#![allow(clippy::float_cmp)]

use alice_fbx::scene::{Axis, Model, RotationOrder};
use alice_fbx::{read_scene, read_tree, FbxError};

const SCENE: &[u8] = include_bytes!("data/scene_7400.fbx");

fn close(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-12)
}

fn scene() -> alice_fbx::Scene {
    read_scene(&read_tree(SCENE).unwrap()).unwrap()
}

#[test]
fn settings_models_and_hierarchy() {
    let s = scene();
    assert_eq!(s.settings.up, Axis { axis: 1, sign: 1 });
    assert_eq!(s.settings.front, Axis { axis: 2, sign: 1 });
    assert_eq!(s.settings.coord, Axis { axis: 0, sign: 1 });
    assert!((s.settings.unit_scale - 100.0).abs() < 1e-12);
    let names: Vec<_> = s
        .models
        .iter()
        .map(|m| (m.name.as_str(), m.kind.as_str(), m.parent))
        .collect();
    assert_eq!(
        names,
        vec![
            ("Hips", "LimbNode", None),
            ("Spine", "LimbNode", Some(0)),
            ("Body", "Mesh", None)
        ]
    );
    assert_eq!(s.models[1].rotation_order, RotationOrder::Zxy);
    assert!(close(
        &s.models[0].local_matrix(),
        &[0.0, 0.0, -2.0, 0.0, 0.0, 2.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0]
    ));
    assert!(close(
        &s.models[1].local_matrix(),
        &[0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.5, 0.0, 1.0]
    ));
    let w = s.world_matrix(1);
    assert!(close(&w[12..15], &[0.0, 2.0, 0.0]), "{:?}", &w[12..15]);
}

#[test]
fn rotation_order_changes_the_matrix() {
    // (90, 90, 0) applied x then y: x -> -z, y -> x, z -> -y
    // applied y then x (YXZ): x -> y, y -> z, z -> x
    let mut m = scene().models[0].clone();
    m.translation = [0.0; 3];
    m.scaling = [1.0; 3];
    m.rotation = [90.0, 90.0, 0.0];
    m.rotation_order = RotationOrder::Xyz;
    assert!(close(
        &m.local_matrix()[..12],
        &[0.0, 0.0, -1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0]
    ));
    m.rotation_order = RotationOrder::Yxz;
    assert!(close(
        &m.local_matrix()[..12],
        &[0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0]
    ));
    let _: &Model = &m;
}

#[test]
fn the_mesh_and_its_layers() {
    let s = scene();
    assert_eq!(s.meshes.len(), 1);
    let m = &s.meshes[0];
    assert_eq!((m.name.as_str(), m.model), ("Body", Some(2)));
    assert_eq!(m.positions.len(), 7);
    assert_eq!(m.positions[6], [1.5, 1.5, 0.0]);
    assert_eq!(m.polygon_vertices, vec![0, 1, 2, 3, 1, 4, 5, 6, 2]);
    assert_eq!(m.polygon_starts, vec![0, 4, 9]);
    assert_eq!(m.polygon_count(), 2);
    assert_eq!(
        m.triangles(),
        vec![[0, 1, 2], [0, 2, 3], [4, 5, 6], [4, 6, 7], [4, 7, 8]]
    );
    assert_eq!(m.normals.as_deref(), Some(&[[0.0, 0.0, 1.0]; 9][..]));
    assert_eq!(m.uvs.len(), 1);
    assert_eq!(m.uvs[0].name, "map1");
    let q = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    assert_eq!(
        m.uvs[0].uv,
        vec![q[0], q[1], q[2], q[3], q[0], q[1], q[2], q[3], q[0]]
    );
    assert_eq!(m.polygon_materials.as_deref(), Some(&[0, 1][..]));
    let mats: Vec<_> = m
        .materials
        .iter()
        .map(|&i| s.materials[i].name.as_str())
        .collect();
    assert_eq!(mats, vec!["Skin", "Cloth"]);
    assert_eq!(s.materials[1].diffuse, Some([0.2, 0.3, 0.9]));
}

#[test]
fn the_skin_and_the_blend_shape() {
    let s = scene();
    let m = &s.meshes[0];
    let skin = m.skin.as_ref().expect("a skin");
    assert_eq!(skin.len(), 2);
    assert_eq!(
        (
            skin[0].bone,
            skin[0].indices.as_slice(),
            skin[0].weights.as_slice()
        ),
        (Some(0), &[0, 1, 3][..], &[1.0, 0.75, 1.0][..])
    );
    assert_eq!(
        (skin[1].bone, skin[1].indices.as_slice()),
        (Some(1), &[1, 2, 4, 5, 6][..])
    );
    assert_eq!(skin[1].weights, vec![0.25, 1.0, 1.0, 1.0, 1.0]);
    assert_eq!(&skin[0].transform_link[12..15], &[0.0, 1.0, 0.0]);
    assert_eq!(&skin[1].transform_link[12..15], &[0.0, 1.5, 0.0]);
    assert_eq!(m.blend_channels.len(), 1);
    let c = &m.blend_channels[0];
    assert_eq!((c.name.as_str(), c.deform_percent), ("smile", 25.0));
    assert_eq!(c.targets.len(), 1);
    let t = &c.targets[0];
    assert_eq!((t.full_weight, t.indices.as_slice()), (80.0, &[2, 6][..]));
    assert_eq!(t.deltas, vec![[0.0, 0.25, 0.0], [0.0, 0.5, 0.0]]);
}

/// A copy of the fixture's tree with one record changed (for the refusals)
fn edited(f: impl FnOnce(&mut alice_fbx::Document)) -> Result<alice_fbx::Scene, FbxError> {
    let mut doc = read_tree(SCENE).unwrap();
    f(&mut doc);
    read_scene(&doc)
}

fn geometry(doc: &mut alice_fbx::Document) -> &mut alice_fbx::Node {
    let objects = doc.nodes.iter_mut().find(|n| n.name == "Objects").unwrap();
    objects
        .children
        .iter_mut()
        .find(|n| n.name == "Geometry")
        .unwrap()
}

fn child<'a>(n: &'a mut alice_fbx::Node, name: &str) -> &'a mut alice_fbx::Node {
    n.children.iter_mut().find(|c| c.name == name).unwrap()
}

/// One change to the fixture's tree
type Edit = Box<dyn FnOnce(&mut alice_fbx::Document)>;

#[test]
fn what_does_not_fit_is_refused() {
    use alice_fbx::Prop;
    let cases: Vec<(&str, Edit)> = vec![
        (
            "a corner naming a point that does not exist",
            Box::new(|d| {
                child(geometry(d), "PolygonVertexIndex").props[0] = Prop::I32s(vec![0, 1, 7, -4]);
            }),
        ),
        (
            "an open last polygon",
            Box::new(|d| {
                child(geometry(d), "PolygonVertexIndex").props[0] = Prop::I32s(vec![0, 1, 2, 3]);
            }),
        ),
        (
            "a polygon of two corners",
            Box::new(|d| {
                child(geometry(d), "PolygonVertexIndex").props[0] =
                    Prop::I32s(vec![0, -2, 1, 2, -4]);
            }),
        ),
        (
            "a UV index past the UVs",
            Box::new(|d| {
                let uv = child(geometry(d), "LayerElementUV");
                child(uv, "UVIndex").props[0] = Prop::I32s(vec![0, 1, 2, 3, 0, 1, 2, 3, 4]);
            }),
        ),
        (
            "too few normals",
            Box::new(|d| {
                let n = child(geometry(d), "LayerElementNormal");
                child(n, "Normals").props[0] = Prop::F64s(vec![0.0, 0.0, 1.0]);
            }),
        ),
        (
            "an unread mapping",
            Box::new(|d| {
                let n = child(geometry(d), "LayerElementNormal");
                child(n, "MappingInformationType").props[0] = Prop::Str(b"ByEdge".to_vec());
            }),
        ),
        (
            "a material slot past the model's materials",
            Box::new(|d| {
                let m = child(geometry(d), "LayerElementMaterial");
                child(m, "Materials").props[0] = Prop::I32s(vec![0, 2]);
            }),
        ),
        (
            "a non-finite point",
            Box::new(|d| {
                let v = child(geometry(d), "Vertices");
                if let Prop::F64s(p) = &mut v.props[0] {
                    p[4] = f64::NAN;
                }
            }),
        ),
        (
            "a connection to an unknown object",
            Box::new(|d| {
                let c = d
                    .nodes
                    .iter_mut()
                    .find(|n| n.name == "Connections")
                    .unwrap();
                c.children[1].props[2] = Prop::I64(999);
            }),
        ),
        (
            "per-vertex full weights that differ",
            Box::new(|d| set_full_weights(d, vec![100.0, 50.0])),
        ),
        (
            "full weights of neither form",
            Box::new(|d| set_full_weights(d, vec![100.0; 3])),
        ),
    ];
    let n = cases.len();
    for (what, edit) in cases {
        assert!(matches!(edited(edit), Err(FbxError::BadScene(_))), "{what}");
    }
    assert_eq!(n, 11);
}

fn set_full_weights(d: &mut alice_fbx::Document, w: Vec<f64>) {
    let objects = d.nodes.iter_mut().find(|n| n.name == "Objects").unwrap();
    let channel = objects
        .children
        .iter_mut()
        .find(|n| matches!(n.props.get(2), Some(alice_fbx::Prop::Str(s)) if s == b"BlendShapeChannel"))
        .unwrap();
    child(channel, "FullWeights").props[0] = alice_fbx::Prop::F64s(w);
}

#[test]
fn full_weights_per_vertex_of_a_single_target_are_its_full_weight() {
    // the target moves 2 points; some exporters write one equal weight per point
    let s = edited(|d| set_full_weights(d, vec![60.0, 60.0])).unwrap();
    assert_eq!(s.meshes[0].blend_channels[0].targets[0].full_weight, 60.0);
    // with no FullWeights a target is full at 100
    let s = edited(|d| {
        let objects = d.nodes.iter_mut().find(|n| n.name == "Objects").unwrap();
        for o in &mut objects.children {
            o.children.retain(|c| c.name != "FullWeights");
        }
    })
    .unwrap();
    assert_eq!(s.meshes[0].blend_channels[0].targets[0].full_weight, 100.0);
}

fn set_setting(d: &mut alice_fbx::Document, key: &[u8], value: alice_fbx::Prop) {
    let g = d
        .nodes
        .iter_mut()
        .find(|n| n.name == "GlobalSettings")
        .unwrap();
    let p70 = child(g, "Properties70");
    let p = p70
        .children
        .iter_mut()
        .find(|p| matches!(p.props.first(), Some(alice_fbx::Prop::Str(s)) if s == key))
        .unwrap();
    p.props[4] = value;
}

#[test]
fn axis_signs_are_read_and_bad_axes_refused() {
    use alice_fbx::Prop;
    let s = edited(|d| set_setting(d, b"FrontAxisSign", Prop::I32(-1))).unwrap();
    assert_eq!(s.settings.front, Axis { axis: 2, sign: -1 });
    assert!(matches!(
        edited(|d| set_setting(d, b"UpAxis", Prop::I32(3))),
        Err(FbxError::BadScene(_))
    ));
    assert!(matches!(
        edited(|d| set_setting(d, b"UpAxisSign", Prop::I32(0))),
        Err(FbxError::BadScene(_))
    ));
    assert!(matches!(
        edited(|d| set_setting(d, b"UnitScaleFactor", Prop::F64(0.0))),
        Err(FbxError::BadScene(_))
    ));
}
