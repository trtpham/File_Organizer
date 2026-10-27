"""
Cross-Device LAN File Transfer & Sharing Server.
Enables instant file sharing between PC, phones, and laptops on the same Wi-Fi network.
Zero external dependencies (Python Standard Library http.server + socket).
Fully compatible with Python 3.10 through 3.14+ (No deprecated cgi module).
"""

import os
import re
import socket
import threading
import urllib.parse
from pathlib import Path
from http.server import HTTPServer, BaseHTTPRequestHandler
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
    """HTTP request handler providing file download, upload, and dashboard."""

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
        """Handles multipart/form-data upload without using deprecated cgi module."""
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
            if b"\r\n\r\n" not in part:
                continue

            header_bytes, file_bytes = part.split(b"\r\n\r\n", 1)
            # Strip trailing CRLF from file content
            if file_bytes.endswith(b"\r\n"):
                file_bytes = file_bytes[:-2]

            header_str = header_bytes.decode("utf-8", errors="ignore")
            # Extract filename
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
        try:
            size = target.stat().st_size
            self.send_response(200)
            self.send_header("Content-Type", "application/octet-stream")
            self.send_header("Content-Disposition", f'attachment; filename="{target.name}"')
            self.send_header("Content-Length", str(size))
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
        button:hover {{ background: #2563eb; }}
    </style>
</head>
<body>
    <div class="card">
        <h2>🔒 Protected LAN Drop</h2>
        <p style="color: #94a3b8; font-size: 14px;">Enter the 4-digit PIN displayed on your PC screen:</p>
        {error_html}
        <form method="POST" action="/login">
            <input type="password" name="pin" placeholder="••••" maxlength="6" autofocus required>
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
        files: List[Path] = []
        try:
            for p in self.shared_dir.iterdir():
                if p.is_file() and not p.name.startswith("."):
                    files.append(p)
            files.sort(key=lambda x: x.stat().st_mtime, reverse=True)
        except Exception:
            pass

        rows = []
        for f in files:
            sz = f.stat().st_size
            sz_str = f"{sz / (1024*1024):.1f} MB" if sz > 1024*1024 else f"{sz / 1024:.1f} KB"
            dl_url = f"/download/{urllib.parse.quote(f.name)}"
            rows.append(f"""
            <tr>
                <td style="padding: 12px; border-bottom: 1px solid #1e293b;">📄 {f.name}</td>
                <td style="padding: 12px; border-bottom: 1px solid #1e293b; color: #94a3b8;">{sz_str}</td>
                <td style="padding: 12px; border-bottom: 1px solid #1e293b; text-align: right;">
                    <a href="{dl_url}" style="background: #2563eb; color: #fff; text-decoration: none; padding: 6px 14px; border-radius: 4px; font-size: 13px; font-weight: bold;">Download</a>
                </td>
            </tr>
            """)

        files_table = "".join(rows) if rows else "<tr><td colspan='3' style='padding: 24px; text-align: center; color: #64748b;'>No files in shared directory.</td></tr>"

        html = f"""<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>SFO LAN Drop - File Exchange</title>
    <style>
        body {{ background: #0b0f19; color: #f8fafc; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; margin: 0; padding: 24px; }}
        .container {{ max-width: 800px; margin: 0 auto; }}
        .header {{ display: flex; justify-content: space-between; align-items: center; margin-bottom: 24px; border-bottom: 1px solid #1e293b; padding-bottom: 16px; }}
        .card {{ background: #131d31; border-radius: 10px; border: 1px solid #1e293b; padding: 20px; margin-bottom: 24px; box-shadow: 0 4px 12px rgba(0,0,0,0.3); }}
        .upload-zone {{ border: 2px dashed #3b82f6; border-radius: 8px; padding: 24px; text-align: center; background: rgba(59, 130, 246, 0.05); }}
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
            <span style="background: #1e293b; color: #38bdf8; padding: 6px 12px; border-radius: 20px; font-size: 12px; font-weight: bold;">LAN Connected</span>
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
    """Manages the background HTTP daemon for LAN file sharing."""

    def __init__(self, shared_dir: Path, port: int = 8080, pin: Optional[str] = None):
        self.shared_dir = Path(shared_dir).resolve()
        self.port = port
        self.pin = pin
        self.server: Optional[HTTPServer] = None
        self.thread: Optional[threading.Thread] = None

    def start(self) -> Dict[str, Any]:
        """Starts the HTTP server on a background daemon thread."""
        if self.is_running():
            return {"success": True, "message": "Server is already running.", "url": self.get_url()}

        class CustomHandler(LANShareHandler):
            pass

        CustomHandler.shared_dir = self.shared_dir
        CustomHandler.pin = self.pin

        try:
            self.server = HTTPServer(("0.0.0.0", self.port), CustomHandler)
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
