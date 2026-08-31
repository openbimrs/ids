# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- Relicensed repository-authored work from MIT to `AGPL-3.0-or-later`; historical releases remain under their published MIT terms, and third-party material retains its own terms.
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

[Unreleased]: https://github.com/openbimrs/ids/commits/main
[0.1.0]: https://crates.io/crates/openbim-ids/0.1.0
