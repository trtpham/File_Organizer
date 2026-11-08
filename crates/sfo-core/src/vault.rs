//! Encrypted `.sfovault` containers (a ZIP of the source, encrypted).
//!
//! **v2** (written by the Rust build): XChaCha20-Poly1305, key from
//! Argon2id with parameters stored in the header, whole header bound as AAD.
//!
//! ```text
//! "SFOVAULT" | u16 version=2 | u8 kdf=1 (argon2id) | u32 m_cost KiB | u32 t_cost
//! | u8 p_cost | salt[16] | nonce[24] | ciphertext || tag[16]
//! ```
//!
//! **v1** (Python v3.0, read-only): `"SFOVAULT" | u16 1 | salt[32] | nonce[12]
//! | tag[16] | ciphertext`, PBKDF2-HMAC-SHA256 × 100 000. Python wrote one of
//! two schemes under the same version: AES-256-GCM (with pycryptodome) or a
//! SHA-256 counter keystream + truncated HMAC (stdlib fallback). Both are
//! tried. Use [`upgrade_vault`] to re-encrypt v1 files as v2.

use crate::archiver;
use crate::fsutil;
use crate::guardrails::{self, AuditLogChain};
use crate::paths;
use crate::shredder::{self, ShredMethod};
use aes_gcm::Aes256Gcm;
use aes_gcm::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::XChaCha20Poly1305;
use hmac::{Hmac, Mac};
use serde_json::{Map, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

pub const MAGIC: &[u8; 8] = b"SFOVAULT";
pub const EXTENSION: &str = "sfovault";

const V1: u16 = 1;
const V1_SALT: usize = 32;
const V1_NONCE: usize = 12;
const V1_TAG: usize = 16;
const V1_PBKDF2_ROUNDS: u32 = 100_000;

const V2: u16 = 2;
const V2_KDF_ARGON2ID: u8 = 1;
const V2_SALT: usize = 16;
const V2_NONCE: usize = 24;
const V2_HEADER: usize = 8 + 2 + 1 + 4 + 4 + 1 + V2_SALT + V2_NONCE;

/// Argon2id cost parameters for new vaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KdfParams {
    pub m_cost_kib: u32,
    pub t_cost: u32,
    pub p_cost: u8,
}

impl Default for KdfParams {
    /// OWASP 2023 baseline: 19 MiB, 2 iterations, 1 lane.
    fn default() -> Self {
        Self {
            m_cost_kib: 19_456,
            t_cost: 2,
            p_cost: 1,
        }
    }
}

/// Vault failures.
#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("Incorrect password or tampered vault data.")]
    InvalidPassword,
    #[error("{0}")]
    Corrupted(String),
    #[error("Action blocked by Enterprise Guardrails: {0}")]
    Protected(String),
    #[error("not found: {}", .0.display())]
    NotFound(PathBuf),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Core(#[from] crate::SfoError),
}

type VResult<T> = std::result::Result<T, VaultError>;

fn corrupted(msg: &str) -> VaultError {
    VaultError::Corrupted(msg.into())
}

/// Encrypts `plaintext` into a v2 container.
pub fn encrypt(plaintext: &[u8], password: &str, kdf: KdfParams) -> VResult<Vec<u8>> {
    let salt = fsutil::random_bytes(V2_SALT);
    let nonce = fsutil::random_bytes(V2_NONCE);
    let mut header = Vec::with_capacity(V2_HEADER);
    header.extend_from_slice(MAGIC);
    header.extend_from_slice(&V2.to_be_bytes());
    header.push(V2_KDF_ARGON2ID);
    header.extend_from_slice(&kdf.m_cost_kib.to_be_bytes());
    header.extend_from_slice(&kdf.t_cost.to_be_bytes());
    header.push(kdf.p_cost);
    header.extend_from_slice(&salt);
    header.extend_from_slice(&nonce);

    let key = argon2_key(password, &salt, kdf)?;
    let cipher =
        XChaCha20Poly1305::new_from_slice(&key[..]).map_err(|_| corrupted("bad key length"))?;
    let ct = cipher
        .encrypt(
            nonce.as_slice().into(),
            Payload {
                msg: plaintext,
                aad: &header,
            },
        )
        .map_err(|_| corrupted("encryption failed"))?;
    header.extend_from_slice(&ct);
    Ok(header)
}

fn argon2_key(password: &str, salt: &[u8], kdf: KdfParams) -> VResult<Zeroizing<[u8; 32]>> {
    let params = argon2::Params::new(kdf.m_cost_kib, kdf.t_cost, u32::from(kdf.p_cost), Some(32))
        .map_err(|e| VaultError::Corrupted(format!("invalid KDF parameters: {e}")))?;
    let a2 = argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    let mut key = Zeroizing::new([0u8; 32]);
    a2.hash_password_into(password.as_bytes(), salt, &mut key[..])
        .map_err(|e| VaultError::Corrupted(format!("key derivation failed: {e}")))?;
    Ok(key)
}

/// Container format version (1 or 2).
pub fn version_of(data: &[u8]) -> VResult<u16> {
    if data.len() < 10 {
        return Err(corrupted("Vault file is truncated or invalid."));
    }
    if &data[..8] != MAGIC {
        return Err(corrupted(
            "Invalid vault header: not an SFO Vault container.",
        ));
    }
    Ok(u16::from_be_bytes([data[8], data[9]]))
}

/// Authenticates and decrypts a v1 or v2 container.
pub fn decrypt(data: &[u8], password: &str) -> VResult<Vec<u8>> {
    match version_of(data)? {
        V1 => decrypt_v1(data, password),
        V2 => decrypt_v2(data, password),
        v => Err(VaultError::Corrupted(format!(
            "Unsupported vault version {v}."
        ))),
    }
}

fn decrypt_v2(data: &[u8], password: &str) -> VResult<Vec<u8>> {
    if data.len() < V2_HEADER + 16 {
        return Err(corrupted("Vault file is truncated or invalid."));
    }
    if data[10] != V2_KDF_ARGON2ID {
        return Err(corrupted("Unsupported key derivation function."));
    }
    let u32_at = |i: usize| u32::from_be_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
    let kdf = KdfParams {
        m_cost_kib: u32_at(11),
        t_cost: u32_at(15),
        p_cost: data[19],
    };
    // Refuse absurd parameters from a hostile file (> 4 GiB or > 64 passes).
    if kdf.m_cost_kib > 4 * 1024 * 1024 || kdf.t_cost > 64 {
        return Err(corrupted("Vault KDF parameters are out of range."));
    }
    let salt = &data[20..20 + V2_SALT];
    let nonce = &data[20 + V2_SALT..V2_HEADER];
    let key = argon2_key(password, salt, kdf)?;
    let cipher =
        XChaCha20Poly1305::new_from_slice(&key[..]).map_err(|_| corrupted("bad key length"))?;
    cipher
        .decrypt(
            nonce.into(),
            Payload {
                msg: &data[V2_HEADER..],
                aad: &data[..V2_HEADER],
            },
        )
        .map_err(|_| VaultError::InvalidPassword)
}

fn decrypt_v1(data: &[u8], password: &str) -> VResult<Vec<u8>> {
    let header = 8 + 2 + V1_SALT + V1_NONCE;
    if data.len() < header + V1_TAG {
        return Err(corrupted("Vault file is truncated or invalid."));
    }
    let salt = &data[10..10 + V1_SALT];
    let nonce = &data[10 + V1_SALT..header];
    let tag = &data[header..header + V1_TAG];
    let ct = &data[header + V1_TAG..];
    let aad = &data[..header];

    // PBKDF2 with dklen 64; its first 32 bytes equal the dklen-32 key.
    let mut keys = Zeroizing::new([0u8; 64]);
    pbkdf2::pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, V1_PBKDF2_ROUNDS, &mut keys[..]);

    // Variant A: AES-256-GCM (pycryptodome).
    if let Ok(cipher) = Aes256Gcm::new_from_slice(&keys[..32]) {
        let mut ct_tag = ct.to_vec();
        ct_tag.extend_from_slice(tag);
        if let Ok(pt) = cipher.decrypt(nonce.into(), Payload { msg: &ct_tag, aad }) {
            return Ok(pt);
        }
    }
    // Variant B: SHA-256 counter keystream + HMAC-SHA256[..16] (stdlib).
    let (enc_key, mac_key) = keys.split_at(32);
    let mut mac =
        <Hmac<Sha256> as Mac>::new_from_slice(mac_key).map_err(|_| corrupted("bad MAC key"))?;
    mac.update(aad);
    mac.update(ct);
    if mac.verify_truncated_left(tag).is_err() {
        return Err(VaultError::InvalidPassword);
    }
    Ok(xor_stream_v1(ct, enc_key, nonce))
}

/// Python v1 keystream: block *i* = SHA-256(key ‖ nonce ‖ be64(i)).
fn xor_stream_v1(data: &[u8], key: &[u8], nonce: &[u8]) -> Vec<u8> {
    data.chunks(32)
        .enumerate()
        .flat_map(|(i, chunk)| {
            let ks = Sha256::new()
                .chain_update(key)
                .chain_update(nonce)
                .chain_update((i as u64).to_be_bytes())
                .finalize();
            chunk
                .iter()
                .zip(ks)
                .map(|(b, k)| b ^ k)
                .collect::<Vec<u8>>()
        })
        .collect()
}

/// Test hook: builds a v1 container exactly as Python v3.0 would.
#[doc(hidden)]
pub fn encrypt_v1_for_tests(plaintext: &[u8], password: &str, aes_gcm: bool) -> Vec<u8> {
    let salt = fsutil::random_bytes(V1_SALT);
    let nonce = fsutil::random_bytes(V1_NONCE);
    let mut header = MAGIC.to_vec();
    header.extend_from_slice(&V1.to_be_bytes());
    header.extend_from_slice(&salt);
    header.extend_from_slice(&nonce);
    let mut keys = [0u8; 64];
    pbkdf2::pbkdf2_hmac::<Sha256>(password.as_bytes(), &salt, V1_PBKDF2_ROUNDS, &mut keys);
    let (tag, ct) = if aes_gcm {
        let c = Aes256Gcm::new_from_slice(&keys[..32]).expect("32-byte key");
        let mut out = c
            .encrypt(
                nonce.as_slice().into(),
                Payload {
                    msg: plaintext,
                    aad: &header,
                },
            )
            .expect("encrypt");
        let tag = out.split_off(out.len() - 16);
        (tag, out)
    } else {
        let ct = xor_stream_v1(plaintext, &keys[..32], &nonce);
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(&keys[32..]).expect("key");
        mac.update(&header);
        mac.update(&ct);
        (mac.finalize().into_bytes()[..16].to_vec(), ct)
    };
    header.extend_from_slice(&tag);
    header.extend_from_slice(&ct);
    header
}

/// Contents listing from [`inspect_vault`].
#[derive(Debug, Clone, PartialEq)]
pub struct VaultMetadata {
    pub vault_path: PathBuf,
    pub file_count: usize,
    pub total_uncompressed_bytes: u64,
    pub created_version: u16,
    /// `(name, size, "YYYY-MM-DD")`.
    pub files: Vec<(String, u64, String)>,
}

fn zip_source(src: &Path) -> VResult<Vec<u8>> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    if src.is_file() {
        archiver::add_file(&mut zip, src, &fsutil::name_of(src))?;
    } else {
        for f in fsutil::walk_files(src) {
            let rel = f.strip_prefix(src).unwrap_or(&f);
            let name = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            archiver::add_file(&mut zip, &f, &name)?;
        }
    }
    Ok(zip.finish().map_err(archiver::zip_err)?.into_inner())
}

fn audit_log(chain: Option<&AuditLogChain>, action: &str, details: Map<String, serde_json::Value>) {
    if let Some(c) = chain {
        let _ = c.log_event(action, details, None);
    }
}

/// Zips and encrypts `source` into a v2 vault (adds `.sfovault` if missing).
/// With `shred_source`, the source is DoD-shredded after the vault is written.
pub fn lock_path(
    source: &Path,
    vault_path: &Path,
    password: &str,
    shred_source: bool,
    kdf: KdfParams,
    audit: Option<&AuditLogChain>,
) -> VResult<PathBuf> {
    let src = paths::resolve(source);
    let mut dst = paths::resolve(vault_path);
    if !src.exists() {
        return Err(VaultError::NotFound(src));
    }
    if let Some(reason) = guardrails::protected_reason(&src) {
        return Err(VaultError::Protected(reason));
    }
    if !fsutil::name_of(&dst).ends_with(".sfovault") {
        dst.set_extension(EXTENSION);
    }
    if let Some(d) = dst.parent() {
        fs::create_dir_all(d)?;
    }
    let payload = Zeroizing::new(zip_source(&src)?);
    let sealed = encrypt(&payload, password, kdf)?;
    let tmp = dst.with_file_name(format!(".{}.tmp", fsutil::name_of(&dst)));
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&sealed)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, &dst)?;

    if shred_source {
        let res = if src.is_file() {
            shredder::shred_file(&src, ShredMethod::Dod3Pass, audit, None).map(|_| ())
        } else {
            shredder::shred_directory(&src, ShredMethod::Dod3Pass, true, audit, None).map(|_| ())
        };
        res.map_err(|e| {
            VaultError::Corrupted(format!(
                "vault written, but shredding the source failed: {e}"
            ))
        })?;
    }
    let mut d = Map::new();
    d.insert("source".into(), json!(src.display().to_string()));
    d.insert("vault".into(), json!(dst.display().to_string()));
    d.insert("shred_source".into(), json!(shred_source));
    d.insert(
        "vault_size_bytes".into(),
        json!(fs::metadata(&dst).map(|m| m.len()).unwrap_or(0)),
    );
    audit_log(audit, "VAULT_LOCK", d);
    Ok(dst)
}

fn open_payload(vault_path: &Path, password: &str) -> VResult<(PathBuf, u16, Zeroizing<Vec<u8>>)> {
    let file = paths::resolve(vault_path);
    if !file.exists() {
        return Err(VaultError::NotFound(file));
    }
    let data = fs::read(&file)?;
    let version = version_of(&data)?;
    let pt = Zeroizing::new(decrypt(&data, password)?);
    Ok((file, version, pt))
}

/// Decrypts and extracts into `dest_dir`. Members that would escape
/// `dest_dir` are skipped.
pub fn unlock_vault(
    vault_path: &Path,
    dest_dir: &Path,
    password: &str,
    audit: Option<&AuditLogChain>,
) -> VResult<Vec<PathBuf>> {
    let (file, _, pt) = open_payload(vault_path, password)?;
    let target = paths::resolve(dest_dir);
    fs::create_dir_all(&target)?;
    let mut z = zip::ZipArchive::new(Cursor::new(&pt[..]))
        .map_err(|e| VaultError::Corrupted(e.to_string()))?;
    let mut out = Vec::new();
    for i in 0..z.len() {
        let mut m = z
            .by_index(i)
            .map_err(|e| VaultError::Corrupted(e.to_string()))?;
        let Some(rel) = m.enclosed_name() else {
            continue;
        };
        let dest = target.join(rel);
        if m.is_dir() {
            fs::create_dir_all(&dest)?;
            continue;
        }
        if let Some(d) = dest.parent() {
            fs::create_dir_all(d)?;
        }
        let mut buf = Vec::new();
        m.read_to_end(&mut buf)
            .map_err(|e| VaultError::Corrupted(e.to_string()))?;
        fs::write(&dest, buf)?;
        out.push(dest);
    }
    let mut d = Map::new();
    d.insert("vault".into(), json!(file.display().to_string()));
    d.insert("dest_dir".into(), json!(target.display().to_string()));
    d.insert("extracted_count".into(), json!(out.len()));
    audit_log(audit, "VAULT_UNLOCK", d);
    Ok(out)
}

/// Lists contents without writing anything to disk.
pub fn inspect_vault(vault_path: &Path, password: &str) -> VResult<VaultMetadata> {
    let (file, version, pt) = open_payload(vault_path, password)?;
    let mut z = zip::ZipArchive::new(Cursor::new(&pt[..]))
        .map_err(|e| VaultError::Corrupted(e.to_string()))?;
    let mut files = Vec::new();
    for i in 0..z.len() {
        let m = z
            .by_index(i)
            .map_err(|e| VaultError::Corrupted(e.to_string()))?;
        if m.is_dir() {
            continue;
        }
        let dt = m
            .last_modified()
            .map(|d| format!("{}-{:02}-{:02}", d.year(), d.month(), d.day()))
            .unwrap_or_default();
        files.push((m.name().to_string(), m.size(), dt));
    }
    Ok(VaultMetadata {
        vault_path: file,
        file_count: files.len(),
        total_uncompressed_bytes: files.iter().map(|f| f.1).sum(),
        created_version: version,
        files,
    })
}

/// `true` when `password` opens the vault.
pub fn verify_password(vault_path: &Path, password: &str) -> bool {
    inspect_vault(vault_path, password).is_ok()
}

/// Re-encrypts a vault (typically v1) as v2 in place. Returns the old version.
pub fn upgrade_vault(vault_path: &Path, password: &str, kdf: KdfParams) -> VResult<u16> {
    let (file, version, pt) = open_payload(vault_path, password)?;
    let sealed = encrypt(&pt, password, kdf)?;
    let tmp = file.with_file_name(format!(".{}.upgrade.tmp", fsutil::name_of(&file)));
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&sealed)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, &file)?;
    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fast parameters so debug-build tests stay quick.
    pub(crate) const TEST_KDF: KdfParams = KdfParams {
        m_cost_kib: 1024,
        t_cost: 1,
        p_cost: 1,
    };

    #[test]
    fn v2_round_trip_and_tamper() {
        let ct = encrypt(b"hello vault", "pw", TEST_KDF).unwrap();
        assert_eq!(version_of(&ct).unwrap(), 2);
        assert_eq!(decrypt(&ct, "pw").unwrap(), b"hello vault");
        assert!(matches!(
            decrypt(&ct, "wrong"),
            Err(VaultError::InvalidPassword)
        ));
        let mut bad = ct.clone();
        bad[12] ^= 1; // KDF parameter byte is authenticated.
        assert!(decrypt(&bad, "pw").is_err());
        let mut bad = ct;
        *bad.last_mut().unwrap() ^= 1;
        assert!(matches!(
            decrypt(&bad, "pw"),
            Err(VaultError::InvalidPassword)
        ));
        assert!(matches!(
            decrypt(b"NOTAVAULT...", "pw"),
            Err(VaultError::Corrupted(_))
        ));
    }

    #[test]
    fn v1_both_variants_decode() {
        for gcm in [true, false] {
            let v1 = encrypt_v1_for_tests(b"legacy data", "old pw", gcm);
            assert_eq!(decrypt(&v1, "old pw").unwrap(), b"legacy data", "gcm={gcm}");
            assert!(matches!(
                decrypt(&v1, "nope"),
                Err(VaultError::InvalidPassword)
            ));
        }
    }

    #[test]
    fn lock_inspect_unlock_upgrade() {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("docs");
        fs::create_dir_all(src.join("sub")).unwrap();
        fs::write(src.join("a.txt"), "alpha").unwrap();
        fs::write(src.join("sub/b.txt"), "beta").unwrap();
        let chain = AuditLogChain::new(Some(&d.path().join("audit.jsonl"))).unwrap();

        let vault = lock_path(
            &src,
            &d.path().join("box"),
            "s3cret",
            true,
            TEST_KDF,
            Some(&chain),
        )
        .unwrap();
        assert!(fsutil::name_of(&vault).ends_with(".sfovault"));
        assert!(!src.exists(), "source shredded");

        let meta = inspect_vault(&vault, "s3cret").unwrap();
        assert_eq!(
            (
                meta.file_count,
                meta.total_uncompressed_bytes,
                meta.created_version
            ),
            (2, 9, 2)
        );
        assert!(!verify_password(&vault, "nope"));

        let out = unlock_vault(&vault, &d.path().join("out"), "s3cret", Some(&chain)).unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(
            fs::read_to_string(d.path().join("out/sub/b.txt")).unwrap(),
            "beta"
        );
        assert!(chain.verify_integrity().valid);

        let legacy = d.path().join("legacy.sfovault");
        let payload = zip_source(&d.path().join("out")).unwrap();
        fs::write(&legacy, encrypt_v1_for_tests(&payload, "pw", false)).unwrap();
        assert_eq!(upgrade_vault(&legacy, "pw", TEST_KDF).unwrap(), 1);
        assert_eq!(version_of(&fs::read(&legacy).unwrap()).unwrap(), 2);
        assert_eq!(inspect_vault(&legacy, "pw").unwrap().file_count, 2);
    }
}
