//! Local cloud-drive folder detection and sync routing.

use crate::models::{SyncMode, SyncProfile};
use crate::paths;
use crate::syncer::{self, SyncResult};
use std::path::{Path, PathBuf};

/// An installed cloud provider's local folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudProvider {
    pub name: String,
    pub local_path: PathBuf,
    pub is_active: bool,
    pub provider_type: String,
}

fn provider(name: &str, path: PathBuf, kind: &str) -> CloudProvider {
    CloudProvider {
        name: name.into(),
        local_path: path,
        is_active: true,
        provider_type: kind.into(),
    }
}

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

fn first_dir(candidates: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    candidates.into_iter().find(|p| p.is_dir())
}

fn env_path(var: &str) -> Option<PathBuf> {
    std::env::var_os(var)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// OneDrive, Google Drive, Dropbox, iCloud, Nextcloud, Box — in that order.
pub fn detect_all() -> Vec<CloudProvider> {
    [
        detect_onedrive(),
        detect_google_drive(),
        detect_dropbox(),
        detect_icloud(),
        detect_nextcloud(),
        detect_box(),
    ]
    .into_iter()
    .flatten()
    .collect()
}

#[cfg(windows)]
fn onedrive_registry() -> Vec<PathBuf> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    [
        r"Software\Microsoft\OneDrive\Accounts\Personal",
        r"Software\Microsoft\OneDrive",
    ]
    .iter()
    .filter_map(|k| {
        hkcu.open_subkey(k)
            .ok()?
            .get_value::<String, _>("UserFolder")
            .ok()
    })
    .map(PathBuf::from)
    .collect()
}

#[cfg(not(windows))]
fn onedrive_registry() -> Vec<PathBuf> {
    Vec::new()
}

pub fn detect_onedrive() -> Option<CloudProvider> {
    let mut c: Vec<PathBuf> = ["OneDriveConsumer", "OneDrive", "OneDriveCommercial"]
        .iter()
        .filter_map(|v| env_path(v))
        .collect();
    c.extend(onedrive_registry());
    c.push(home().join("OneDrive"));
    first_dir(c).map(|p| provider("OneDrive", p, "Microsoft OneDrive"))
}

/// Python v3.0 also accepted any existing `F:`–`J:` drive root as Google
/// Drive; the Rust build only accepts drives that contain `My Drive`, so a
/// USB stick on `F:` is not mistaken for a cloud folder.
pub fn detect_google_drive() -> Option<CloudProvider> {
    let mut c = Vec::new();
    if cfg!(windows) {
        c.extend(
            ["G", "H", "I", "J", "F"]
                .iter()
                .map(|l| PathBuf::from(format!("{l}:/My Drive"))),
        );
    }
    let h = home();
    c.extend([
        h.join("Google Drive").join("My Drive"),
        h.join("Google Drive"),
        h.join("My Drive"),
    ]);
    first_dir(c).map(|p| provider("Google Drive", p, "Google Drive"))
}

pub fn detect_dropbox() -> Option<CloudProvider> {
    let infos: Vec<PathBuf> = if cfg!(windows) {
        ["LOCALAPPDATA", "APPDATA"]
            .iter()
            .filter_map(|v| env_path(v))
            .map(|p| p.join("Dropbox").join("info.json"))
            .collect()
    } else {
        vec![home().join(".dropbox").join("info.json")]
    };
    let mut c = Vec::new();
    for info in infos {
        let Ok(text) = std::fs::read_to_string(&info) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        for key in ["personal", "business"] {
            if let Some(p) = v
                .get(key)
                .and_then(|o| o.get("path"))
                .and_then(|p| p.as_str())
            {
                c.push(PathBuf::from(p));
            }
        }
    }
    c.push(home().join("Dropbox"));
    first_dir(c).map(|p| provider("Dropbox", p, "Dropbox"))
}

pub fn detect_icloud() -> Option<CloudProvider> {
    let mut c: Vec<PathBuf> = env_path("USERPROFILE")
        .map(|p| p.join("iCloudDrive"))
        .into_iter()
        .collect();
    c.push(
        home()
            .join("Library")
            .join("Mobile Documents")
            .join("com~apple~CloudDocs"),
    );
    first_dir(c).map(|p| provider("iCloud Drive", p, "Apple iCloud"))
}

pub fn detect_nextcloud() -> Option<CloudProvider> {
    first_dir([home().join("Nextcloud"), home().join("ownCloud")])
        .map(|p| provider("Nextcloud", p, "Nextcloud"))
}

pub fn detect_box() -> Option<CloudProvider> {
    first_dir([home().join("Box"), home().join("Box Sync")]).map(|p| provider("Box", p, "Box"))
}

/// Syncs `source_dir` into `<provider>/<subfolder>`.
pub fn sync_to_cloud(
    source_dir: &Path,
    p: &CloudProvider,
    subfolder: &str,
    mode: SyncMode,
    dry_run: bool,
) -> SyncResult {
    let source = paths::resolve(source_dir);
    if !source.exists() {
        let msg = format!("Source directory does not exist: {}", source.display());
        return SyncResult {
            errors: vec![msg.clone()],
            message: msg,
            ..Default::default()
        };
    }
    let target = p.local_path.join(subfolder);
    if let Err(e) = std::fs::create_dir_all(&target) {
        return SyncResult {
            message: e.to_string(),
            errors: vec![e.to_string()],
            ..Default::default()
        };
    }
    let profile = SyncProfile {
        name: format!("Cloud Sync ({})", p.name),
        source_dir: source,
        dest_dir: target,
        mode,
        enabled: true,
        trigger_on_usb: false,
        usb_volume_name: None,
    };
    syncer::sync(&profile, dry_run)
}

/// Files whose names mark a sync conflict or lock.
pub fn find_sync_conflicts(folder: &Path) -> Vec<PathBuf> {
    crate::fsutil::walk_files(folder)
        .into_iter()
        .filter(|p| {
            let n = crate::fsutil::name_of(p).to_lowercase();
            [".sync-conflict", "conflicted copy", ".lock"]
                .iter()
                .any(|t| n.contains(t))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflicts_and_routing() {
        let d = tempfile::tempdir().unwrap();
        let cloud = d.path().join("cloud");
        std::fs::create_dir_all(cloud.join("x")).unwrap();
        std::fs::write(cloud.join("x/report (conflicted copy).docx"), "a").unwrap();
        std::fs::write(cloud.join("a.sync-conflict-2026.txt"), "a").unwrap();
        std::fs::write(cloud.join("fine.txt"), "a").unwrap();
        assert_eq!(find_sync_conflicts(&cloud).len(), 2);

        let src = d.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("f.txt"), "data").unwrap();
        let p = provider("Test", cloud.clone(), "Test");
        let r = sync_to_cloud(&src, &p, "SFO_Cloud_Sync", SyncMode::Incremental, false);
        assert!(r.success, "{r:?}");
        assert!(cloud.join("SFO_Cloud_Sync/f.txt").exists());
        let _ = detect_all();
    }
}
