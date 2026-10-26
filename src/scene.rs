//! The scene of a binary FBX file, read from its node tree: models (bones and mesh holders) with
//! their transforms and hierarchy, meshes with their per-corner layers, skins, blend shapes and
//! materials, joined by the file's connections. Every index is checked against what it names.

use std::collections::HashMap;

use crate::tree::{Document, Node, Prop};
use crate::FbxError;

/// One axis of the file's coordinate system: 0 = x, 1 = y, 2 = z, and its sign
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Axis {
    /// 0, 1 or 2
    pub axis: u8,
    /// 1 or -1
    pub sign: i8,
}

/// The file's axes and unit (`GlobalSettings`); absent values take FBX's defaults (y up, z
/// front, x coord, all positive, 1 cm units)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    /// `UpAxis` / `UpAxisSign`
    pub up: Axis,
    /// `FrontAxis` / `FrontAxisSign`
    pub front: Axis,
    /// `CoordAxis` / `CoordAxisSign`
    pub coord: Axis,
    /// `UnitScaleFactor`: centimetres per file unit
    pub unit_scale: f64,
}

/// The order Euler angles are applied in, the first letter first
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationOrder {
    /// x, then y, then z (FBX 0, the default)
    Xyz,
    /// FBX 1
    Xzy,
    /// FBX 2
    Yzx,
    /// FBX 3
    Yxz,
    /// FBX 4
    Zxy,
    /// FBX 5
    Zyx,
}

/// A `Model`: a bone (`LimbNode`), a mesh holder (`Mesh`), a `Null` or another kind
#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    /// The object id
    pub id: i64,
    /// The name (the part before the `\0\x01` class separator)
    pub name: String,
    /// The sub-class: `LimbNode`, `Mesh`, `Null`, ...
    pub kind: String,
    /// The parent model, if any
    pub parent: Option<usize>,
    /// `Lcl Translation`
    pub translation: [f64; 3],
    /// `Lcl Rotation` (degrees)
    pub rotation: [f64; 3],
    /// `Lcl Scaling`
    pub scaling: [f64; 3],
    /// `PreRotation` (degrees)
    pub pre_rotation: [f64; 3],
    /// `PostRotation` (degrees)
    pub post_rotation: [f64; 3],
    /// `RotationOffset`
    pub rotation_offset: [f64; 3],
    /// `RotationPivot`
    pub rotation_pivot: [f64; 3],
    /// `ScalingOffset`
    pub scaling_offset: [f64; 3],
    /// `ScalingPivot`
    pub scaling_pivot: [f64; 3],
    /// `RotationOrder`
    pub rotation_order: RotationOrder,
}

/// A per-corner UV set
#[derive(Debug, Clone, PartialEq)]
pub struct UvSet {
    /// The layer's `Name`
    pub name: String,
    /// One UV per polygon corner
    pub uv: Vec<[f64; 2]>,
}

/// A skin cluster: the vertices one bone moves
#[derive(Debug, Clone, PartialEq)]
pub struct Cluster {
    /// The bone model, if the cluster is connected to one
    pub bone: Option<usize>,
    /// Control point indices
    pub indices: Vec<u32>,
    /// One weight per index
    pub weights: Vec<f64>,
    /// `Transform` (column-major): the mesh's bind matrix
    pub transform: [f64; 16],
    /// `TransformLink` (column-major): the bone's bind matrix
    pub transform_link: [f64; 16],
}

/// One target shape of a blend shape channel
#[derive(Debug, Clone, PartialEq)]
pub struct BlendTarget {
    /// The shape's name
    pub name: String,
    /// The channel weight (percent) at which this target is fully applied
    pub full_weight: f64,
    /// Control point indices
    pub indices: Vec<u32>,
    /// One position delta per index
    pub deltas: Vec<[f64; 3]>,
    /// One normal delta per index, when present
    pub normals: Option<Vec<[f64; 3]>>,
}

/// A blend shape channel (a morph with one or more in-between targets)
#[derive(Debug, Clone, PartialEq)]
pub struct BlendChannel {
    /// The channel's name
    pub name: String,
    /// `DeformPercent` (0 to 100)
    pub deform_percent: f64,
    /// The targets, in order of their full weights
    pub targets: Vec<BlendTarget>,
}

/// A polygon mesh with its layers resolved to corners (normals, UVs) and polygons (materials)
#[derive(Debug, Clone, PartialEq)]
pub struct Mesh {
    /// The geometry's object id
    pub id: i64,
    /// The geometry's name
    pub name: String,
    /// The model the geometry belongs to
    pub model: Option<usize>,
    /// Control points
    pub positions: Vec<[f64; 3]>,
    /// The control point of every polygon corner, polygon after polygon
    pub polygon_vertices: Vec<u32>,
    /// Where each polygon's corners start in `polygon_vertices`, and the end (one longer than
    /// the polygon count)
    pub polygon_starts: Vec<u32>,
    /// One normal per corner, when the mesh has a normal layer
    pub normals: Option<Vec<[f64; 3]>>,
    /// The UV sets, one UV per corner each
    pub uvs: Vec<UvSet>,
    /// The material slot of every polygon, when the mesh has a material layer
    pub polygon_materials: Option<Vec<u32>>,
    /// The scene materials of the slots, in the model's connection order
    pub materials: Vec<usize>,
    /// The skin, when the geometry has one
    pub skin: Option<Vec<Cluster>>,
    /// The blend shape channels
    pub blend_channels: Vec<BlendChannel>,
}

impl Mesh {
    /// The number of polygons
    #[must_use]
    pub fn polygon_count(&self) -> usize {
        self.polygon_starts.len().saturating_sub(1)
    }

    /// The polygons as triangles of corner indices, each polygon a fan from its first corner
    #[must_use]
    pub fn triangles(&self) -> Vec<[u32; 3]> {
        let mut out = Vec::new();
        for w in self.polygon_starts.windows(2) {
            for k in w[0] + 1..w[1] - 1 {
                out.push([w[0], k, k + 1]);
            }
        }
        out
    }
}

/// A material
#[derive(Debug, Clone, PartialEq)]
pub struct Material {
    /// The object id
    pub id: i64,
    /// The name
    pub name: String,
    /// `DiffuseColor`, when present
    pub diffuse: Option<[f64; 3]>,
}

/// A whole scene
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    /// Axes and unit
    pub settings: Settings,
    /// Every model
    pub models: Vec<Model>,
    /// Every mesh geometry
    pub meshes: Vec<Mesh>,
    /// Every material
    pub materials: Vec<Material>,
}

type Mat = [f64; 16];

const IDENTITY: Mat = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

fn mul(a: &Mat, b: &Mat) -> Mat {
    let mut m = [0.0; 16];
    for c in 0..4 {
        for r in 0..4 {
            m[c * 4 + r] = (0..4).map(|k| a[k * 4 + r] * b[c * 4 + k]).sum();
        }
    }
    m
}

fn translate(t: [f64; 3]) -> Mat {
    let mut m = IDENTITY;
    m[12] = t[0];
    m[13] = t[1];
    m[14] = t[2];
    m
}

fn scale(s: [f64; 3]) -> Mat {
    let mut m = IDENTITY;
    m[0] = s[0];
    m[5] = s[1];
    m[10] = s[2];
    m
}

/// The rotation about one axis (0 = x, 1 = y, 2 = z) by `deg` degrees, right-handed
fn axis_rotation(axis: usize, deg: f64) -> Mat {
    let (sin, cos) = deg.to_radians().sin_cos();
    // the two axes the rotation moves: `from` turns towards `to`
    let (from, to) = match axis {
        0 => (1, 2),
        1 => (2, 0),
        _ => (0, 1),
    };
    let mut out = IDENTITY;
    out[from * 4 + from] = cos;
    out[from * 4 + to] = sin;
    out[to * 4 + from] = -sin;
    out[to * 4 + to] = cos;
    out
}

/// The rotation of Euler angles (degrees) applied in `order`, the first axis first
fn euler(deg: [f64; 3], order: RotationOrder) -> Mat {
    let seq: [usize; 3] = match order {
        RotationOrder::Xyz => [0, 1, 2],
        RotationOrder::Xzy => [0, 2, 1],
        RotationOrder::Yzx => [1, 2, 0],
        RotationOrder::Yxz => [1, 0, 2],
        RotationOrder::Zxy => [2, 0, 1],
        RotationOrder::Zyx => [2, 1, 0],
    };
    seq.iter()
        .fold(IDENTITY, |m, &a| mul(&axis_rotation(a, deg[a]), &m))
}

fn neg(v: [f64; 3]) -> [f64; 3] {
    [-v[0], -v[1], -v[2]]
}

impl Model {
    /// The local transform (column-major), FBX's
    /// `T * Roff * Rp * Rpre * R * Rpost^-1 * Rp^-1 * Soff * Sp * S * Sp^-1`
    #[must_use]
    pub fn local_matrix(&self) -> [f64; 16] {
        let post_inv = {
            // the inverse of a rotation in XYZ order is its transpose
            let r = euler(self.post_rotation, RotationOrder::Xyz);
            let mut t = [0.0; 16];
            for c in 0..4 {
                for k in 0..4 {
                    t[c * 4 + k] = r[k * 4 + c];
                }
            }
            t
        };
        [
            translate(self.translation),
            translate(self.rotation_offset),
            translate(self.rotation_pivot),
            euler(self.pre_rotation, RotationOrder::Xyz),
            euler(self.rotation, self.rotation_order),
            post_inv,
            translate(neg(self.rotation_pivot)),
            translate(self.scaling_offset),
            translate(self.scaling_pivot),
            scale(self.scaling),
            translate(neg(self.scaling_pivot)),
        ]
        .iter()
        .fold(IDENTITY, |m, x| mul(&m, x))
    }
}

impl Scene {
    /// The world transform of a model: its parents' local transforms times its own
    ///
    /// # Panics
    ///
    /// When `model` is not an index into [`Scene::models`]
    #[must_use]
    pub fn world_matrix(&self, model: usize) -> [f64; 16] {
        let mut m = self.models[model].local_matrix();
        let mut at = self.models[model].parent;
        while let Some(p) = at {
            m = mul(&self.models[p].local_matrix(), &m);
            at = self.models[p].parent;
        }
        m
    }
}

fn bad(what: impl Into<String>) -> FbxError {
    FbxError::BadScene(what.into())
}

fn object_name(n: &Node) -> String {
    match n.props.get(1) {
        Some(Prop::Str(s)) => {
            let end = s
                .windows(2)
                .position(|w| w == b"\x00\x01")
                .unwrap_or(s.len());
            String::from_utf8_lossy(&s[..end]).into_owned()
        }
        _ => String::new(),
    }
}

fn object_id(n: &Node) -> Result<i64, FbxError> {
    match n.props.first() {
        Some(Prop::I64(id)) => Ok(*id),
        _ => Err(bad(format!("{} without an id", n.name))),
    }
}

fn sub_class(n: &Node) -> &[u8] {
    match n.props.get(2) {
        Some(Prop::Str(s)) => s,
        _ => b"",
    }
}

fn str_child<'a>(n: &'a Node, name: &str) -> Option<&'a [u8]> {
    match n.child(name)?.props.first() {
        Some(Prop::Str(s)) => Some(s),
        _ => None,
    }
}

fn f64s<'a>(n: &'a Node, name: &str, what: &str) -> Result<&'a [f64], FbxError> {
    match n.child(name).and_then(|c| c.props.first()) {
        Some(Prop::F64s(v)) => Ok(v),
        _ => Err(bad(format!("{what}: no {name} array of doubles"))),
    }
}

fn i32s<'a>(n: &'a Node, name: &str, what: &str) -> Result<&'a [i32], FbxError> {
    match n.child(name).and_then(|c| c.props.first()) {
        Some(Prop::I32s(v)) => Ok(v),
        _ => Err(bad(format!("{what}: no {name} array of ints"))),
    }
}

fn triples(v: &[f64], what: &str) -> Result<Vec<[f64; 3]>, FbxError> {
    if v.len() % 3 != 0 || !v.iter().all(|x| x.is_finite()) {
        return Err(bad(format!("{what}: not finite triples")));
    }
    Ok(v.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect())
}

fn matrix(n: &Node, name: &str, what: &str) -> Result<Mat, FbxError> {
    if n.child(name).is_none() {
        return Ok(IDENTITY);
    }
    let v = f64s(n, name, what)?;
    if v.len() != 16 || !v.iter().all(|x| x.is_finite()) {
        return Err(bad(format!("{what}: {name} is not 16 finite numbers")));
    }
    let mut m = [0.0; 16];
    m.copy_from_slice(v);
    Ok(m)
}

fn index(i: i32, len: usize, what: &str) -> Result<u32, FbxError> {
    u32::try_from(i)
        .ok()
        .filter(|&u| (u as usize) < len)
        .ok_or_else(|| bad(format!("{what}: index {i} outside 0..{len}")))
}

/// The values of a `Properties70` record by name
fn properties(n: &Node) -> HashMap<String, &[Prop]> {
    let mut out = HashMap::new();
    if let Some(p70) = n.child("Properties70") {
        for p in p70.children_named("P") {
            if let Some(Prop::Str(name)) = p.props.first() {
                out.insert(
                    String::from_utf8_lossy(name).into_owned(),
                    p.props.get(4..).unwrap_or(&[]),
                );
            }
        }
    }
    out
}

fn num(p: &Prop) -> Option<f64> {
    match p {
        Prop::F64(v) => Some(*v),
        Prop::F32(v) => Some(f64::from(*v)),
        Prop::I32(v) => Some(f64::from(*v)),
        #[allow(clippy::cast_precision_loss)]
        Prop::I64(v) => Some(*v as f64),
        Prop::I16(v) => Some(f64::from(*v)),
        _ => None,
    }
}

fn vec3(
    props: &HashMap<String, &[Prop]>,
    key: &str,
    default: [f64; 3],
    what: &str,
) -> Result<[f64; 3], FbxError> {
    let Some(v) = props.get(key) else {
        return Ok(default);
    };
    let n: Vec<f64> = v.iter().filter_map(num).collect();
    if n.len() != 3 || !n.iter().all(|x| x.is_finite()) {
        return Err(bad(format!("{what}: {key} is not 3 finite numbers")));
    }
    Ok([n[0], n[1], n[2]])
}

fn scalar(props: &HashMap<String, &[Prop]>, key: &str) -> Option<f64> {
    props.get(key).and_then(|v| v.first()).and_then(num)
}

/// An enum or int property: an integer type, or a double holding a whole number
fn int(props: &HashMap<String, &[Prop]>, key: &str, what: &str) -> Result<Option<i64>, FbxError> {
    let Some(p) = props.get(key).and_then(|v| v.first()) else {
        return Ok(None);
    };
    let v = match p {
        Prop::I16(v) => Some(i64::from(*v)),
        Prop::I32(v) => Some(i64::from(*v)),
        Prop::I64(v) => Some(*v),
        #[allow(clippy::cast_possible_truncation)]
        Prop::F64(v) if v.fract() == 0.0 && v.abs() < 1e15 => Some(*v as i64),
        _ => None,
    };
    v.map(Some)
        .ok_or_else(|| bad(format!("{what}: {key} is not a whole number")))
}

fn axis(props: &HashMap<String, &[Prop]>, key: &str, default: u8) -> Result<Axis, FbxError> {
    let a = int(props, key, "GlobalSettings")?.unwrap_or(i64::from(default));
    let sign = int(props, &format!("{key}Sign"), "GlobalSettings")?.unwrap_or(1);
    let axis = u8::try_from(a)
        .ok()
        .filter(|&a| a <= 2)
        .ok_or_else(|| bad(format!("GlobalSettings: {key} {a}")))?;
    let sign = match sign {
        1 => 1,
        -1 => -1,
        _ => return Err(bad(format!("GlobalSettings: {key}Sign {sign}"))),
    };
    Ok(Axis { axis, sign })
}

fn model(n: &Node) -> Result<Model, FbxError> {
    let name = object_name(n);
    let what = format!("model {name}");
    let p = properties(n);
    let order = match int(&p, "RotationOrder", &what)?.unwrap_or(0) {
        0 => RotationOrder::Xyz,
        1 => RotationOrder::Xzy,
        2 => RotationOrder::Yzx,
        3 => RotationOrder::Yxz,
        4 => RotationOrder::Zxy,
        5 => RotationOrder::Zyx,
        x => return Err(bad(format!("{what}: rotation order {x} is not read"))),
    };
    Ok(Model {
        id: object_id(n)?,
        name,
        kind: String::from_utf8_lossy(sub_class(n)).into_owned(),
        parent: None,
        translation: vec3(&p, "Lcl Translation", [0.0; 3], &what)?,
        rotation: vec3(&p, "Lcl Rotation", [0.0; 3], &what)?,
        scaling: vec3(&p, "Lcl Scaling", [1.0; 3], &what)?,
        pre_rotation: vec3(&p, "PreRotation", [0.0; 3], &what)?,
        post_rotation: vec3(&p, "PostRotation", [0.0; 3], &what)?,
        rotation_offset: vec3(&p, "RotationOffset", [0.0; 3], &what)?,
        rotation_pivot: vec3(&p, "RotationPivot", [0.0; 3], &what)?,
        scaling_offset: vec3(&p, "ScalingOffset", [0.0; 3], &what)?,
        scaling_pivot: vec3(&p, "ScalingPivot", [0.0; 3], &what)?,
        rotation_order: order,
    })
}

/// A layer's values resolved to `count` slots (corners or polygons)
/// Where a mesh's corners and polygons are: what a layer resolves against
struct Topology<'a> {
    /// The control point of every corner
    corner_vertex: &'a [u32],
    /// The polygon of every corner
    polygon_of_corner: &'a [u32],
    /// The number of polygons
    polygons: usize,
}

fn resolve<T: Copy>(
    values: &[T],
    index: Option<&[i32]>,
    mapping: &[u8],
    topo: &Topology<'_>,
    per_polygon: bool,
    what: &str,
) -> Result<Vec<T>, FbxError> {
    let (corner_vertex, polygon_of_corner, polygons) =
        (topo.corner_vertex, topo.polygon_of_corner, topo.polygons);
    let pick = |k: usize| -> Result<T, FbxError> {
        let i = match index {
            Some(ix) => {
                let raw = *ix
                    .get(k)
                    .ok_or_else(|| bad(format!("{what}: index array too short")))?;
                index_usize(raw, values.len(), what)?
            }
            None => k,
        };
        values
            .get(i)
            .copied()
            .ok_or_else(|| bad(format!("{what}: value {i} outside 0..{}", values.len())))
    };
    let corners = corner_vertex.len();
    match (mapping, per_polygon) {
        (b"AllSame", false) => (0..corners).map(|_| pick(0)).collect(),
        (b"AllSame", true) => (0..polygons).map(|_| pick(0)).collect(),
        (b"ByPolygonVertex", false) => (0..corners).map(pick).collect(),
        (b"ByPolygon", true) => (0..polygons).map(pick).collect(),
        (b"ByPolygon", false) => polygon_of_corner
            .iter()
            .map(|&p| pick(p as usize))
            .collect(),
        (b"ByVertex" | b"ByVertice" | b"ByControlPoint", false) => {
            corner_vertex.iter().map(|&v| pick(v as usize)).collect()
        }
        _ => Err(bad(format!(
            "{what}: mapping {} is not read",
            String::from_utf8_lossy(mapping)
        ))),
    }
}

fn index_usize(i: i32, len: usize, what: &str) -> Result<usize, FbxError> {
    index(i, len, what).map(|u| u as usize)
}

struct Layer<'a> {
    mapping: &'a [u8],
    index: Option<&'a [i32]>,
}

fn layer<'a>(n: &'a Node, index_name: &str, what: &str) -> Result<Layer<'a>, FbxError> {
    let mapping =
        str_child(n, "MappingInformationType").ok_or_else(|| bad(format!("{what}: no mapping")))?;
    let reference = str_child(n, "ReferenceInformationType").unwrap_or(b"Direct");
    let index = match reference {
        b"Direct" => None,
        b"IndexToDirect" | b"Index" => Some(i32s(n, index_name, what)?),
        other => {
            return Err(bad(format!(
                "{what}: reference {} is not read",
                String::from_utf8_lossy(other)
            )))
        }
    };
    Ok(Layer { mapping, index })
}

#[allow(clippy::too_many_lines)]
fn mesh(n: &Node) -> Result<Mesh, FbxError> {
    let name = object_name(n);
    let what = format!("geometry {name}");
    let positions = triples(f64s(n, "Vertices", &what)?, &what)?;
    let pvi = i32s(n, "PolygonVertexIndex", &what)?;
    let mut polygon_vertices = Vec::with_capacity(pvi.len());
    let mut polygon_starts = vec![0u32];
    let mut polygon_of_corner = Vec::with_capacity(pvi.len());
    for &raw in pvi {
        let last = raw < 0;
        let v = if last { !raw } else { raw };
        polygon_vertices.push(index(v, positions.len(), &what)?);
        #[allow(clippy::cast_possible_truncation)]
        polygon_of_corner.push((polygon_starts.len() - 1) as u32);
        if last {
            #[allow(clippy::cast_possible_truncation)]
            let end = polygon_vertices.len() as u32;
            if end - polygon_starts[polygon_starts.len() - 1] < 3 {
                return Err(bad(format!("{what}: a polygon with fewer than 3 corners")));
            }
            polygon_starts.push(end);
        }
    }
    if polygon_starts.last().copied() != u32::try_from(polygon_vertices.len()).ok() {
        return Err(bad(format!("{what}: the last polygon is not closed")));
    }
    let topo = Topology {
        corner_vertex: &polygon_vertices,
        polygon_of_corner: &polygon_of_corner,
        polygons: polygon_starts.len() - 1,
    };
    let mut normals = None;
    if let Some(l) = n.child("LayerElementNormal") {
        let lw = format!("{what} normals");
        let lay = layer(l, "NormalsIndex", &lw)?;
        let values = triples(f64s(l, "Normals", &lw)?, &lw)?;
        normals = Some(resolve(&values, lay.index, lay.mapping, &topo, false, &lw)?);
    }
    let mut uvs = Vec::new();
    for l in n.children_named("LayerElementUV") {
        let lw = format!("{what} uv");
        let lay = layer(l, "UVIndex", &lw)?;
        let v = f64s(l, "UV", &lw)?;
        if v.len() % 2 != 0 || !v.iter().all(|x| x.is_finite()) {
            return Err(bad(format!("{lw}: not finite pairs")));
        }
        let values: Vec<[f64; 2]> = v.chunks_exact(2).map(|c| [c[0], c[1]]).collect();
        uvs.push(UvSet {
            name: str_child(l, "Name")
                .map(|s| String::from_utf8_lossy(s).into_owned())
                .unwrap_or_default(),
            uv: resolve(&values, lay.index, lay.mapping, &topo, false, &lw)?,
        });
    }
    let mut polygon_materials = None;
    if let Some(l) = n.child("LayerElementMaterial") {
        let lw = format!("{what} materials");
        let mapping = str_child(l, "MappingInformationType")
            .ok_or_else(|| bad(format!("{lw}: no mapping")))?;
        let slots = i32s(l, "Materials", &lw)?;
        let slots: Vec<u32> = slots
            .iter()
            .map(|&s| u32::try_from(s).map_err(|_| bad(format!("{lw}: slot {s}"))))
            .collect::<Result<_, _>>()?;
        polygon_materials = Some(resolve(&slots, None, mapping, &topo, true, &lw)?);
    }
    Ok(Mesh {
        id: object_id(n)?,
        name,
        model: None,
        positions,
        polygon_vertices,
        polygon_starts,
        normals,
        uvs,
        polygon_materials,
        materials: Vec::new(),
        skin: None,
        blend_channels: Vec::new(),
    })
}

/// What an object became, by index into the scene's lists
#[derive(Clone, Copy)]
enum Obj {
    Model(usize),
    Mesh(usize),
    Material(usize),
    Skin,
    Cluster(usize),
    BlendShape,
    Channel(usize),
    Shape(usize),
    Other,
}

/// Reads the scene of a binary FBX node tree
///
/// # Errors
///
/// [`FbxError::BadScene`] for an object or connection that does not fit what it names (an
/// index outside its array, arrays of mismatched lengths, an open polygon, an unread mapping
/// or rotation order, a connection to an object that does not exist)
#[allow(clippy::too_many_lines)]
pub fn read_scene(doc: &Document) -> Result<Scene, FbxError> {
    let settings = {
        let p = doc
            .node("GlobalSettings")
            .map(properties)
            .unwrap_or_default();
        let unit_scale = scalar(&p, "UnitScaleFactor").unwrap_or(1.0);
        if !(unit_scale.is_finite() && unit_scale > 0.0) {
            return Err(bad(format!("GlobalSettings: UnitScaleFactor {unit_scale}")));
        }
        Settings {
            up: axis(&p, "UpAxis", 1)?,
            front: axis(&p, "FrontAxis", 2)?,
            coord: axis(&p, "CoordAxis", 0)?,
            unit_scale,
        }
    };
    let empty = Node {
        name: String::new(),
        props: Vec::new(),
        children: Vec::new(),
    };
    let objects = doc.node("Objects").unwrap_or(&empty);

    // every object by id, and what kind of thing it became
    let mut kind: HashMap<i64, Obj> = HashMap::new();
    let mut models = Vec::new();
    let mut meshes = Vec::new();
    let mut materials = Vec::new();
    let mut clusters: Vec<Cluster> = Vec::new();
    let mut channels: Vec<BlendChannel> = Vec::new();
    let mut channel_full: Vec<Vec<f64>> = Vec::new();
    let mut shapes: Vec<BlendTarget> = Vec::new();
    for o in &objects.children {
        let id = object_id(o)?;
        let what = format!("{} {}", o.name, object_name(o));
        let k = match (o.name.as_str(), sub_class(o)) {
            ("Model", _) => {
                models.push(model(o)?);
                Obj::Model(models.len() - 1)
            }
            ("Geometry", b"Mesh") => {
                meshes.push(mesh(o)?);
                Obj::Mesh(meshes.len() - 1)
            }
            ("Geometry", b"Shape") => {
                let indices = i32s(o, "Indexes", &what)?;
                let deltas = triples(f64s(o, "Vertices", &what)?, &what)?;
                if deltas.len() != indices.len() {
                    return Err(bad(format!(
                        "{what}: {} indexes, {} deltas",
                        indices.len(),
                        deltas.len()
                    )));
                }
                let normals = if o.child("Normals").is_some() {
                    let n = triples(f64s(o, "Normals", &what)?, &what)?;
                    if n.len() != indices.len() {
                        return Err(bad(format!(
                            "{what}: {} indexes, {} normals",
                            indices.len(),
                            n.len()
                        )));
                    }
                    Some(n)
                } else {
                    None
                };
                shapes.push(BlendTarget {
                    name: object_name(o),
                    full_weight: 100.0,
                    indices: indices
                        .iter()
                        .map(|&i| u32::try_from(i).map_err(|_| bad(format!("{what}: index {i}"))))
                        .collect::<Result<_, _>>()?,
                    deltas,
                    normals,
                });
                Obj::Shape(shapes.len() - 1)
            }
            ("Material", _) => {
                let p = properties(o);
                let diffuse = match p.get("DiffuseColor") {
                    None => None,
                    Some(_) => Some(vec3(&p, "DiffuseColor", [0.0; 3], &what)?),
                };
                materials.push(Material {
                    id,
                    name: object_name(o),
                    diffuse,
                });
                Obj::Material(materials.len() - 1)
            }
            ("Deformer", b"Skin") => Obj::Skin,
            ("Deformer", b"Cluster") => {
                let (indices, weights) = match (o.child("Indexes"), o.child("Weights")) {
                    (None, None) => (Vec::new(), Vec::new()),
                    _ => (
                        i32s(o, "Indexes", &what)?.to_vec(),
                        f64s(o, "Weights", &what)?.to_vec(),
                    ),
                };
                if indices.len() != weights.len() || !weights.iter().all(|w| w.is_finite()) {
                    return Err(bad(format!(
                        "{what}: {} indexes, {} weights",
                        indices.len(),
                        weights.len()
                    )));
                }
                clusters.push(Cluster {
                    bone: None,
                    indices: indices
                        .iter()
                        .map(|&i| u32::try_from(i).map_err(|_| bad(format!("{what}: index {i}"))))
                        .collect::<Result<_, _>>()?,
                    weights,
                    transform: matrix(o, "Transform", &what)?,
                    transform_link: matrix(o, "TransformLink", &what)?,
                });
                Obj::Cluster(clusters.len() - 1)
            }
            ("Deformer", b"BlendShape") => Obj::BlendShape,
            ("Deformer", b"BlendShapeChannel") => {
                let deform = match o.child("DeformPercent").and_then(|c| c.props.first()) {
                    None => 0.0,
                    Some(p) => num(p)
                        .filter(|x| x.is_finite())
                        .ok_or_else(|| bad(format!("{what}: DeformPercent")))?,
                };
                let full = match o.child("FullWeights") {
                    None => Vec::new(),
                    Some(_) => f64s(o, "FullWeights", &what)?.to_vec(),
                };
                channels.push(BlendChannel {
                    name: object_name(o),
                    deform_percent: deform,
                    targets: Vec::new(),
                });
                channel_full.push(full);
                Obj::Channel(channels.len() - 1)
            }
            _ => Obj::Other,
        };
        if kind.insert(id, k).is_some() {
            return Err(bad(format!("two objects with id {id}")));
        }
    }

    // connections: object to object, child first
    let mut skin_of_mesh: HashMap<usize, Vec<usize>> = HashMap::new(); // mesh -> skin ids' clusters
    let mut skin_clusters: HashMap<i64, Vec<usize>> = HashMap::new();
    let mut skin_mesh: HashMap<i64, usize> = HashMap::new();
    let mut blend_channels: HashMap<i64, Vec<usize>> = HashMap::new();
    let mut blend_mesh: HashMap<i64, usize> = HashMap::new();
    let mut channel_shapes: Vec<Vec<usize>> = vec![Vec::new(); channels.len()];
    let mut model_materials: Vec<Vec<usize>> = vec![Vec::new(); models.len()];
    let mut mesh_model: Vec<Option<usize>> = vec![None; meshes.len()];
    let connections = doc
        .node("Connections")
        .map_or(&[][..], |c| c.children.as_slice());
    for c in connections.iter().filter(|c| c.name == "C") {
        let (Some(Prop::Str(ty)), Some(Prop::I64(child)), Some(Prop::I64(parent))) =
            (c.props.first(), c.props.get(1), c.props.get(2))
        else {
            return Err(bad("a connection that is not (type, child, parent)"));
        };
        if ty.as_slice() != b"OO" {
            continue;
        }
        if *parent == 0 {
            continue;
        }
        let ck = *kind
            .get(child)
            .ok_or_else(|| bad(format!("a connection from unknown object {child}")))?;
        let pk = *kind
            .get(parent)
            .ok_or_else(|| bad(format!("a connection to unknown object {parent}")))?;
        match (ck, pk) {
            (Obj::Model(m), Obj::Model(p)) => {
                if models[m].parent.replace(p).is_some() {
                    return Err(bad(format!("model {} has two parents", models[m].name)));
                }
            }
            (Obj::Mesh(g), Obj::Model(m)) => mesh_model[g] = Some(m),
            (Obj::Material(x), Obj::Model(m)) => model_materials[m].push(x),
            (Obj::Skin, Obj::Mesh(g)) => {
                skin_mesh.insert(*child, g);
            }
            (Obj::Cluster(k), Obj::Skin) => skin_clusters.entry(*parent).or_default().push(k),
            (Obj::Model(m), Obj::Cluster(k)) => clusters[k].bone = Some(m),
            (Obj::BlendShape, Obj::Mesh(g)) => {
                blend_mesh.insert(*child, g);
            }
            (Obj::Channel(ch), Obj::BlendShape) => {
                blend_channels.entry(*parent).or_default().push(ch);
            }
            (Obj::Shape(s), Obj::Channel(ch)) => channel_shapes[ch].push(s),
            _ => {}
        }
    }
    // a model's parents must not loop
    for start in 0..models.len() {
        let mut at = models[start].parent;
        let mut steps = 0;
        while let Some(p) = at {
            steps += 1;
            if steps > models.len() {
                return Err(bad(format!(
                    "model {} is its own ancestor",
                    models[start].name
                )));
            }
            at = models[p].parent;
        }
    }
    for (skin, g) in &skin_mesh {
        skin_of_mesh
            .entry(*g)
            .or_default()
            .extend(skin_clusters.get(skin).map_or(&[][..], Vec::as_slice));
    }
    for (ch, list) in channel_shapes.iter().enumerate() {
        // FullWeights holds one percentage per target; some exporters write one per vertex of a
        // channel's single target instead, all equal, which is that target's full weight
        let full = &channel_full[ch];
        let per_target: Vec<f64> = if full.is_empty() {
            vec![100.0; list.len()]
        } else if full.len() == list.len() {
            full.clone()
        } else if list.len() == 1
            && full.len() == shapes[list[0]].indices.len()
            && full.windows(2).all(|w| w[0].to_bits() == w[1].to_bits())
        {
            vec![full[0]]
        } else {
            return Err(bad(format!(
                "channel {}: {} targets, {} full weights",
                channels[ch].name,
                list.len(),
                full.len()
            )));
        };
        if !per_target.iter().all(|w| w.is_finite()) {
            return Err(bad(format!(
                "channel {}: a non-finite full weight",
                channels[ch].name
            )));
        }
        for (&s, &w) in list.iter().zip(&per_target) {
            let mut t = shapes[s].clone();
            t.full_weight = w;
            channels[ch].targets.push(t);
        }
    }
    for (g, m) in meshes.iter_mut().enumerate() {
        m.model = mesh_model[g];
        if let Some(model) = m.model {
            m.materials.clone_from(&model_materials[model]);
        }
        if let Some(pm) = &m.polygon_materials {
            if let Some(&slot) = pm.iter().find(|&&s| s as usize >= m.materials.len()) {
                return Err(bad(format!(
                    "geometry {}: material slot {slot} of {}",
                    m.name,
                    m.materials.len()
                )));
            }
        }
        if let Some(list) = skin_of_mesh.get(&g) {
            let mut cl = Vec::new();
            for &k in list {
                let c = clusters[k].clone();
                if let Some(&i) = c.indices.iter().find(|&&i| i as usize >= m.positions.len()) {
                    return Err(bad(format!(
                        "geometry {}: a cluster index {i} of {} points",
                        m.name,
                        m.positions.len()
                    )));
                }
                cl.push(c);
            }
            m.skin = Some(cl);
        }
        for (shape, chans) in &blend_channels {
            if blend_mesh.get(shape) == Some(&g) {
                for &ch in chans {
                    let c = channels[ch].clone();
                    for t in &c.targets {
                        if let Some(&i) =
                            t.indices.iter().find(|&&i| i as usize >= m.positions.len())
                        {
                            return Err(bad(format!(
                                "geometry {}: a shape index {i} of {} points",
                                m.name,
                                m.positions.len()
                            )));
                        }
                    }
                    m.blend_channels.push(c);
                }
            }
        }
    }
    Ok(Scene {
        settings,
        models,
        meshes,
        materials,
    })
}
