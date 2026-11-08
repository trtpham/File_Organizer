//! "LAN Drop": a small HTTP server for sharing a folder with phones and
//! laptops on the same network (download with Range support, upload, PIN).
//!
//! Hardening over Python v3.0: HTML output is escaped, PINs are compared in
//! constant time, uploads are size-capped and never overwrite existing files,
//! and invalid Range requests get `416`.

use crate::fsutil;
use crate::paths;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::net::{SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

/// Largest accepted upload body.
pub const MAX_UPLOAD_BYTES: u64 = 512 * 1024 * 1024;

const LOGIN_HTML: &str = include_str!("../data/lan_login.html");
const DASHBOARD_HTML: &str = include_str!("../data/lan_dashboard.html");

/// Primary LAN IPv4 address (no packet is sent), or `127.0.0.1`.
pub fn local_ip() -> String {
    UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| {
            s.connect("8.8.8.8:80")?;
            s.local_addr()
        })
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".into())
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn header(k: &str, v: &str) -> Header {
    Header::from_bytes(k.as_bytes(), v.as_bytes()).expect("ASCII header")
}

fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    let hex = |b: u8| (b as char).to_digit(16).map(|d| d as u8);
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => match (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                (Some(h), Some(l)) => {
                    out.push(h * 16 + l);
                    i += 3;
                    continue;
                }
                _ => out.push(b'%'),
            },
            b'+' => out.push(b' '),
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn percent_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn query_param(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
        (percent_decode(k) == key).then(|| percent_decode(v))
    })
}

struct Ctx {
    shared: PathBuf,
    pin: Option<String>,
}

impl Ctx {
    fn authed(&self, req: &Request, query: &str) -> bool {
        let Some(pin) = &self.pin else { return true };
        let cookie_ok = req
            .headers()
            .iter()
            .filter(|h| h.field.equiv("Cookie"))
            .any(|h| {
                h.value
                    .as_str()
                    .split(';')
                    .filter_map(|c| c.trim().strip_prefix("sfo_pin="))
                    .any(|v| ct_eq(v.as_bytes(), pin.as_bytes()))
            });
        cookie_ok || query_param(query, "pin").is_some_and(|v| ct_eq(v.as_bytes(), pin.as_bytes()))
    }

    fn handle(&self, mut req: Request) {
        let url = req.url().to_string();
        let (path, query) = url.split_once('?').unwrap_or((&url, ""));
        let path = percent_decode(path);
        let method = req.method().clone();

        if method == Method::Post && path == "/login" {
            let mut body = String::new();
            let _ = req.as_reader().take(4096).read_to_string(&mut body);
            let entered = query_param(&body, "pin").unwrap_or_default();
            match &self.pin {
                Some(pin) if ct_eq(entered.as_bytes(), pin.as_bytes()) => {
                    let resp = Response::empty(StatusCode(303))
                        .with_header(header("Location", "/"))
                        .with_header(header(
                            "Set-Cookie",
                            &format!("sfo_pin={pin}; Path=/; HttpOnly; SameSite=Strict"),
                        ));
                    let _ = req.respond(resp);
                }
                _ => self.login(req, Some("Invalid PIN. Please try again.")),
            }
            return;
        }
        if !self.authed(&req, query) {
            if method == Method::Post {
                let _ = req.respond(Response::from_string("Forbidden").with_status_code(403));
            } else {
                self.login(req, None);
            }
            return;
        }
        match (method, path.as_str()) {
            (Method::Post, "/upload") => self.upload(req),
            (Method::Post, _) => {
                let _ = req.respond(Response::from_string("Not Found").with_status_code(404));
            }
            (_, p) if p.starts_with("/download/") => self.download(req, &p["/download/".len()..]),
            _ => self.dashboard(req),
        }
    }

    fn html(req: Request, status: u16, body: String) {
        let _ = req.respond(
            Response::from_string(body)
                .with_status_code(status)
                .with_header(header("Content-Type", "text/html; charset=utf-8")),
        );
    }

    fn login(&self, req: Request, error: Option<&str>) {
        let err = error
            .map(|e| {
                format!(
                    "<p style=\"color: #ef4444; font-weight: bold;\">{}</p>",
                    esc(e)
                )
            })
            .unwrap_or_default();
        Self::html(req, 200, LOGIN_HTML.replace("{{ERROR}}", &err));
    }

    fn dashboard(&self, req: Request) {
        let mut files: Vec<(String, u64)> = fs::read_dir(&self.shared)
            .map(|it| {
                it.flatten()
                    .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
                    .map(|e| {
                        (
                            e.file_name().to_string_lossy().into_owned(),
                            e.metadata().map(|m| m.len()).unwrap_or(0),
                        )
                    })
                    .filter(|(n, _)| !n.starts_with('.'))
                    .collect()
            })
            .unwrap_or_default();
        files.sort();
        let rows: String = files
            .iter()
            .map(|(name, size)| {
                let mb = *size as f64 / (1024.0 * 1024.0);
                let size = if mb >= 1.0 { format!("{mb:.2} MB") } else { format!("{:.1} KB", *size as f64 / 1024.0) };
                format!(
                    "\n            <tr style=\"border-bottom: 1px solid #1e293b;\">\n                <td style=\"padding: 12px 10px; font-weight: 500;\">📄 {}</td>\n                <td style=\"padding: 12px 10px; color: #94a3b8;\">{size}</td>\n                <td style=\"padding: 12px 10px; text-align: right;\">\n                    <a href=\"/download/{}\" style=\"background: #3b82f6; color: #fff; text-decoration: none; padding: 6px 12px; border-radius: 6px; font-size: 13px; font-weight: bold;\">Download</a>\n                </td>\n            </tr>\n            ",
                    esc(name),
                    percent_encode(name)
                )
            })
            .collect();
        let table = if rows.is_empty() {
            "<tr><td colspan=\"3\" style=\"text-align: center; padding: 24px; color: #64748b;\">No files in shared directory.</td></tr>".to_string()
        } else {
            rows
        };
        let html = DASHBOARD_HTML
            .replace("{{SHARED_NAME}}", &esc(&fsutil::name_of(&self.shared)))
            .replace("{{COUNT}}", &files.len().to_string())
            .replace("{{ROWS}}", &table);
        Self::html(req, 200, html);
    }

    fn download(&self, req: Request, rel: &str) {
        let target = paths::resolve(&self.shared.join(rel));
        let inside =
            paths::is_within(&target, &self.shared) && !paths::path_eq(&target, &self.shared);
        if !inside || !target.is_file() {
            let _ = req.respond(Response::from_string("File Not Found").with_status_code(404));
            return;
        }
        let Ok(mut file) = fs::File::open(&target) else {
            let _ =
                req.respond(Response::from_string("Failed to stream file").with_status_code(500));
            return;
        };
        let size = file.metadata().map(|m| m.len()).unwrap_or(0);
        let disp = format!(
            "attachment; filename=\"{}\"",
            fsutil::name_of(&target).replace(['"', '\r', '\n'], "_")
        );
        let disp = header(
            "Content-Disposition",
            &disp
                .chars()
                .map(|c| if c.is_ascii() { c } else { '_' })
                .collect::<String>(),
        );
        let range = req
            .headers()
            .iter()
            .find(|h| h.field.equiv("Range"))
            .and_then(|h| h.value.as_str().strip_prefix("bytes=").map(str::to_string));

        if let Some(spec) = range
            && let Some((s, e)) = spec.split(',').next().and_then(|r| r.split_once('-'))
            && let Ok(start) = s.trim().parse::<u64>()
        {
            let end = e
                .trim()
                .parse::<u64>()
                .ok()
                .map_or(size.saturating_sub(1), |e| e.min(size.saturating_sub(1)));
            if start >= size || start > end {
                let resp = Response::empty(StatusCode(416))
                    .with_header(header("Content-Range", &format!("bytes */{size}")));
                let _ = req.respond(resp);
                return;
            }
            let len = end - start + 1;
            if file.seek(SeekFrom::Start(start)).is_err() {
                let _ = req
                    .respond(Response::from_string("Failed to stream file").with_status_code(500));
                return;
            }
            let resp = Response::new(
                StatusCode(206),
                vec![],
                file.take(len),
                Some(len as usize),
                None,
            )
            .with_header(header("Content-Type", "application/octet-stream"))
            .with_header(disp)
            .with_header(header(
                "Content-Range",
                &format!("bytes {start}-{end}/{size}"),
            ))
            .with_header(header("Accept-Ranges", "bytes"));
            let _ = req.respond(resp);
            return;
        }
        let resp = Response::new(StatusCode(200), vec![], file, Some(size as usize), None)
            .with_header(header("Content-Type", "application/octet-stream"))
            .with_header(disp)
            .with_header(header("Accept-Ranges", "bytes"));
        let _ = req.respond(resp);
    }

    fn upload(&self, mut req: Request) {
        let ctype = req
            .headers()
            .iter()
            .find(|h| h.field.equiv("Content-Type"))
            .map(|h| h.value.as_str().to_string())
            .unwrap_or_default();
        if !ctype.starts_with("multipart/form-data") {
            let _ = req.respond(
                Response::from_string("Bad Request: Expected multipart/form-data")
                    .with_status_code(400),
            );
            return;
        }
        let Some(boundary) = ctype
            .split(';')
            .find_map(|p| p.trim().strip_prefix("boundary="))
            .map(|b| b.trim_matches('"').to_string())
        else {
            let _ = req.respond(
                Response::from_string("Bad Request: No boundary found").with_status_code(400),
            );
            return;
        };
        if req
            .body_length()
            .is_some_and(|n| n as u64 > MAX_UPLOAD_BYTES)
        {
            let _ = req.respond(Response::from_string("Payload Too Large").with_status_code(413));
            return;
        }
        let mut body = Vec::new();
        if req
            .as_reader()
            .take(MAX_UPLOAD_BYTES + 1)
            .read_to_end(&mut body)
            .is_err()
            || body.len() as u64 > MAX_UPLOAD_BYTES
        {
            let _ = req.respond(Response::from_string("Payload Too Large").with_status_code(413));
            return;
        }
        let saved = save_multipart(
            &body,
            boundary.as_bytes(),
            &self.shared.join("SFO_Incoming"),
        );
        let resp = Response::empty(StatusCode(303))
            .with_header(header("Location", &format!("/?uploaded={saved}")));
        let _ = req.respond(resp);
    }
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    hay.get(from..)?
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

/// Writes every file part into `incoming`; returns how many were saved.
fn save_multipart(body: &[u8], boundary: &[u8], incoming: &Path) -> usize {
    let delim = [b"--", boundary].concat();
    let mut saved = 0;
    let mut pos = 0;
    while let Some(start) = find(body, &delim, pos) {
        let part_start = start + delim.len();
        let next = find(body, &delim, part_start).unwrap_or(body.len());
        let part = &body[part_start..next];
        pos = next;
        let Some(hend) = find(part, b"\r\n\r\n", 0) else {
            continue;
        };
        let headers = String::from_utf8_lossy(&part[..hend]);
        let mut data = &part[hend + 4..];
        if data.ends_with(b"\r\n") {
            data = &data[..data.len() - 2];
        }
        let Some(fname) = headers
            .split("filename=\"")
            .nth(1)
            .and_then(|r| r.split('"').next())
        else {
            continue;
        };
        let name = fsutil::name_of(Path::new(&fname.replace('\\', "/")));
        if name.is_empty() || name == "." || name == ".." {
            continue;
        }
        if fs::create_dir_all(incoming).is_err() {
            return saved;
        }
        let mut dest = incoming.join(&name);
        if dest.exists() {
            dest = crate::executor::unique_name(&dest);
        }
        if fs::write(&dest, data).is_ok() {
            saved += 1;
        }
    }
    saved
}

/// Running server handle.
pub struct LanShareServer {
    pub shared_dir: PathBuf,
    pub port: u16,
    pub pin: Option<String>,
    server: Option<Arc<Server>>,
    stop: Arc<AtomicBool>,
    workers: Vec<JoinHandle<()>>,
}

impl LanShareServer {
    pub fn new(shared_dir: &Path, port: u16, pin: Option<String>) -> Self {
        Self {
            shared_dir: paths::resolve(shared_dir),
            port,
            pin: pin.filter(|p| !p.is_empty()),
            server: None,
            stop: Arc::new(AtomicBool::new(false)),
            workers: Vec::new(),
        }
    }

    /// Binds `0.0.0.0:port` (port 0 = any free port) with 8 worker threads.
    pub fn start(&mut self) -> std::io::Result<String> {
        if self.is_running() {
            return Ok(self.url());
        }
        let server = Arc::new(Server::http(("0.0.0.0", self.port)).map_err(std::io::Error::other)?);
        if let Some(SocketAddr::V4(a)) = server.server_addr().to_ip() {
            self.port = a.port();
        }
        self.stop.store(false, Ordering::SeqCst);
        let ctx = Arc::new(Ctx {
            shared: self.shared_dir.clone(),
            pin: self.pin.clone(),
        });
        for _ in 0..8 {
            let (srv, ctx, stop) = (
                Arc::clone(&server),
                Arc::clone(&ctx),
                Arc::clone(&self.stop),
            );
            self.workers.push(std::thread::spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    match srv.recv_timeout(Duration::from_millis(200)) {
                        Ok(Some(req)) => ctx.handle(req),
                        Ok(None) => {}
                        Err(_) => break,
                    }
                }
            }));
        }
        self.server = Some(server);
        Ok(self.url())
    }

    pub fn stop(&mut self) -> bool {
        let Some(server) = self.server.take() else {
            return false;
        };
        self.stop.store(true, Ordering::SeqCst);
        server.unblock();
        for w in self.workers.drain(..) {
            let _ = w.join();
        }
        true
    }

    pub fn is_running(&self) -> bool {
        self.server.is_some()
    }

    pub fn url(&self) -> String {
        format!("http://{}:{}", local_ip(), self.port)
    }
}

impl Drop for LanShareServer {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpStream;

    fn http(port: u16, raw: &str) -> (u16, String, Vec<u8>) {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.write_all(raw.as_bytes()).unwrap();
        let mut r = BufReader::new(s);
        let mut status = String::new();
        r.read_line(&mut status).unwrap();
        let code = status.split(' ').nth(1).unwrap().parse().unwrap();
        let mut headers = String::new();
        loop {
            let mut l = String::new();
            r.read_line(&mut l).unwrap();
            if l == "\r\n" || l.is_empty() {
                break;
            }
            headers.push_str(&l);
        }
        let mut body = Vec::new();
        let _ = r.read_to_end(&mut body);
        (code, headers, body)
    }

    #[test]
    fn pin_dashboard_download_range_upload() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("R&D 'q'.txt"), "0123456789").unwrap();
        fs::write(d.path().join(".hidden"), "h").unwrap();
        fs::write(d.path().parent().unwrap().join("sfo_outside_lan.txt"), "no").ok();
        let mut srv = LanShareServer::new(d.path(), 0, Some("4321".into()));
        srv.start().unwrap();
        let port = srv.port;
        let get = |path: &str, extra: &str| {
            http(
                port,
                &format!("GET {path} HTTP/1.1\r\nHost: x\r\nConnection: close\r\n{extra}\r\n"),
            )
        };

        let (c, _, body) = get("/", "");
        assert_eq!(c, 200);
        assert!(String::from_utf8_lossy(&body).contains("PIN Required"));

        let (c, h, _) = http(
            port,
            "POST /login HTTP/1.1\r\nHost: x\r\nConnection: close\r\nContent-Length: 8\r\n\r\npin=4321",
        );
        assert_eq!(c, 303);
        assert!(h.contains("sfo_pin=4321"));

        let cookie = "Cookie: sfo_pin=4321\r\n";
        let (c, _, body) = get("/", cookie);
        let html = String::from_utf8_lossy(&body).into_owned();
        assert_eq!(c, 200);
        assert!(
            html.contains("R&amp;D &#39;q&#39;.txt")
                && !html.contains("R&D")
                && !html.contains(".hidden")
        );

        let (c, _, body) = get("/download/R%26D%20%27q%27.txt", cookie);
        assert_eq!((c, body.as_slice()), (200, &b"0123456789"[..]));
        let (c, h, body) = get(
            "/download/R%26D%20%27q%27.txt?pin=4321",
            "Range: bytes=2-4\r\n",
        );
        assert_eq!((c, body.as_slice()), (206, &b"234"[..]));
        assert!(h.contains("bytes 2-4/10"));
        assert_eq!(
            get(
                "/download/R%26D%20%27q%27.txt",
                &format!("{cookie}Range: bytes=50-\r\n")
            )
            .0,
            416
        );
        assert_eq!(get("/download/../sfo_outside_lan.txt", cookie).0, 404);

        let part = "--BND\r\nContent-Disposition: form-data; name=\"files\"; filename=\"../evil.txt\"\r\n\r\nPAYLOAD\r\n--BND--\r\n";
        let req = format!(
            "POST /upload HTTP/1.1\r\nHost: x\r\nConnection: close\r\n{cookie}Content-Type: multipart/form-data; boundary=BND\r\nContent-Length: {}\r\n\r\n{part}",
            part.len()
        );
        let (c, h, _) = http(port, &req);
        assert_eq!(c, 303);
        assert!(h.contains("uploaded=1"));
        assert_eq!(
            fs::read_to_string(d.path().join("SFO_Incoming/evil.txt")).unwrap(),
            "PAYLOAD"
        );
        assert_eq!(
            http(
                port,
                "POST /upload HTTP/1.1\r\nHost: x\r\nConnection: close\r\nContent-Length: 0\r\n\r\n"
            )
            .0,
            403
        );

        assert!(srv.stop());
        assert!(!srv.is_running());
        let _ = fs::remove_file(d.path().parent().unwrap().join("sfo_outside_lan.txt"));
    }

    #[test]
    fn helpers() {
        assert_eq!(percent_decode("a%20b+c%2"), "a b c%2");
        assert_eq!(
            query_param("x=1&pin=12%2034", "pin").as_deref(),
            Some("12 34")
        );
        assert!(ct_eq(b"abc", b"abc") && !ct_eq(b"abc", b"abd") && !ct_eq(b"a", b"ab"));
        assert!(!local_ip().is_empty());
    }
}
