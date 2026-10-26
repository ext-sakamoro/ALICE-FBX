//! Any bytes: the reader returns a tree or an error, never panics or hangs
#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let _ = alice_fbx::read_tree(data);
});
