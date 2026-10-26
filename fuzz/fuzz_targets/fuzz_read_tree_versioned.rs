//! Bytes behind a valid header: the first input byte picks 32- or 64-bit record words, so the
//! fuzzer spends its time in the records, properties and arrays rather than in the magic
#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let Some((&pick, rest)) = data.split_first() else {
        return;
    };
    let version: u32 = if pick & 1 == 0 { 7400 } else { 7500 };
    let mut file = b"Kaydara FBX Binary  \x00\x1a\x00".to_vec();
    file.extend_from_slice(&version.to_le_bytes());
    file.extend_from_slice(rest);
    let _ = alice_fbx::read_tree(&file);
});
