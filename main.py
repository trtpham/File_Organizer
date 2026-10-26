#!/usr/bin/env python3
"""
Smart File Organizer (SFO)
Main unified entrypoint for CLI and Desktop GUI.
"""

import sys


def main() -> None:
    # If no CLI arguments provided, launch Desktop GUI
    if len(sys.argv) == 1:
        try:
            from smart_file_organizer.gui.app import launch_gui
            launch_gui()
            sys.exit(0)
        except Exception as e:
            print(f"Failed to start GUI ({e}). Falling back to CLI help...")
            from smart_file_organizer.cli.app import run_cli
            sys.exit(run_cli(["--help"]))
    else:
        from smart_file_organizer.cli.app import run_cli
        sys.exit(run_cli())


if __name__ == "__main__":
    main()
