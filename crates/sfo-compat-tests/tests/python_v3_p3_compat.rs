//! Phase 2/3 compatibility: vaults, cold archives and perceptual hashes
//! produced by the Python v3.0 build (see `scripts/gen_compat_fixtures.py`).

use serde_json::Value;
use sfo_compat_tests::fixtures_dir;
use sfo_core::vault::{self, KdfParams, VaultError};
use std::fs;
use std::path::Path;

fn read_json(rel: &str) -> Value {
    serde_json::from_str(&fs::read_to_string(fixtures_dir().join(rel)).unwrap()).unwrap()
}

const FAST_KDF: KdfParams = KdfParams {
    m_cost_kib: 1024,
    t_cost: 1,
    p_cost: 1,
};

#[test]
fn python_raw_v1_ciphertext_decrypts() {
    let meta = read_json("vault/meta.json");
    let data = fs::read(fixtures_dir().join("vault/raw_v1.bin")).unwrap();
    assert_eq!(vault::version_of(&data).unwrap(), 1);
    let pt = vault::decrypt(&data, meta["raw_password"].as_str().unwrap()).unwrap();
    assert_eq!(hex::encode(pt), meta["raw_plaintext_hex"].as_str().unwrap());
    assert!(matches!(
        vault::decrypt(&data, "wrong"),
        Err(VaultError::InvalidPassword)
    ));
}

#[test]
fn python_folder_vault_unlocks_and_upgrades() {
    let meta = read_json("vault/meta.json");
    let pw = meta["folder_password"].as_str().unwrap();
    let d = tempfile::tempdir().unwrap();
    let v = d.path().join("folder_v1.sfovault");
    fs::copy(fixtures_dir().join("vault/folder_v1.sfovault"), &v).unwrap();

    let info = vault::inspect_vault(&v, pw).unwrap();
    assert_eq!((info.file_count, info.created_version), (2, 1));
    let out = vault::unlock_vault(&v, &d.path().join("out"), pw, None).unwrap();
    assert_eq!(out.len(), 2);
    assert_eq!(
        fs::read_to_string(d.path().join("out/secret.txt")).unwrap(),
        meta["folder_files"]["secret.txt"]
    );
    assert_eq!(
        fs::metadata(d.path().join("out/sub/data.bin"))
            .unwrap()
            .len(),
        meta["folder_files"]["sub/data.bin_len"].as_u64().unwrap()
    );

    assert_eq!(vault::upgrade_vault(&v, pw, FAST_KDF).unwrap(), 1);
    assert_eq!(vault::version_of(&fs::read(&v).unwrap()).unwrap(), 2);
    assert_eq!(vault::inspect_vault(&v, pw).unwrap().file_count, 2);
}

#[test]
fn python_cold_archive_restores() {
    let meta = read_json("archive/meta.json");
    let d = tempfile::tempdir().unwrap();
    let zip = d.path().join("cold.zip");
    fs::copy(fixtures_dir().join("archive/cold.zip"), &zip).unwrap();
    // Original paths point at a deleted temp dir; restore into a target folder.
    let r = sfo_core::archiver::restore_archive(&zip, Some(&d.path().join("restored")));
    assert!(r.success, "{r:?}");
    assert_eq!(r.restored_count, 2);
    for (name, content) in meta["members"].as_object().unwrap() {
        let p = sfo_core::paths::join_rel(&d.path().join("restored"), name);
        assert_eq!(fs::read_to_string(p).unwrap(), content.as_str().unwrap());
    }
}

#[test]
fn dhash_matches_pillow() {
    let exp = read_json("visual/expected.json");
    for (name, want) in exp["dhash"].as_object().unwrap() {
        let got = sfo_core::visual_hasher::compute_dhash(&fixtures_dir().join("visual").join(name))
            .unwrap();
        let want = u64::from_str_radix(want.as_str().unwrap(), 16).unwrap();
        let dist = sfo_core::visual_hasher::hamming_distance(got, want);
        assert!(
            dist <= 2,
            "{name}: rust {got:016x} vs pillow {want:016x} ({dist} bits)"
        );
    }
}

#[test]
fn fixtures_exist() {
    for f in [
        "vault/folder_v1.sfovault",
        "vault/raw_v1.bin",
        "archive/cold.zip",
        "visual/pattern.png",
    ] {
        assert!(Path::new(&fixtures_dir().join(f)).is_file(), "{f}");
    }
}
