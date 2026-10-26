//! Bytes behind a valid header, read as a tree and then as a scene: every index and connection
//! the scene follows comes from the input, so a bad one must be an error, never a panic
#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let mut file = b"Kaydara FBX Binary  \x00\x1a\x00".to_vec();
    file.extend_from_slice(&7400u32.to_le_bytes());
    file.extend_from_slice(data);
    if let Ok(doc) = alice_fbx::read_tree(&file) {
        if let Ok(scene) = alice_fbx::read_scene(&doc) {
            for (i, _) in scene.models.iter().enumerate() {
                let _ = scene.world_matrix(i);
            }
            for m in &scene.meshes {
                let _ = m.triangles();
            }
        }
    }
});
