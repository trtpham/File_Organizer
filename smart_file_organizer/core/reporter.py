"""
Report generation for organization and deduplication sessions (Console, Markdown, HTML).
"""

from pathlib import Path
from typing import Dict, Any, List
from .models import SessionLog, DuplicateGroup


class Reporter:
    """Creates structured reports for file organization and deduplication sessions."""

    @staticmethod
    def format_bytes(num_bytes: int) -> str:
        """Converts raw byte count into human-readable string (KB, MB, GB)."""
        for unit in ["B", "KB", "MB", "GB", "TB"]:
            if abs(num_bytes) < 1024.0:
                return f"{num_bytes:3.1f} {unit}"
            num_bytes /= 1024.0
        return f"{num_bytes:.1f} PB"

    @classmethod
    def generate_console_summary(cls, session: SessionLog) -> str:
        """Returns concise plain-text console summary."""
        dur = ""
        if session.end_time:
            delta = (session.end_time - session.start_time).total_seconds()
            dur = f" ({delta:.2f}s)"

        lines = [
            "=" * 60,
            f" SMART FILE ORGANIZER - SESSION REPORT{dur}",
            "=" * 60,
            f"Session ID     : {session.session_id}",
            f"Mode           : {'[DRY RUN - SIMULATION]' if session.dry_run else '[LIVE EXECUTION]'}",
            f"Strategy       : {session.strategy.upper()}",
            f"Source Dir     : {session.source_dir}",
            f"Destination Dir: {session.destination_dir}",
            "-" * 60,
            f"Total Processed: {session.total_files} files ({cls.format_bytes(session.total_size_bytes)})",
            f"Moved/Cleaned  : {session.moved_count} files",
            f"Skipped        : {session.skipped_count} files",
            f"Duplicates     : {session.duplicate_count} files",
            f"Space Saved    : {cls.format_bytes(session.space_saved_bytes)}",
            "=" * 60,
        ]
        return "\n".join(lines)

    @classmethod
    def generate_markdown(cls, session: SessionLog) -> str:
        """Generates a GitHub-flavored Markdown report."""
        mode_badge = "🛡️ **DRY-RUN SIMULATION**" if session.dry_run else "⚡ **LIVE EXECUTION**"
        lines = [
            f"# Smart File Organizer Report",
            f"",
            f"- **Session ID:** `{session.session_id}`",
            f"- **Mode:** {mode_badge}",
            f"- **Strategy:** `{session.strategy}`",
            f"- **Source:** `{session.source_dir}`",
            f"- **Destination:** `{session.destination_dir}`",
            f"",
            f"## Summary Metrics",
            f"",
            f"| Metric | Value |",
            f"| :--- | :--- |",
            f"| Total Files Processed | {session.total_files} |",
            f"| Total Size | {cls.format_bytes(session.total_size_bytes)} |",
            f"| Moved / Organized | {session.moved_count} |",
            f"| Skipped Files | {session.skipped_count} |",
            f"| Duplicates Resolved | {session.duplicate_count} |",
            f"| Space Reclaimed | {cls.format_bytes(session.space_saved_bytes)} |",
            f"",
            f"## Action Details",
            f"",
            f"| Action | Source File | Target / Status | Message |",
            f"| :--- | :--- | :--- | :--- |",
        ]

        for r in session.records[:100]:  # Limit markdown table to top 100 for readability
            src = Path(r.source_path).name
            dst = Path(r.destination_path).name if r.destination_path else "-"
            lines.append(f"| {r.action_type.value} | `{src}` | `{dst}` ({r.status}) | {r.message} |")

        if len(session.records) > 100:
            lines.append(f"| ... | *and {len(session.records) - 100} more items* | | |")

        return "\n".join(lines)

    @classmethod
    def generate_html(cls, session: SessionLog, output_path: Path) -> Path:
        """Generates a standalone, beautiful HTML dashboard report."""
        out = Path(output_path)
        out.parent.mkdir(parents=True, exist_ok=True)

        rows_html = []
        for r in session.records:
            badge_class = "badge-success" if r.status == "SUCCESS" else ("badge-warning" if r.status == "SKIPPED" else "badge-danger")
            src = Path(r.source_path).name
            dst = str(r.destination_path) if r.destination_path else "-"
            size_fmt = cls.format_bytes(r.file_size)
            rows_html.append(f"""
                <tr>
                    <td><span class="badge {badge_class}">{r.action_type.value}</span></td>
                    <td title="{r.source_path}"><strong>{src}</strong></td>
                    <td title="{dst}">{dst}</td>
                    <td>{size_fmt}</td>
                    <td>{r.message}</td>
                </tr>
            """)

        html_content = f"""<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>SFO Report - {session.session_id}</title>
    <style>
        :root {{
            --bg: #0f172a;
            --card-bg: #1e293b;
            --text: #f8fafc;
            --text-dim: #94a3b8;
            --accent: #38bdf8;
            --success: #22c55e;
            --warning: #f59e0b;
            --danger: #ef4444;
            --border: #334155;
        }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
            background-color: var(--bg);
            color: var(--text);
            margin: 0;
            padding: 2rem;
        }}
        .container {{ max-width: 1200px; margin: 0 auto; }}
        header {{ margin-bottom: 2rem; }}
        h1 {{ margin: 0 0 0.5rem 0; font-size: 2rem; color: var(--accent); }}
        .meta {{ color: var(--text-dim); font-size: 0.95rem; }}
        .grid {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
            gap: 1.5rem;
            margin-bottom: 2rem;
        }}
        .card {{
            background: var(--card-bg);
            border: 1px solid var(--border);
            border-radius: 12px;
            padding: 1.5rem;
        }}
        .card .title {{ font-size: 0.85rem; color: var(--text-dim); text-transform: uppercase; }}
        .card .val {{ font-size: 1.8rem; font-weight: bold; margin-top: 0.5rem; color: #fff; }}
        table {{
            width: 100%;
            border-collapse: collapse;
            background: var(--card-bg);
            border-radius: 12px;
            overflow: hidden;
            border: 1px solid var(--border);
        }}
        th, td {{ padding: 12px 16px; text-align: left; border-bottom: 1px solid var(--border); }}
        th {{ background: #172033; color: var(--text-dim); font-size: 0.85rem; }}
        tr:hover {{ background: #26334d; }}
        .badge {{ padding: 4px 8px; border-radius: 6px; font-size: 0.75rem; font-weight: bold; }}
        .badge-success {{ background: rgba(34, 197, 94, 0.2); color: var(--success); }}
        .badge-warning {{ background: rgba(245, 158, 11, 0.2); color: var(--warning); }}
        .badge-danger {{ background: rgba(239, 68, 68, 0.2); color: var(--danger); }}
    </style>
</head>
<body>
    <div class="container">
        <header>
            <h1>Smart File Organizer Report</h1>
            <div class="meta">Session: <code>{session.session_id}</code> &bull; Mode: {'DRY RUN' if session.dry_run else 'LIVE'} &bull; Strategy: {session.strategy.upper()}</div>
        </header>

        <div class="grid">
            <div class="card">
                <div class="title">Total Files</div>
                <div class="val">{session.total_files}</div>
            </div>
            <div class="card">
                <div class="title">Moved / Cleaned</div>
                <div class="val" style="color: var(--accent);">{session.moved_count}</div>
            </div>
            <div class="card">
                <div class="title">Duplicates</div>
                <div class="val" style="color: var(--warning);">{session.duplicate_count}</div>
            </div>
            <div class="card">
                <div class="title">Space Saved</div>
                <div class="val" style="color: var(--success);">{cls.format_bytes(session.space_saved_bytes)}</div>
            </div>
        </div>

        <h2>Detailed Action Log</h2>
        <table>
            <thead>
                <tr>
                    <th>Action</th>
                    <th>Source File</th>
                    <th>Destination Path</th>
                    <th>Size</th>
                    <th>Message</th>
                </tr>
            </thead>
            <tbody>
                {''.join(rows_html)}
            </tbody>
        </table>
    </div>
</body>
</html>
"""
        with open(out, "w", encoding="utf-8") as f:
            f.write(html_content)
        return out
