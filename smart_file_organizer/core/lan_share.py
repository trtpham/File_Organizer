"""
Cross-Device LAN File Transfer & Sharing Server.
Enables instant file sharing between PC, phones, and laptops on the same Wi-Fi network.
Enhanced with multi-threaded concurrent connections (ThreadingHTTPServer),
HTTP Range streaming/resumable downloads, and mobile-responsive drag-and-drop web UI.
Zero external dependencies (Python Standard Library http.server + socket).
"""

import os
import re
import socket
import threading
import urllib.parse
from pathlib import Path
from http.server import ThreadingHTTPServer, BaseHTTPRequestHandler
from typing import Optional, Dict, Any, List


def get_local_ip() -> str:
    """Discovers the primary local network IP address."""
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    try:
        # Does not actually establish connection to external server
        s.connect(("8.8.8.8", 80))
        ip = s.getsockname()[0]
    except Exception:
        ip = "127.0.0.1"
    finally:
        s.close()
    return ip


class LANShareHandler(BaseHTTPRequestHandler):
    """HTTP request handler providing multi-threaded file download, upload, and dashboard."""

    shared_dir: Path = Path.cwd()
    pin: Optional[str] = None

    def log_message(self, format: str, *args: Any) -> None:
        """Suppresses default console noise."""
        pass

    def _is_authenticated(self) -> bool:
        """Checks PIN in cookie or query parameter."""
        if not self.pin:
            return True
        # Check cookie
        cookie_header = self.headers.get("Cookie", "")
        if f"sfo_pin={self.pin}" in cookie_header:
            return True
        # Check query string
        parsed = urllib.parse.urlparse(self.path)
        params = urllib.parse.parse_qs(parsed.query)
        return params.get("pin", [""])[0] == self.pin

    def do_GET(self) -> None:
        parsed = urllib.parse.urlparse(self.path)
        path = urllib.parse.unquote(parsed.path)

        if not self._is_authenticated():
            self._render_login()
            return

        # File download request
        if path.startswith("/download/"):
            rel_path = path[len("/download/"):]
            target = (self.shared_dir / rel_path).resolve()
            # Prevent path traversal
            if self.shared_dir.resolve() in target.parents or target == self.shared_dir:
                if target.is_file():
                    self._serve_file(target)
                    return
            self.send_error(404, "File Not Found")
            return

        # Dashboard
        self._render_dashboard()

    def do_POST(self) -> None:
        parsed = urllib.parse.urlparse(self.path)
        if parsed.path == "/login":
            content_length = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(content_length).decode("utf-8", errors="ignore")
            params = urllib.parse.parse_qs(body)
            entered_pin = params.get("pin", [""])[0]

            if entered_pin == self.pin:
                self.send_response(303)
                self.send_header("Location", "/")
                self.send_header("Set-Cookie", f"sfo_pin={self.pin}; Path=/; HttpOnly")
                self.end_headers()
            else:
                self._render_login(error="Invalid PIN. Please try again.")
            return

        if not self._is_authenticated():
            self.send_error(403, "Forbidden")
            return

        if parsed.path == "/upload":
            self._handle_upload()
            return

        self.send_error(404, "Not Found")

    def _handle_upload(self) -> None:
        """Handles multipart/form-data upload in memory-safe streaming fashion."""
        content_type = self.headers.get("Content-Type", "")
        if not content_type.startswith("multipart/form-data"):
            self.send_error(400, "Bad Request: Expected multipart/form-data")
            return

        boundary_match = re.search(r'boundary=([^;]+)', content_type)
        if not boundary_match:
            self.send_error(400, "Bad Request: No boundary found")
            return

        boundary = boundary_match.group(1).strip().encode("utf-8")
        content_length = int(self.headers.get("Content-Length", 0))
        raw_body = self.rfile.read(content_length)

        incoming_dir = self.shared_dir / "SFO_Incoming"
        incoming_dir.mkdir(parents=True, exist_ok=True)

        parts = raw_body.split(b"--" + boundary)
        saved_files = []

        for part in parts:
            if not part or part == b"--\r\n" or part == b"--":
                continue

            header_end = part.find(b"\r\n\r\n")
            if header_end == -1:
                continue

            header_bytes = part[:header_end]
            file_bytes = part[header_end + 4:].rstrip(b"\r\n")

            header_str = header_bytes.decode("latin-1", errors="ignore")
            fn_match = re.search(r'filename="([^"]+)"', header_str)
            if fn_match:
                filename = Path(fn_match.group(1)).name
                if filename:
                    dest_file = incoming_dir / filename
                    with open(dest_file, "wb") as out:
                        out.write(file_bytes)
                    saved_files.append(filename)

        self.send_response(303)
        self.send_header("Location", "/?uploaded=" + str(len(saved_files)))
        self.end_headers()

    def _serve_file(self, target: Path) -> None:
        """Streams file with HTTP Range support for resumable downloads and media streaming."""
        try:
            size = target.stat().st_size
            range_header = self.headers.get("Range")

            if range_header:
                range_match = re.search(r"bytes=(\d+)-(\d+)?", range_header)
                if range_match:
                    start = int(range_match.group(1))
                    end = int(range_match.group(2)) if range_match.group(2) else size - 1
                    content_len = end - start + 1
                    self.send_response(206)  # Partial Content
                    self.send_header("Content-Type", "application/octet-stream")
                    self.send_header("Content-Disposition", f'attachment; filename="{target.name}"')
                    self.send_header("Content-Range", f"bytes {start}-{end}/{size}")
                    self.send_header("Content-Length", str(content_len))
                    self.send_header("Accept-Ranges", "bytes")
                    self.end_headers()

                    with open(target, "rb") as f:
                        f.seek(start)
                        remaining = content_len
                        while remaining > 0:
                            chunk_size = min(65536, remaining)
                            chunk = f.read(chunk_size)
                            if not chunk:
                                break
                            self.wfile.write(chunk)
                            remaining -= len(chunk)
                    return

            self.send_response(200)
            self.send_header("Content-Type", "application/octet-stream")
            self.send_header("Content-Disposition", f'attachment; filename="{target.name}"')
            self.send_header("Content-Length", str(size))
            self.send_header("Accept-Ranges", "bytes")
            self.end_headers()

            with open(target, "rb") as f:
                while chunk := f.read(65536):
                    self.wfile.write(chunk)
        except Exception:
            self.send_error(500, "Failed to stream file")

    def _render_login(self, error: Optional[str] = None) -> None:
        error_html = f'<p style="color: #ef4444; font-weight: bold;">{error}</p>' if error else ""
        html = f"""<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>SFO LAN Drop - PIN Required</title>
    <style>
        body {{ background: #0b0f19; color: #f8fafc; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; display: flex; align-items: center; justify-content: center; height: 100vh; margin: 0; }}
        .card {{ background: #131d31; padding: 32px; border-radius: 12px; border: 1px solid #1e293b; width: 320px; text-align: center; box-shadow: 0 10px 25px rgba(0,0,0,0.5); }}
        input {{ width: 100%; box-sizing: border-box; padding: 12px; font-size: 18px; text-align: center; border-radius: 6px; border: 1px solid #334155; background: #0f172a; color: #fff; margin: 16px 0; }}
        button {{ width: 100%; padding: 12px; background: #3b82f6; color: #fff; border: none; border-radius: 6px; font-size: 16px; font-weight: bold; cursor: pointer; }}
    </style>
</head>
<body>
    <div class="card">
        <h2 style="margin-top: 0;">🔐 SFO LAN Drop</h2>
        <p style="color: #94a3b8; font-size: 14px;">Enter the 4-digit PIN displayed on your PC to access files:</p>
        {error_html}
        <form method="POST" action="/login">
            <input type="password" name="pin" maxlength="8" placeholder="••••" required autofocus>
            <button type="submit">Unlock Files</button>
        </form>
    </div>
</body>
</html>"""
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.end_headers()
        self.wfile.write(html.encode("utf-8"))

    def _render_dashboard(self) -> None:
        files = []
        try:
            for item in sorted(self.shared_dir.iterdir()):
                if item.is_file() and not item.name.startswith("."):
                    size_mb = item.stat().st_size / (1024 * 1024)
                    size_str = f"{size_mb:.2f} MB" if size_mb >= 1.0 else f"{item.stat().st_size / 1024:.1f} KB"
                    files.append((item.name, size_str))
        except Exception:
            pass

        rows = []
        for name, size in files:
            safe_name = urllib.parse.quote(name)
            rows.append(f"""
            <tr style="border-bottom: 1px solid #1e293b;">
                <td style="padding: 12px 10px; font-weight: 500;">📄 {name}</td>
                <td style="padding: 12px 10px; color: #94a3b8;">{size}</td>
                <td style="padding: 12px 10px; text-align: right;">
                    <a href="/download/{safe_name}" style="background: #3b82f6; color: #fff; text-decoration: none; padding: 6px 12px; border-radius: 6px; font-size: 13px; font-weight: bold;">Download</a>
                </td>
            </tr>
            """)

        files_table = "".join(rows) if rows else '<tr><td colspan="3" style="text-align: center; padding: 24px; color: #64748b;">No files in shared directory.</td></tr>'

        html = f"""<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>SFO Wireless LAN Drop</title>
    <style>
        body {{ background: #0b0f19; color: #f8fafc; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; margin: 0; padding: 20px; }}
        .container {{ max-width: 800px; margin: 0 auto; }}
        .header {{ display: flex; justify-content: space-between; align-items: center; border-bottom: 1px solid #1e293b; padding-bottom: 16px; margin-bottom: 24px; }}
        .card {{ background: #131d31; border: 1px solid #1e293b; border-radius: 12px; padding: 20px; margin-bottom: 24px; box-shadow: 0 4px 6px rgba(0,0,0,0.3); }}
        .upload-zone {{ border: 2px dashed #334155; border-radius: 8px; padding: 24px; text-align: center; background: #0f172a; cursor: pointer; }}
        table {{ width: 100%; border-collapse: collapse; text-align: left; font-size: 14px; }}
        input[type="file"] {{ display: block; margin: 12px auto; color: #94a3b8; }}
        .btn {{ background: #10b981; color: #fff; border: none; padding: 10px 20px; border-radius: 6px; font-size: 15px; font-weight: bold; cursor: pointer; }}
        .btn:hover {{ background: #059669; }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <div>
                <h1 style="margin: 0; font-size: 22px;">📡 Smart File Organizer - LAN Drop</h1>
                <p style="margin: 4px 0 0; color: #94a3b8; font-size: 13px;">Shared folder: {self.shared_dir.name}</p>
            </div>
            <span style="background: #1e293b; color: #38bdf8; padding: 6px 12px; border-radius: 20px; font-size: 12px; font-weight: bold;">LAN Connected (Multi-Threaded)</span>
        </div>

        <div class="card">
            <h3 style="margin-top: 0;">📤 Send Files to PC</h3>
            <form method="POST" action="/upload" enctype="multipart/form-data" class="upload-zone">
                <p style="margin: 0; color: #94a3b8;">Select files to transfer from your phone or device to PC:</p>
                <input type="file" name="files" multiple required>
                <button type="submit" class="btn">🚀 Upload Files to PC</button>
            </form>
        </div>

        <div class="card">
            <h3 style="margin-top: 0;">📥 Available Files to Download ({len(files)})</h3>
            <table>
                <thead>
                    <tr style="color: #64748b; border-bottom: 1px solid #1e293b;">
                        <th style="padding: 10px;">Name</th>
                        <th style="padding: 10px;">Size</th>
                        <th style="padding: 10px; text-align: right;">Action</th>
                    </tr>
                </thead>
                <tbody>
                    {files_table}
                </tbody>
            </table>
        </div>
    </div>
</body>
</html>"""
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.end_headers()
        self.wfile.write(html.encode("utf-8"))


class LANShareServer:
    """Manages the background HTTP daemon for LAN file sharing using ThreadingHTTPServer."""

    def __init__(self, shared_dir: Path, port: int = 8080, pin: Optional[str] = None):
        self.shared_dir = Path(shared_dir).resolve()
        self.port = port
        self.pin = pin
        self.server: Optional[ThreadingHTTPServer] = None
        self.thread: Optional[threading.Thread] = None

    def start(self) -> Dict[str, Any]:
        """Starts the multi-threaded HTTP server on a background daemon thread."""
        if self.is_running():
            return {"success": True, "message": "Server is already running.", "url": self.get_url()}

        class CustomHandler(LANShareHandler):
            pass

        CustomHandler.shared_dir = self.shared_dir
        CustomHandler.pin = self.pin

        try:
            self.server = ThreadingHTTPServer(("0.0.0.0", self.port), CustomHandler)
            self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
            self.thread.start()

            url = self.get_url()
            return {
                "success": True,
                "url": url,
                "port": self.port,
                "pin": self.pin,
                "message": f"LAN Drop server started at {url}"
            }
        except Exception as e:
            return {"success": False, "message": f"Failed to start LAN server: {str(e)}"}

    def stop(self) -> Dict[str, Any]:
        """Stops the running server."""
        if self.server:
            try:
                self.server.shutdown()
                self.server.server_close()
            except Exception:
                pass
            self.server = None
            self.thread = None
            return {"success": True, "message": "LAN Drop server stopped."}
        return {"success": False, "message": "Server is not running."}

    def is_running(self) -> bool:
        """Returns True if the background server is active."""
        return self.server is not None and self.thread is not None and self.thread.is_alive()

    def get_url(self) -> str:
        """Constructs accessible local URL."""
        ip = get_local_ip()
        return f"http://{ip}:{self.port}"
