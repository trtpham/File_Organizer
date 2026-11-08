//! Configuration loader: embedded defaults, then user config, or a custom file.
//!
//! The config is kept as an ordered JSON tree (like the Python dict) so key
//! order — which decides category precedence, e.g. `.ts` → `Code` — matches
//! the Python v3.0 build exactly.

use crate::error::{Result, SfoError};
use crate::fnmatch::Pattern;
use crate::models::CollisionStrategy;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// `config/default_config.json`, compiled into the binary.
pub const DEFAULT_CONFIG_JSON: &str = include_str!("../../../config/default_config.json");

/// Loaded configuration plus precomputed lookups.
#[derive(Debug, Clone)]
pub struct Config {
    data: Map<String, Value>,
    extension_map: HashMap<String, String>,
    ignore: Vec<IgnoreRule>,
}

#[derive(Debug, Clone)]
struct IgnoreRule {
    name: Pattern,
    inside: Pattern,
    trailing: Pattern,
}

impl Config {
    /// Embedded defaults only (no user config). Deterministic; used by tests.
    pub fn defaults() -> Self {
        let data = serde_json::from_str(DEFAULT_CONFIG_JSON)
            .expect("embedded default_config.json is valid");
        Self::from_map(data)
    }

    /// Python `ConfigManager(custom_path)`: defaults, then the user config
    /// (only when no custom path is given; parse errors ignored), then the
    /// custom file (errors propagate).
    pub fn load(custom_path: Option<&Path>) -> Result<Self> {
        let mut data: Map<String, Value> = serde_json::from_str(DEFAULT_CONFIG_JSON)
            .expect("embedded default_config.json is valid");

        match custom_path {
            None => {
                if let Ok(text) = std::fs::read_to_string(Self::user_config_path())
                    && let Ok(Value::Object(user)) = serde_json::from_str::<Value>(&text)
                {
                    deep_merge(&mut data, user);
                }
            }
            Some(path) if path.exists() => {
                let text = std::fs::read_to_string(path)?;
                match serde_json::from_str::<Value>(&text)? {
                    Value::Object(custom) => deep_merge(&mut data, custom),
                    _ => {
                        return Err(SfoError::Config(format!(
                            "{} is not a JSON object",
                            path.display()
                        )));
                    }
                }
            }
            Some(_) => {}
        }
        Ok(Self::from_map(data))
    }

    /// Builds lookups from an already-merged tree.
    pub fn from_map(data: Map<String, Value>) -> Self {
        let mut cfg = Self {
            data,
            extension_map: HashMap::new(),
            ignore: Vec::new(),
        };
        cfg.rebuild_indexes();
        cfg
    }

    fn rebuild_indexes(&mut self) {
        self.extension_map.clear();
        for (category, exts) in self.categories() {
            for ext in exts {
                self.extension_map
                    .insert(ext.to_lowercase(), category.clone());
            }
        }
        self.ignore = self
            .ignore_patterns()
            .iter()
            .map(|p| IgnoreRule {
                name: Pattern::new(p),
                inside: Pattern::new(&format!("*/{p}/*")),
                trailing: Pattern::new(&format!("*/{p}")),
            })
            .collect();
    }

    /// `~/.smart_file_organizer/config.json`.
    pub fn user_config_path() -> PathBuf {
        crate::default_data_dir().join("config.json")
    }

    /// Raw config tree.
    pub fn data(&self) -> &Map<String, Value> {
        &self.data
    }

    /// Sets a top-level key and refreshes lookups.
    pub fn set(&mut self, key: &str, value: Value) {
        self.data.insert(key.to_string(), value);
        self.rebuild_indexes();
    }

    /// Categories in file order.
    pub fn categories(&self) -> Vec<(String, Vec<String>)> {
        self.data
            .get("categories")
            .and_then(Value::as_object)
            .map(|m| m.iter().map(|(k, v)| (k.clone(), string_list(v))).collect())
            .unwrap_or_default()
    }

    pub fn ignore_patterns(&self) -> Vec<String> {
        self.data
            .get("ignore_patterns")
            .map(string_list)
            .unwrap_or_default()
    }

    pub fn date_format(&self) -> String {
        self.str_or("date_format", "YYYY/MM")
    }

    pub fn date_attribute(&self) -> String {
        self.str_or("date_attribute", "modified")
    }

    /// Unknown values fall back to `auto_rename`, like Python.
    pub fn collision_strategy(&self) -> CollisionStrategy {
        self.data
            .get("collision_strategy")
            .and_then(Value::as_str)
            .and_then(CollisionStrategy::parse)
            .unwrap_or_default()
    }

    pub fn clean_empty_folders(&self) -> bool {
        self.data
            .get("clean_empty_folders")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    pub fn deduplication_settings(&self) -> Map<String, Value> {
        self.data
            .get("deduplication")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default()
    }

    /// `(debounce_seconds, check_interval_seconds)`.
    pub fn watcher_settings(&self) -> (f64, f64) {
        let w = self.data.get("watcher");
        let get = |k: &str, d: f64| {
            w.and_then(|w| w.get(k))
                .and_then(Value::as_f64)
                .unwrap_or(d)
        };
        (
            get("debounce_seconds", 3.0),
            get("check_interval_seconds", 2.0),
        )
    }

    /// `(pattern, folder)` pairs in priority order.
    pub fn custom_regex_rules(&self) -> Vec<(String, String)> {
        self.data
            .get("custom_regex_rules")
            .and_then(Value::as_array)
            .map(|rules| {
                rules
                    .iter()
                    .map(|r| {
                        let get =
                            |k: &str| r.get(k).and_then(Value::as_str).unwrap_or("").to_string();
                        (get("pattern"), get("folder"))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Category for an extension (with or without the dot), or `"Others"`.
    pub fn category_for_extension(&self, extension: &str) -> String {
        let mut ext = extension.trim().to_lowercase();
        if !ext.is_empty() && !ext.starts_with('.') {
            ext.insert(0, '.');
        }
        self.extension_map
            .get(&ext)
            .cloned()
            .unwrap_or_else(|| "Others".into())
    }

    /// Python `ConfigManager.is_ignored` for a bare name or a full path.
    pub fn is_ignored(&self, filename_or_path: &str) -> bool {
        let name = Path::new(filename_or_path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let full = filename_or_path.replace('\\', "/");
        self.ignore
            .iter()
            .any(|r| r.name.matches(&name) || r.inside.matches(&full) || r.trailing.matches(&full))
    }

    /// Size bucket label for `size_bytes`, or `"Others"`.
    pub fn size_folder(&self, size_bytes: u64) -> String {
        let Some(thresholds) = self.data.get("size_thresholds").and_then(Value::as_object) else {
            return "Others".into();
        };
        let size = size_bytes as i128;
        for (label, range) in thresholds {
            let (Some(min), Some(max)) = (
                range.get(0).and_then(Value::as_i64),
                range.get(1).and_then(Value::as_i64),
            ) else {
                continue;
            };
            let (min, max) = (min as i128, max as i128);
            if max == -1 {
                if size >= min {
                    return label.clone();
                }
            } else if min <= size && size < max {
                return label.clone();
            }
        }
        "Others".into()
    }

    /// Writes the current tree to `~/.smart_file_organizer/config.json`.
    pub fn save_user_config(&self) -> Result<PathBuf> {
        let path = Self::user_config_path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, serde_json::to_string_pretty(&self.data)?)?;
        Ok(path)
    }

    fn str_or(&self, key: &str, default: &str) -> String {
        self.data
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or(default)
            .to_string()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::defaults()
    }
}

fn string_list(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Python `_deep_merge`: nested objects merge, everything else replaces.
pub fn deep_merge(base: &mut Map<String, Value>, over: Map<String, Value>) {
    for (k, v) in over {
        match (base.get_mut(&k), v) {
            (Some(Value::Object(b)), Value::Object(o)) => deep_merge(b, o),
            (_, v) => {
                base.insert(k, v);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn default_categories_loaded() {
        let cfg = Config::defaults();
        let cats: HashMap<_, _> = cfg.categories().into_iter().collect();
        assert!(cats["Images"].contains(&".jpg".to_string()));
        assert!(cats["Documents"].contains(&".pdf".to_string()));
        assert!(cats.contains_key("Audio"));
    }

    #[test]
    fn extension_lookup() {
        let cfg = Config::defaults();
        assert_eq!(cfg.category_for_extension(".png"), "Images");
        assert_eq!(cfg.category_for_extension("docx"), "Documents");
        assert_eq!(cfg.category_for_extension(".py"), "Code");
        assert_eq!(cfg.category_for_extension(".ts"), "Code"); // later category wins
        assert_eq!(cfg.category_for_extension(".xyz123"), "Others");
    }

    #[test]
    fn ignore_patterns() {
        let cfg = Config::defaults();
        assert!(cfg.is_ignored(".git"));
        assert!(cfg.is_ignored("desktop.ini"));
        assert!(cfg.is_ignored("video.mp4.crdownload"));
        assert!(cfg.is_ignored("temp.tmp"));
        assert!(!cfg.is_ignored("document.pdf"));
    }

    #[test]
    fn size_buckets() {
        let cfg = Config::defaults();
        assert_eq!(cfg.size_folder(500), "Tiny (< 1MB)");
        assert_eq!(cfg.size_folder(5 * 1024 * 1024), "Small (1MB - 10MB)");
        assert_eq!(cfg.size_folder(50 * 1024 * 1024), "Medium (10MB - 100MB)");
        assert_eq!(cfg.size_folder(500 * 1024 * 1024), "Large (100MB - 1GB)");
        assert_eq!(cfg.size_folder(2 * 1024 * 1024 * 1024), "Huge (> 1GB)");
    }

    #[test]
    fn deep_merge_and_custom_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.json");
        std::fs::write(
            &path,
            json!({"collision_strategy": "skip", "watcher": {"debounce_seconds": 9.0}}).to_string(),
        )
        .unwrap();
        let cfg = Config::load(Some(&path)).unwrap();
        assert_eq!(cfg.collision_strategy(), CollisionStrategy::Skip);
        assert_eq!(cfg.watcher_settings(), (9.0, 2.0));
        assert_eq!(cfg.category_for_extension(".png"), "Images");
    }

    #[test]
    fn bad_collision_value_falls_back() {
        let mut cfg = Config::defaults();
        cfg.set("collision_strategy", json!("nope"));
        assert_eq!(cfg.collision_strategy(), CollisionStrategy::AutoRename);
    }
}
