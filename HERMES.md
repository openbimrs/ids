# OpenBIM.rs IDS

Canonical repository: <https://github.com/openbimrs/ids>
Integration repository: <https://github.com/openbimrs/openbim>

Read `AGENTS.md` before changing the repository and the nested `AGENTS.md`
before editing a crate. Keep the crate independently buildable; the parent
OpenBIM.rs workspace pins this repository as a submodule but is not required for
standalone development.

## Verification

Run `./scripts/gate.sh`. It is the authoritative local and CI gate and decides
success from command exit codes.

## Project conventions

- Rust 2021, MSRV 1.85, MIT.
- Pure Rust; forbid unsafe code unless a future ADR establishes a narrowly
  reviewed exception.
- Maintain explicit dependency direction: IDS consumes core/IFC, never reverse.
- Do not vendor standards schemas without confirmed redistribution rights.
- Use Keep a Changelog and document implemented versus reserved capabilities.
