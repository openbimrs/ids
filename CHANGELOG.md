# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- Relicensed repository-authored work from `AGPL-3.0-or-later` back to `MIT`.
  `0.1.1` was published under the AGPL and stays that way: a crates.io version
  cannot be relicensed in place. `0.1.2` onwards is MIT, restoring the terms of
  `0.1.0`. `LICENSING.md` records the per-version boundaries.

### Added

- `Occurrence`, modelling IDS 0.9 `xs:occurs` and IDS 1.0 `@cardinality` as one
  type so the same intent expressed in either revision produces equal typed
  output. The schema default of *required* is explicit, since reading an absent
  `@cardinality` as optional silently drops requirements.
- `VersionSignals`, `Signal` and `detect_version`, resolving a document's
  revision into `openbim_core::Detected`. A document declaring 1.0 while
  carrying `minOccurs` on a requirement facet is reported as a conflict instead
  of being read as 1.0 with its occurrence constraints dropped. Detection
  consumes observations rather than XML, so it is testable before a reader
  exists.

## [0.1.1] - 2026-09-21

### Added

- `IdsVersion::as_str`, `Display` and `FromStr`, so a version round-trips
  through text instead of needing a `match` at every call site. `FromStr` also
  accepts the three-component spelling the schema files carry in their own
  `@version` attribute (`1.0.0`), which is what a version read straight out of
  `ids.xsd` looks like.
- `IdsVersion::ALL`, so callers can iterate published versions without
  hand-listing variants.

### Fixed

- Pointed published package metadata at `openbimrs/ids`. The `0.1.0` release
  was published from the integration workspace and its `repository` field sent
  anyone arriving from crates.io or docs.rs to `openbimrs/openbim` instead.
- Declared `homepage` and `documentation`, which were absent from `0.1.0`.

### Changed

- Relicensed repository-authored work from MIT to `AGPL-3.0-or-later`. The
  `0.1.0` release remains under its published MIT terms, so `0.1.1` is not a
  drop-in upgrade: consumers must accept the copyleft terms or stay on `0.1.0`.
  Third-party material retains its own terms.
- Updated CI checkout to `actions/checkout@v7`, using the supported Node 24
  runtime and current fork-safety behavior.
- Made packaged README links archive-safe and linked the historical `0.1.0`
  release to its crates.io artifact rather than a nonexistent Git tag.
- Extracted the IDS family into its canonical standalone repository while
  preserving its OpenBIM.rs history.
- Made package and dependency metadata independent of the integration workspace.
- Added standalone documentation, CI, and package verification.

## [0.1.0] - 2026-08-24

### Added

- Reserved the `openbim-ids` crate name.
- Added the IDS namespace, published-version model, and approved-version tests.

[Unreleased]: https://github.com/openbimrs/ids/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/openbimrs/ids/releases/tag/v0.1.1
[0.1.0]: https://crates.io/crates/openbim-ids/0.1.0
