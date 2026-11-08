//! Metadata-driven bulk renamer (templates, EXIF, ID3, casing, find/replace).

use crate::models::{ActionRecord, ActionType, FileInfo, RecordStatus};
use crate::paths;
use regex::Regex;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::OnceLock;
use unicode_normalization::UnicodeNormalization;

pub const IMAGE_EXTS: [&str; 10] = [
    ".jpg", ".jpeg", ".png", ".tiff", ".webp", ".raw", ".cr2", ".nef", ".arw", ".heic",
];
pub const AUDIO_EXTS: [&str; 6] = [".mp3", ".flac", ".wav", ".aac", ".m4a", ".ogg"];

/// Photo metadata used by templates.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExifMeta {
    pub date: Option<String>,
    pub time: Option<String>,
    pub camera: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub iso: Option<String>,
    pub shutter: Option<String>,
    pub aperture: Option<String>,
    pub lens: Option<String>,
}

/// Reads EXIF; date/time fall back to the file's mtime.
pub fn extract_exif(path: &Path) -> ExifMeta {
    let mut m = ExifMeta::default();
    if let Ok((w, h)) = image::image_dimensions(path) {
        m.width = Some(w);
        m.height = Some(h);
    }
    if let Some(exif) = File::open(path).ok().and_then(|f| {
        exif::Reader::new()
            .read_from_container(&mut BufReader::new(f))
            .ok()
    }) {
        use exif::{In, Tag, Value};
        let ascii = |tag| {
            exif.get_field(tag, In::PRIMARY)
                .and_then(|f| match &f.value {
                    Value::Ascii(v) => v.first().map(|b| {
                        String::from_utf8_lossy(b)
                            .trim_matches(['\0', ' '])
                            .to_string()
                    }),
                    _ => None,
                })
        };
        let rational = |tag| {
            exif.get_field(tag, In::PRIMARY)
                .and_then(|f| match &f.value {
                    Value::Rational(v) => v.first().map(|r| r.to_f64()),
                    _ => None,
                })
        };

        if let Some(dt) = ascii(Tag::DateTimeOriginal)
            .or_else(|| ascii(Tag::DateTime))
            .filter(|s| !s.is_empty())
        {
            let mut parts = dt.split(' ');
            if let Some(d) = parts.next() {
                m.date = Some(d.replace(':', "-"));
            }
            if let Some(t) = parts.next() {
                m.time = Some(t.replace(':', ""));
            }
        }
        let make = ascii(Tag::Make).unwrap_or_default();
        let model = ascii(Tag::Model).unwrap_or_default();
        if !model.is_empty() {
            m.camera = Some(if make.is_empty() || model.contains(&make) {
                model
            } else {
                format!("{make}_{model}")
            });
        }
        if let Some(f) = exif.get_field(Tag::PhotographicSensitivity, In::PRIMARY) {
            m.iso = f.value.get_uint(0).map(|v| v.to_string());
        }
        if let Some(fnum) = rational(Tag::FNumber) {
            m.aperture = Some(format!("f{fnum:.1}"));
        }
        if let Some(exp) = rational(Tag::ExposureTime).filter(|e| *e > 0.0) {
            m.shutter = Some(if exp < 1.0 {
                format!("1-{}", (1.0 / exp) as i64)
            } else {
                format!("{exp:.1}s")
            });
        }
        m.lens = ascii(Tag::LensModel).filter(|s| !s.is_empty());
    }
    if m.date.is_none()
        && let Ok(mtime) = std::fs::metadata(path).and_then(|md| md.modified())
    {
        let dt = paths::local_naive(mtime);
        m.date = Some(dt.format("%Y-%m-%d").to_string());
        m.time = Some(dt.format("%H%M%S").to_string());
    }
    m
}

/// Audio tags used by templates.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Id3Meta {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<String>,
    pub track: Option<String>,
    pub genre: Option<String>,
}

impl Id3Meta {
    fn slot(&mut self, frame: &str) -> Option<&mut Option<String>> {
        Some(match frame {
            "TIT2" => &mut self.title,
            "TPE1" => &mut self.artist,
            "TALB" => &mut self.album,
            "TYER" | "TDRC" => &mut self.year,
            "TRCK" => &mut self.track,
            "TCON" => &mut self.genre,
            _ => return None,
        })
    }
}

fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| char::from(c)).collect()
}

fn utf16(b: &[u8], big_endian_default: bool) -> String {
    let (be, body) = match b {
        [0xFF, 0xFE, rest @ ..] => (false, rest),
        [0xFE, 0xFF, rest @ ..] => (true, rest),
        _ => (big_endian_default, b),
    };
    let units: Vec<u16> = body
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| {
            if be {
                u16::from_be_bytes(*c)
            } else {
                u16::from_le_bytes(*c)
            }
        })
        .collect();
    String::from_utf16_lossy(&units)
}

/// ID3v2 frames, then ID3v1 trailer for missing title/artist/album/year.
pub fn extract_id3(path: &Path) -> Id3Meta {
    let mut m = Id3Meta::default();
    let Ok(mut f) = File::open(path) else {
        return m;
    };
    let mut header = [0u8; 10];
    if f.read_exact(&mut header).is_ok() && &header[..3] == b"ID3" {
        let size = (usize::from(header[6] & 0x7F) << 21)
            | (usize::from(header[7] & 0x7F) << 14)
            | (usize::from(header[8] & 0x7F) << 7)
            | usize::from(header[9] & 0x7F);
        let mut data = Vec::new();
        if (&mut f)
            .take(size.min(200_000) as u64)
            .read_to_end(&mut data)
            .is_ok()
        {
            parse_id3v2_frames(&data, &mut m);
        }
    }
    if (m.title.is_none() || m.artist.is_none()) && f.seek(SeekFrom::End(-128)).is_ok() {
        let mut v1 = [0u8; 128];
        if f.read_exact(&mut v1).is_ok() && &v1[..3] == b"TAG" {
            let field = |r: std::ops::Range<usize>| {
                latin1(&v1[r]).trim_matches(['\0', ' ']).trim().to_string()
            };
            m.title.get_or_insert_with(|| field(3..33));
            m.artist.get_or_insert_with(|| field(33..63));
            m.album.get_or_insert_with(|| field(63..93));
            m.year.get_or_insert_with(|| field(93..97));
        }
    }
    m
}

fn parse_id3v2_frames(data: &[u8], m: &mut Id3Meta) {
    let mut idx = 0;
    while idx + 10 < data.len() {
        let id = latin1(&data[idx..idx + 4]);
        if !id.chars().all(|c| c.is_ascii_alphanumeric()) || m.slot(&id).is_none() {
            idx += 1;
            continue;
        }
        let size = u32::from_be_bytes([data[idx + 4], data[idx + 5], data[idx + 6], data[idx + 7]])
            as usize;
        if size == 0 || idx + 10 + size > data.len() {
            break;
        }
        let content = &data[idx + 10..idx + 10 + size];
        let (enc, raw) = content
            .split_first()
            .map(|(e, r)| (*e, r))
            .unwrap_or((0, &[][..]));
        let text = match enc {
            1 => utf16(raw, false),
            2 => utf16(raw, true),
            3 => String::from_utf8_lossy(raw).into_owned(),
            _ => latin1(raw),
        };
        let clean = text.replace('\0', "").trim().to_string();
        if !clean.is_empty()
            && let Some(slot) = m.slot(&id)
        {
            *slot = Some(clean);
        }
        idx += 10 + size;
    }
}

/// NFKD + drop combining marks + `đ/Đ` → `d/D`.
pub fn remove_accents(text: &str) -> String {
    text.nfkd()
        .filter(|c| unicode_normalization::char::canonical_combining_class(*c) == 0)
        .collect::<String>()
        .replace('đ', "d")
        .replace('Đ', "D")
}

/// Python `str.title()`.
pub fn title_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_cased = false;
    for c in s.chars() {
        let cased = c.is_lowercase() || c.is_uppercase();
        if cased {
            if prev_cased {
                out.extend(c.to_lowercase())
            } else {
                out.extend(c.to_uppercase())
            }
        } else {
            out.push(c);
        }
        prev_cased = cased;
    }
    out
}

/// Converts a Python `re.sub` replacement (`\1`, `\g<1>`, `\g<name>`) to
/// `regex` syntax.
fn python_replacement(rep: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = rep.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '$' => out.push_str("$$"),
            '\\' if i + 1 < chars.len() => {
                let n = chars[i + 1];
                if n.is_ascii_digit() {
                    let mut j = i + 1;
                    while j < chars.len() && j < i + 3 && chars[j].is_ascii_digit() {
                        j += 1;
                    }
                    out.push_str(&format!(
                        "${{{}}}",
                        chars[i + 1..j].iter().collect::<String>()
                    ));
                    i = j;
                    continue;
                }
                if n == 'g'
                    && chars.get(i + 2) == Some(&'<')
                    && let Some(end) = chars[i + 3..].iter().position(|&c| c == '>')
                {
                    let name: String = chars[i + 3..i + 3 + end].iter().collect();
                    out.push_str(&format!("${{{name}}}"));
                    i += 4 + end;
                    continue;
                }
                match n {
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    '\\' => out.push('\\'),
                    other => {
                        out.push('\\');
                        out.push(other);
                    }
                }
                i += 2;
                continue;
            }
            c => out.push(c),
        }
        i += 1;
    }
    out
}

/// Options for [`plan_rename`]. `Default` = template `{name}`, counter 1, pad 3.
#[derive(Debug, Clone)]
pub struct RenameOptions {
    pub template: String,
    pub find_text: String,
    pub replace_text: String,
    /// `lowercase`, `uppercase`, `title_case`, `snake_case`, `kebab_case`,
    /// `slugify` / `no_accents`.
    pub case_transform: Option<String>,
    pub prefix: String,
    pub suffix: String,
    pub start_counter: i64,
    pub counter_padding: usize,
    pub use_regex: bool,
}

impl Default for RenameOptions {
    fn default() -> Self {
        Self {
            template: "{name}".into(),
            find_text: String::new(),
            replace_text: String::new(),
            case_transform: None,
            prefix: String::new(),
            suffix: String::new(),
            start_counter: 1,
            counter_padding: 3,
            use_regex: false,
        }
    }
}

macro_rules! re {
    ($name:ident, $pat:expr) => {
        fn $name() -> &'static Regex {
            static R: OnceLock<Regex> = OnceLock::new();
            R.get_or_init(|| Regex::new($pat).expect("static regex"))
        }
    };
}
re!(re_counter, r"\{counter:0?(\d+)d\}");
re!(re_space_dash, r"[\s\-]+");
re!(re_space_underscore, r"[\s_]+");
re!(re_non_word, r"[^\w\s-]");
re!(re_invalid_fs, r#"[\\/*?:"<>|]"#);

/// Plans one `RENAME` (or `SKIP` when unchanged) record per file.
pub fn plan_rename(files: &[FileInfo], opts: &RenameOptions) -> Vec<ActionRecord> {
    let mut counter = opts.start_counter;
    let pad = opts.counter_padding;
    let user_re = if opts.use_regex && !opts.find_text.is_empty() {
        Regex::new(&opts.find_text).ok()
    } else {
        None
    };
    let replacement = python_replacement(&opts.replace_text);

    files
        .iter()
        .map(|f| {
            let path = &f.path;
            let ext = f.extension.trim_start_matches('.').to_string();
            let old_name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let mut meta: Vec<(&str, String)> = vec![
                ("name", paths::stem(path)),
                ("ext", ext.clone()),
                ("counter", format!("{counter:0pad$}")),
                ("size", f.size_bytes.to_string()),
                ("date", f.modified_time.format("%Y-%m-%d").to_string()),
                ("time", f.modified_time.format("%H%M%S").to_string()),
            ];
            for k in [
                "camera", "artist", "title", "album", "year", "genre", "track", "iso", "aperture",
                "shutter", "lens",
            ] {
                meta.push((k, String::new()));
            }
            let mut set = |k: &str, v: Option<String>| {
                if let Some(v) = v.filter(|v| !v.is_empty())
                    && let Some(slot) = meta.iter_mut().find(|(mk, _)| *mk == k)
                {
                    slot.1 = v;
                }
            };
            let lower_ext = f.extension.to_lowercase();
            if IMAGE_EXTS.contains(&lower_ext.as_str()) {
                let e = extract_exif(path);
                set("date", e.date);
                set("time", e.time);
                set("camera", e.camera);
                set("iso", e.iso);
                set("aperture", e.aperture);
                set("shutter", e.shutter);
                set("lens", e.lens);
            } else if AUDIO_EXTS.contains(&lower_ext.as_str()) {
                let t = extract_id3(path);
                set("artist", t.artist);
                set("title", t.title);
                set("album", t.album);
                set("year", t.year);
                set("genre", t.genre);
                set("track", t.track);
            }

            let mut name = opts.template.clone();
            if let Some(c) = re_counter().captures(&name) {
                let whole = c[0].to_string();
                let w: usize = c[1].parse().unwrap_or(pad);
                name = name.replace(&whole, &format!("{counter:0w$}"));
            }
            for (k, v) in &meta {
                name = name.replace(&format!("{{{k}}}"), v);
            }

            if !opts.find_text.is_empty() {
                if opts.use_regex {
                    if let Some(re) = &user_re {
                        name = re.replace_all(&name, replacement.as_str()).into_owned();
                    }
                } else {
                    name = name.replace(&opts.find_text, &opts.replace_text);
                }
            }

            name = match opts.case_transform.as_deref() {
                Some("lowercase") => name.to_lowercase(),
                Some("uppercase") => name.to_uppercase(),
                Some("title_case") => title_case(&name),
                Some("snake_case") => re_space_dash().replace_all(&name, "_").to_lowercase(),
                Some("kebab_case") => re_space_underscore().replace_all(&name, "-").to_lowercase(),
                Some("slugify") | Some("no_accents") => {
                    let clean = re_non_word()
                        .replace_all(&remove_accents(&name), "")
                        .trim()
                        .to_string();
                    re_space_underscore()
                        .replace_all(&clean, "-")
                        .to_lowercase()
                }
                _ => name,
            };

            let stem = format!("{}{}{}", opts.prefix, name, opts.suffix)
                .trim()
                .to_string();
            let stem = re_invalid_fs().replace_all(&stem, "").into_owned();
            let target_name = if ext.is_empty() {
                stem
            } else {
                format!("{stem}.{ext}")
            };
            let target = path.parent().unwrap_or(Path::new("")).join(&target_name);

            let mut r =
                ActionRecord::new(ActionType::Rename, path.clone(), Some(target), f.size_bytes);
            if target_name == old_name {
                r.action_type = ActionType::Skip;
                r.status = RecordStatus::Skipped;
                r.message = "Name unchanged".into();
            } else {
                r.message = format!("Rename: {old_name} -> {target_name}");
            }
            counter += 1;
            r
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(dir: &Path, name: &str, data: &[u8]) -> FileInfo {
        let p = dir.join(name);
        std::fs::write(&p, data).unwrap();
        FileInfo::from_path(&p).unwrap()
    }

    fn dest_name(r: &ActionRecord) -> String {
        r.destination_path
            .as_ref()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn templates_and_counters() {
        let d = tempfile::tempdir().unwrap();
        let f = info(d.path(), "old_document.txt", b"x");
        let opts = RenameOptions {
            template: "{counter}_{name}".into(),
            ..Default::default()
        };
        assert_eq!(
            dest_name(&plan_rename(std::slice::from_ref(&f), &opts)[0]),
            "001_old_document.txt"
        );
        let opts = RenameOptions {
            template: "{counter:05d}-{name}.{ext}x".into(),
            start_counter: 7,
            ..Default::default()
        };
        assert_eq!(
            dest_name(&plan_rename(std::slice::from_ref(&f), &opts)[0]),
            "00007-old_document.txtx.txt"
        );
        let same = plan_rename(&[f], &RenameOptions::default());
        assert_eq!(same[0].action_type, ActionType::Skip);
        assert_eq!(same[0].message, "Name unchanged");
    }

    #[test]
    fn case_transformations() {
        let d = tempfile::tempdir().unwrap();
        let f = info(d.path(), "My Cool Project_V1.txt", b"x");
        let run = |case: &str| {
            let o = RenameOptions {
                case_transform: Some(case.into()),
                ..Default::default()
            };
            dest_name(&plan_rename(std::slice::from_ref(&f), &o)[0])
        };
        assert_eq!(run("snake_case"), "my_cool_project_v1.txt");
        assert_eq!(run("uppercase"), "MY COOL PROJECT_V1.txt");
        assert_eq!(run("kebab_case"), "my-cool-project-v1.txt");
        assert_eq!(run("title_case"), "My Cool Project_V1.txt");

        let g = info(d.path(), "Hóa Đơn Tiền Điện Tháng 10.pdf", b"x");
        let o = RenameOptions {
            case_transform: Some("slugify".into()),
            ..Default::default()
        };
        assert_eq!(
            dest_name(&plan_rename(&[g], &o)[0]),
            "hoa-don-tien-dien-thang-10.pdf"
        );
        assert_eq!(title_case("hello wORLD2x o'neil"), "Hello World2X O'Neil");
    }

    #[test]
    fn find_replace_plain_and_regex() {
        let d = tempfile::tempdir().unwrap();
        let f = info(d.path(), "IMG_2026_RAW.jpg", b"dummy");
        let o = RenameOptions {
            find_text: "IMG_".into(),
            replace_text: "PHOTO_".into(),
            ..Default::default()
        };
        assert_eq!(
            dest_name(&plan_rename(std::slice::from_ref(&f), &o)[0]),
            "PHOTO_2026_RAW.jpg"
        );
        let o = RenameOptions {
            find_text: r"IMG_(\d+)_(\w+)".into(),
            replace_text: r"\2-\g<1>$".into(),
            use_regex: true,
            ..Default::default()
        };
        assert_eq!(dest_name(&plan_rename(&[f], &o)[0]), "RAW-2026$.jpg");
    }

    #[test]
    fn id3_v1_and_v2() {
        let d = tempfile::tempdir().unwrap();
        let mut v1 = b"FAKE_AUDIO_DATA".repeat(10);
        v1.extend_from_slice(b"TAG");
        for (s, n) in [
            (&b"Test Song"[..], 30),
            (b"Test Artist", 30),
            (b"Test Album", 30),
            (b"2026", 4),
            (b"Comment", 30),
        ] {
            let mut field = s.to_vec();
            field.resize(n, 0);
            v1.extend(field);
        }
        v1.push(0x0c);
        let p = d.path().join("test.mp3");
        std::fs::write(&p, &v1).unwrap();
        let t = extract_id3(&p);
        assert_eq!(t.title.as_deref(), Some("Test Song"));
        assert_eq!(t.artist.as_deref(), Some("Test Artist"));
        assert_eq!(t.album.as_deref(), Some("Test Album"));
        assert_eq!(t.year.as_deref(), Some("2026"));

        // ID3v2.3 with a UTF-16 (BOM) title frame and a UTF-8 artist frame.
        let mut frames = Vec::new();
        let mut title = vec![1u8, 0xFF, 0xFE];
        for u in "Bài Hát".encode_utf16() {
            title.extend(u.to_le_bytes());
        }
        frames.extend(b"TIT2");
        frames.extend((title.len() as u32).to_be_bytes());
        frames.extend([0, 0]);
        frames.extend(&title);
        let artist = [&[3u8][..], "Sơn Tùng".as_bytes()].concat();
        frames.extend(b"TPE1");
        frames.extend((artist.len() as u32).to_be_bytes());
        frames.extend([0, 0]);
        frames.extend(&artist);
        frames.extend([0u8; 16]);
        let size = frames.len();
        let mut v2 = b"ID3\x03\x00\x00".to_vec();
        v2.extend([
            (size >> 21) as u8 & 0x7F,
            (size >> 14) as u8 & 0x7F,
            (size >> 7) as u8 & 0x7F,
            size as u8 & 0x7F,
        ]);
        v2.extend(frames);
        let p2 = d.path().join("v2.mp3");
        std::fs::write(&p2, v2).unwrap();
        let t = extract_id3(&p2);
        assert_eq!(t.title.as_deref(), Some("Bài Hát"));
        assert_eq!(t.artist.as_deref(), Some("Sơn Tùng"));
    }

    #[test]
    fn exif_falls_back_to_mtime() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("x.jpg");
        image::RgbImage::new(4, 3).save(&p).unwrap();
        let e = extract_exif(&p);
        assert_eq!((e.width, e.height), (Some(4), Some(3)));
        assert_eq!(e.date.as_deref().map(str::len), Some(10));
        assert!(e.camera.is_none());
    }
}
