# OpenBIM.rs IDS

[![CI](https://github.com/openbimrs/ids/actions/workflows/ci.yml/badge.svg)](https://github.com/openbimrs/ids/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/openbim-ids.svg)](https://crates.io/crates/openbim-ids)
[![docs.rs](https://docs.rs/openbim-ids/badge.svg)](https://docs.rs/openbim-ids)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-blue)](https://www.rust-lang.org)

Pure-Rust infrastructure for the buildingSMART Information Delivery
Specification (IDS): the machine-readable way to state what an IFC model must
contain and audit whether it does.

This repository is the canonical home of the IDS family in
[OpenBIM.rs](https://github.com/openbimrs/openbim). The integration repository
pins this repository under `packages/ids`.

## Status

The published `0.1.0` release is a **reserved scaffold**, not an IDS parser or
validator. It establishes stable crate ownership and the version/reporting
contracts needed by the implementation.

| Capability | Status |
| --- | --- |
| Shared IDS XML namespace constant | Implemented |
| Published-version model and approved-version test | Implemented |
| IDS XML parsing | Not implemented |
| IDS XML writing | Not implemented |
| IFC applicability and requirement auditing | Not implemented |
| buildingSMART pass/fail corpus conformance | Not implemented |

No parser, writer, or validation capability should be inferred from the crate
existing on crates.io.

## Crates

| Crate | Purpose |
| --- | --- |
| [`openbim-ids`](openbim-ids/) | Canonical IDS types and, in future releases, parsing and auditing |

## Install

```bash
cargo add openbim-ids
```

```rust
use openbim_ids::{IdsVersion, NAMESPACE};

assert_eq!(NAMESPACE, "http://standards.buildingsmart.org/IDS");
assert!(IdsVersion::CURRENT.is_approved());
```

## Version handling

Published IDS files share an XML namespace across multiple schema revisions.
The namespace identifies IDS but does not prove a schema version. Future parsing
must therefore return version evidence and surface disagreements rather than
silently guessing.

Only IDS `1.0` is represented as the approved version for new documents. Older
drafts remain relevant for reading existing files.

## Architecture

- [`docs/architecture.md`](docs/architecture.md) — repository and dependency boundaries
- [`openbimrs/openbim`](https://github.com/openbimrs/openbim) — integrated workspace and facade
- [`openbim-core`](https://crates.io/crates/openbim-core) — shared openBIM vocabulary

IDS may consume shared openBIM and IFC contracts. IFC must never depend on IDS.
That one-way dependency prevents the IFC layer from accumulating every standard
that consumes IFC.

## Standards material

No ISO, CEN, or other restricted schema artifact is vendored here. Source types
may be implemented from legitimately accessed specifications, while conformance
fixtures must have redistribution terms compatible with this repository.

## Development

Requires Rust `1.85` or newer.

```bash
git clone https://github.com/openbimrs/ids.git
cd ids
./scripts/gate.sh
```

The gate checks formatting, build, tests, Clippy, rustdoc, and crates.io package
verification using command exit codes.

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md). Capability work must add executable
conformance evidence and update the status table without overstating coverage.

## License

AGPL-3.0-or-later — see [`LICENSE`](LICENSE).
