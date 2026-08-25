# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- Updated CI checkout to `actions/checkout@v7`, using the supported Node 24
  runtime and current fork-safety behavior.
- Extracted the IDS family into its canonical standalone repository while
  preserving its OpenBIM.rs history.
- Made package and dependency metadata independent of the integration workspace.
- Added standalone documentation, CI, and package verification.

## [0.1.0] - 2026-08-24

### Added

- Reserved the `openbim-ids` crate name.
- Added the IDS namespace, published-version model, and approved-version tests.

[Unreleased]: https://github.com/openbimrs/ids/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/openbimrs/ids/releases/tag/v0.1.0
