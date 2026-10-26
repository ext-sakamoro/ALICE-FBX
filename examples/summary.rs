//! Prints what a binary FBX file holds: its axes and unit, the models by kind, and for every mesh
//! its points, polygons, triangles, layers, materials, skin clusters and blend shape channels.
//!
//! ```sh
//! cargo run --example summary -- model.fbx
//! ```

use std::collections::BTreeMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("usage: summary <file.fbx>")?;
    let doc = alice_fbx::read_tree(&std::fs::read(&path)?)?;
    let scene = alice_fbx::read_scene(&doc)?;
    let s = scene.settings;
    println!(
        "FBX {}: up {:?}, front {:?}, coord {:?}, {} cm per unit",
        doc.version, s.up, s.front, s.coord, s.unit_scale
    );
    let mut kinds: BTreeMap<&str, usize> = BTreeMap::new();
    for m in &scene.models {
        *kinds.entry(m.kind.as_str()).or_default() += 1;
    }
    println!("models: {kinds:?}");
    println!("materials: {}", scene.materials.len());
    for m in &scene.meshes {
        let targets: usize = m.blend_channels.iter().map(|c| c.targets.len()).sum();
        println!(
            "mesh {:?}: {} points, {} polygons, {} triangles, normals {}, {} uv sets, {} material slots, {} skin clusters, {} blend channels ({} targets)",
            m.name,
            m.positions.len(),
            m.polygon_count(),
            m.triangles().len(),
            m.normals.is_some(),
            m.uvs.len(),
            m.materials.len(),
            m.skin.as_ref().map_or(0, Vec::len),
            m.blend_channels.len(),
            targets,
        );
    }
    Ok(())
}
