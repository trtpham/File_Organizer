//! `sfo` — Smart File Organizer command-line interface.

mod args;
mod out;
mod run;

use clap::Parser;
use std::process::ExitCode;

fn main() -> ExitCode {
    // Like Python's main.py: no arguments at all opens the desktop GUI,
    // falling back to help when the GUI is not installed.
    if std::env::args_os().len() == 1 {
        if run::launch_gui(None) == 0 {
            return ExitCode::SUCCESS;
        }
        let _ = <args::Args as clap::CommandFactory>::command().print_help();
        return ExitCode::from(1);
    }
    ExitCode::from(run::run(args::Args::parse()))
}
