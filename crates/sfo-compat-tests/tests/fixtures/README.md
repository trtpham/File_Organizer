# Compatibility fixtures (frozen)

These files were written by the last Python release (v3.0, commit
`c0c68c96b01147de0655291d6342f765c156152f`) and are kept as golden data: the
Rust build must keep reading history, journals, audit logs, vaults and
archives that users created with v3.0.

The Python code and its generator script (`scripts/gen_compat_fixtures.py`)
were removed in v4.0. To regenerate the fixtures, check out a commit before
the removal and run `python scripts/gen_compat_fixtures.py` there. Do not edit
these files by hand.

The fixtures were generated on Windows; tests adapt the recorded paths (and
case-insensitive `fnmatch` results) when run on Linux/macOS.
