#!/usr/bin/env python3
"""
Automated Standalone Windows Executable Build Script.
Compiles Smart File Organizer into a single-file portable sfo.exe using PyInstaller,
bundles release assets, and packages into a distributable ZIP archive.
"""

import os
import sys
import shutil
import zipfile
import subprocess
from pathlib import Path


def build_executable() -> bool:
    root_dir = Path(__file__).resolve().parent.parent
    dist_dir = root_dir / "dist"
    build_dir = root_dir / "build"
    main_py = root_dir / "main.py"
    config_json = root_dir / "config" / "default_config.json"

    print("=" * 70)
    print("  SMART FILE ORGANIZER (SFO) v3.0 - STANDALONE BUILD PIPELINE")
    print("=" * 70)

    # 1. Clean previous build artifacts
    print("[*] Cleaning previous build artifacts...")
    for p in [build_dir, dist_dir]:
        if p.exists():
            shutil.rmtree(p, ignore_errors=True)

    # 2. Assemble PyInstaller arguments
    # On Windows, path separator for --add-data is ';'
    sep = ";" if os.name == "nt" else ":"
    data_arg = f"{config_json}{sep}config"

    cmd = [
        sys.executable, "-m", "PyInstaller",
        "--name", "sfo",
        "--onefile",
        "--clean",
        "--noconfirm",
        "--add-data", data_arg,
        "--hidden-import", "smart_file_organizer",
        "--hidden-import", "smart_file_organizer.core",
        "--hidden-import", "smart_file_organizer.cli",
        "--hidden-import", "smart_file_organizer.gui",
        str(main_py)
    ]

    print(f"[*] Compiling executable with PyInstaller...")
    print(f"    Command: {' '.join(cmd)}")
    
    result = subprocess.run(cmd, cwd=root_dir)
    if result.returncode != 0:
        print("[!] PyInstaller build failed!")
        return False

    exe_path = dist_dir / ("sfo.exe" if os.name == "nt" else "sfo")
    if not exe_path.exists():
        print(f"[!] Target binary not found at {exe_path}")
        return False

    exe_size_mb = exe_path.stat().st_size / (1024 * 1024)
    print(f"[+] Successfully generated standalone binary: {exe_path} ({exe_size_mb:.1f} MB)")

    # 3. Create Release Package
    release_dir = dist_dir / "SmartFileOrganizer_v3.0_Portable"
    release_dir.mkdir(parents=True, exist_ok=True)

    # Copy files
    shutil.copy2(exe_path, release_dir / exe_path.name)
    shutil.copy2(root_dir / "README.md", release_dir / "README.md")
    shutil.copy2(root_dir / "LICENSE", release_dir / "LICENSE")
    
    config_dir = release_dir / "config"
    config_dir.mkdir(exist_ok=True)
    shutil.copy2(config_json, config_dir / "default_config.json")

    # Quick start script for Windows
    bat_content = """@echo off
title Smart File Organizer v3.0
echo Starting Smart File Organizer Desktop GUI...
start "" "%~dp0sfo.exe" --gui
"""
    with open(release_dir / "Launch_GUI.bat", "w", encoding="utf-8") as f:
        f.write(bat_content)

    # 4. Package into ZIP
    zip_path = dist_dir / "SmartFileOrganizer_v3.0_Windows_x64.zip"
    print(f"[*] Packaging release into ZIP archive: {zip_path.name}...")
    with zipfile.ZipFile(zip_path, "w", zipfile.ZIP_DEFLATED) as zf:
        for file in release_dir.rglob("*"):
            if file.is_file():
                zf.write(file, arcname=file.relative_to(dist_dir).as_posix())

    zip_size_mb = zip_path.stat().st_size / (1024 * 1024)
    print(f"[+] Release package created: {zip_path} ({zip_size_mb:.1f} MB)")
    print("=" * 70)
    print("  BUILD COMPLETE & READY FOR COMMERCIAL DISTRIBUTION!")
    print("=" * 70)
    return True


if __name__ == "__main__":
    success = build_executable()
    sys.exit(0 if success else 1)
