# IDS repository instructions

This repository owns the OpenBIM.rs implementation of buildingSMART IDS. The
IDS 1.0 reader, writer and document audit are implemented and corpus-verified;
do not describe checking IFC models as implemented without executable
conformance evidence.

## Map

- `openbim-ids/` — canonical published crate; read its `AGENTS.md` before editing
- `docs/` — repository architecture and maintained documentation
- `scripts/gate.sh` — complete local/CI verification gate
- `CHANGELOG.md` — user-visible changes using Keep a Changelog

## Commands

```bash
./scripts/gate.sh
cargo test --workspace
cargo package -p openbim-ids
```

Trust command exit codes. Never summarize a Cargo pipeline in a way that hides
the Cargo process status.

## Boundaries

- IDS may depend on released `openbim-core` contracts and public IFC
  contracts. IFC crates are AGPL-3.0-or-later: keep them behind optional
  features (`audit-schema`, `audit`) so the default crate stays MIT-only, and
  keep the CC BY-ND template catalog out of `audit-schema`.
- IFC and lower-level codec/core crates must never depend on IDS.
- Do not vendor ISO, CEN, or other schema files without verified redistribution
  rights.
- Release-critical package metadata and cross-repository dependency versions
  are explicit in crate manifests; do not replace them with parent-workspace
  inheritance.

## Documentation discipline

Keep capability tables honest: distinguish reserved API, implemented algorithm,
and conformance-tested behavior. Update README, rustdoc, and CHANGELOG together
for user-visible changes.
