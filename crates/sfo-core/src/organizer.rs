//! Rule-based planner: computes a destination for every file.

use crate::config::Config;
use crate::models::{ActionRecord, ActionType, FileInfo, OrganizeStrategy, RecordStatus};
use crate::paths;
use crate::smart_filter;
use chrono::format::{Item, StrftimeItems};
use chrono::{Datelike, NaiveDateTime};
use regex::RegexBuilder;
use std::path::Path;

/// Planning options beyond the strategy.
#[derive(Debug, Clone, Copy, Default)]
pub struct PlanOptions {
    /// Add a date subfolder under each category (ignored for the date strategy).
    pub sub_organize_by_date: bool,
    /// Route invoices to `Finance_<year>/<vendor>`.
    pub smart_receipts: bool,
}

/// Computes destination paths according to a strategy.
#[derive(Debug, Clone)]
pub struct Organizer<'c> {
    config: &'c Config,
}

impl<'c> Organizer<'c> {
    pub fn new(config: &'c Config) -> Self {
        Self { config }
    }

    /// Plans one `MOVE` (or `SKIP`, when already in place) record per file.
    pub fn plan(
        &self,
        files: &[FileInfo],
        destination_dir: &Path,
        strategy: OrganizeStrategy,
        opts: PlanOptions,
    ) -> Vec<ActionRecord> {
        let dest_root = paths::resolve(destination_dir);
        files
            .iter()
            .map(|f| {
                let mut subfolder = None;
                let mut prefix = "Organize".to_string();

                if opts.smart_receipts
                    && smart_filter::RECEIPT_EXTENSIONS
                        .contains(&f.extension.to_lowercase().as_str())
                {
                    let receipt = smart_filter::analyze_file(f);
                    if receipt.is_receipt {
                        let vendor = receipt.vendor.clone().unwrap_or_else(|| "Invoices".into());
                        let year = receipt.year.unwrap_or_else(|| f.modified_time.year());
                        subfolder = Some(format!("Finance_{year}/{vendor}"));
                        prefix = format!("Smart Sort ({})", receipt.summary);
                    }
                }
                let is_financial = subfolder.is_some();
                let subfolder = subfolder.unwrap_or_else(|| self.determine_folder(f, strategy));

                let mut target_dir = paths::join_rel(&dest_root, &subfolder);
                if opts.sub_organize_by_date && strategy != OrganizeStrategy::Date && !is_financial
                {
                    target_dir = paths::join_rel(&target_dir, &self.date_folder(f));
                }
                let target = target_dir.join(&f.filename);

                let mut rec = ActionRecord::new(
                    ActionType::Move,
                    f.path.clone(),
                    Some(target.clone()),
                    f.size_bytes,
                );
                if paths::path_eq(&paths::resolve(&f.path), &paths::resolve(&target)) {
                    rec.action_type = ActionType::Skip;
                    rec.status = RecordStatus::Skipped;
                    rec.message = "File already in correct destination".into();
                } else {
                    rec.message = format!("{prefix} into '{subfolder}'");
                }
                rec
            })
            .collect()
    }

    /// Folder name for a file under `strategy`.
    pub fn determine_folder(&self, f: &FileInfo, strategy: OrganizeStrategy) -> String {
        match strategy {
            OrganizeStrategy::Date => self.date_folder(f),
            OrganizeStrategy::Size => self.config.size_folder(f.size_bytes),
            OrganizeStrategy::Regex => self.match_regex_rules(f),
            OrganizeStrategy::Extension => self.config.category_for_extension(&f.extension),
        }
    }

    /// Date folder from the configured attribute and format.
    pub fn date_folder(&self, f: &FileInfo) -> String {
        let dt = if self.config.date_attribute() == "modified" {
            f.modified_time
        } else {
            f.created_time
        };
        format_date_folder(&dt, &self.config.date_format())
    }

    fn match_regex_rules(&self, f: &FileInfo) -> String {
        for (pattern, folder) in self.config.custom_regex_rules() {
            if pattern.is_empty() || folder.is_empty() {
                continue;
            }
            // Patterns Rust's regex engine cannot compile are skipped, like `re.error`.
            if let Ok(re) = RegexBuilder::new(&pattern).case_insensitive(true).build()
                && re.is_match(&f.filename)
            {
                return folder;
            }
        }
        self.config.category_for_extension(&f.extension)
    }
}

/// Named presets, otherwise a `strftime` format; invalid formats fall back
/// to `YYYY/MM`.
pub fn format_date_folder(dt: &NaiveDateTime, fmt: &str) -> String {
    let (y, m, d) = (dt.year(), dt.month(), dt.day());
    match fmt {
        "YYYY/MM" => format!("{y}/{m:02}"),
        "YYYY-MM" => format!("{y}-{m:02}"),
        "YYYY/MM/DD" => format!("{y}/{m:02}/{d:02}"),
        "YYYY-MM-DD" => format!("{y}-{m:02}-{d:02}"),
        other => {
            let items: Vec<Item> = StrftimeItems::new(other).collect();
            if items.iter().any(|i| matches!(i, Item::Error)) {
                format!("{y}/{m:02}")
            } else {
                dt.format_with_items(items.into_iter()).to_string()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn file(dir: &Path, name: &str, size: usize) -> FileInfo {
        let p = dir.join(name);
        std::fs::write(&p, vec![b'x'; size]).unwrap();
        FileInfo::from_path(&p).unwrap()
    }

    fn parent_name(r: &ActionRecord) -> String {
        r.destination_path
            .as_ref()
            .unwrap()
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn by_extension_and_size() {
        let d = tempfile::tempdir().unwrap();
        let files = vec![
            file(d.path(), "photo1.jpg", 5),
            file(d.path(), "report.pdf", 5),
            file(d.path(), "script.py", 5),
        ];
        let cfg = Config::defaults();
        let org = Organizer::new(&cfg);

        let recs = org.plan(
            &files,
            d.path(),
            OrganizeStrategy::Extension,
            PlanOptions::default(),
        );
        let folders: Vec<String> = recs.iter().map(parent_name).collect();
        assert_eq!(folders, ["Images", "Documents", "Code"]);
        assert_eq!(recs[0].message, "Organize into 'Images'");
        assert_eq!(recs[0].status, RecordStatus::Pending);

        let recs = org.plan(
            &files,
            d.path(),
            OrganizeStrategy::Size,
            PlanOptions::default(),
        );
        assert!(recs.iter().all(|r| parent_name(r).starts_with("Tiny")));
    }

    #[test]
    fn skip_when_already_in_place() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir(d.path().join("Images")).unwrap();
        let f = file(&d.path().join("Images"), "a.png", 1);
        let cfg = Config::defaults();
        let recs = Organizer::new(&cfg).plan(
            &[f],
            d.path(),
            OrganizeStrategy::Extension,
            PlanOptions::default(),
        );
        assert_eq!(recs[0].action_type, ActionType::Skip);
        assert_eq!(recs[0].status, RecordStatus::Skipped);
    }

    #[test]
    fn regex_rules_and_date_subfolders() {
        let d = tempfile::tempdir().unwrap();
        let f = file(d.path(), "INVOICE_2026.txt", 1);
        let mut cfg = Config::defaults();
        cfg.set(
            "custom_regex_rules",
            json!([{"pattern": "(?<=x)bad", "folder": "Broken"}, {"pattern": "^invoice", "folder": "Bills"}]),
        );
        let org = Organizer::new(&cfg);
        let recs = org.plan(
            std::slice::from_ref(&f),
            d.path(),
            OrganizeStrategy::Regex,
            PlanOptions::default(),
        );
        assert_eq!(parent_name(&recs[0]), "Bills");

        let opts = PlanOptions {
            sub_organize_by_date: true,
            ..Default::default()
        };
        let recs = org.plan(
            std::slice::from_ref(&f),
            d.path(),
            OrganizeStrategy::Extension,
            opts,
        );
        let want = d
            .path()
            .join("Documents")
            .join(f.modified_time.year().to_string())
            .join(format!("{:02}", f.modified_time.month()))
            .join("INVOICE_2026.txt");
        assert!(paths::path_eq(
            recs[0].destination_path.as_ref().unwrap(),
            &paths::resolve(&want)
        ));
    }

    #[test]
    fn date_formats() {
        let dt = paths::py_fromisoformat("2026-03-07T10:00:00").unwrap();
        assert_eq!(format_date_folder(&dt, "YYYY/MM"), "2026/03");
        assert_eq!(format_date_folder(&dt, "YYYY-MM-DD"), "2026-03-07");
        assert_eq!(format_date_folder(&dt, "%Y_%b"), "2026_Mar");
        assert_eq!(format_date_folder(&dt, "%Q"), "2026/03");
    }
}
