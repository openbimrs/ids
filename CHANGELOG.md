# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.2.0] - 2026-10-03

### Added

- `audit` feature: `audit(&Ids) -> Vec<AuditFinding>` checks a document
  against the IFC schemas of each specification's listed `ifcVersion`s.
  Every finding has a stable `AuditCode` (`as_str`, e.g. `entity-unknown`), a
  `Severity`, the specification index, a facet path and, where it depends on
  the release, the `IfcVersion`. Checks: entity names (upper case, defined by
  the release, IFC2X3 occurrence/type mapping honoured, patterns match
  something), predefined types only where the entity or its type has one,
  requirement entities the applicability can select, explicit (not derived
  or inverse) attributes, values only on single-valued attributes, values
  castable to the attribute type or property `dataType`, patterns only on
  strings, `dataType`s that are IFC defined or enumeration types, standard
  `Pset_`/`Qto_` set membership and data types, `partOf` wholes the relation
  allows, prohibited specifications without requirements, and contradictory
  occurrence or restriction bounds. XSD patterns are evaluated with the
  `regex` crate; constructs it cannot express are reported as
  `pattern-unverified` warnings.
- Corpus test `corpus_audit`: every buildingSMART `invalid-` case reports its
  expected codes and every `pass-`/`fail-` case audits clean.
- `audit_dir` example printing the findings for a directory of `.ids` files.

### Changed

- Minimum supported Rust version is now 1.88 (required by `ifc-schema`).
- The `audit` feature depends on AGPL-3.0-or-later crates (template data also
  CC BY-ND 4.0); the crate remains MIT. See `LICENSING.md`.

## [0.1.4] - 2026-09-30

### Added

- IDS 1.0 writer: `write::to_string` and `write::to_writer` (re-exported at
  the crate root) write the typed model as IDS 1.0 with the IDS, `xs` and
  `xsi` namespaces and the 1.0 `xsi:schemaLocation`, in the element order
  `ids.xsd` requires. Values are written as `<simpleValue>` or
  `<xs:restriction>` exactly as the model holds them, requirement facets
  always state `@cardinality`, and optional elements are omitted when
  `None`. Text is escaped so that whitespace, `\r` and markup characters
  read back unchanged.
- The writer refuses, with a typed `WriteError` naming the model path, what
  IDS 1.0 cannot express or the schema rejects: a non-1.0 or conflicting
  version, no specifications, no IFC release, an applicability without
  facets or out of schema order, an `<xs:restriction>` without facets or with
  an unknown base, an `entity` requirement that is not required, an optional
  `partOf`, `@uri` on a facet that cannot carry it, an invalid `@dataType`,
  `author` or `date`, and characters XML 1.0 cannot represent. Nothing is
  written to the output of `to_writer` when a model is refused.
- Producer constructors: `Ids::new` (declares IDS 1.0; no detection evidence
  needed), `Info::new`, `Specification::new` (optional applicability),
  `Specification::require`, `Applicability::new` and `set_occurrence`,
  `Requirement::new` and `with_occurrence`, `Entity::new`, `PartOf::new`,
  `Classification::new`, `Attribute::new`, `Property::new`,
  `Material::default`, `Restriction::new`, `enumeration` and `pattern`, and
  `From` conversions into `Value`, `Facet` and `Requirement`.
- Corpus tests: every buildingSMART test case satisfies
  `read(write(read(x))) == read(x)`, writing is a fixed point, and the
  written documents validate against the official `ids.xsd` through
  `scripts/validate-ids.py` (`xmlschema`), with a negative control proving
  the validator rejects.

## [0.1.3] - 2026-09-28

### Added

- IDS 1.0 reader: `read::from_str` and `read::from_slice` return the typed
  `Ids` model (`model` module) or a `ReadError` with line and column. The
  reader follows `ids.xsd` 1.0.0 strictly: element order, required elements
  and attributes, enumerated tokens (`@cardinality`, `@relation`,
  `@ifcVersion`, `@dataType`), `xs:date` and the `author` pattern. Schema
  defaults are applied explicitly: an absent `@cardinality` is required and an
  absent `minOccurs`/`maxOccurs` is `1`. Requirement facets keep document
  order, `@ifcVersion` is split as the `xs:list` it is, and
  `IFCRELVOIDSELEMENT IFCRELFILLSELEMENT` stays one token.
- Version detection wired into reading: the first pass collects `Signal`s and
  the revision `xsi:schemaLocation` declares, and refuses a pre-1.0 draft or a
  declaration that contradicts the shape with `ErrorKind::UnsupportedVersion`
  carrying the evidence. A document that declares nothing and shows no
  revision-specific shape is read as 1.0 only when it satisfies the 1.0
  schema, and reports `Detected::Inferred`.
- `<xs:restriction>` values keep `base` (prefix resolved) and every facet in
  its lexical form; `xs:whiteSpace`, inline `xs:simpleType` and
  `xs:assertion` are refused as unsupported rather than dropped.
- Corpus test (`cargo test -- --ignored corpus`, needs `IDS_TEST_CASES`): all
  334 buildingSMART test cases read as declared IDS 1.0. The corpus is CC
  BY-ND 4.0 and is not vendored.

## [0.1.2] - 2026-09-21

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

[Unreleased]: https://github.com/openbimrs/ids/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/openbimrs/ids/releases/tag/v0.2.0
[0.1.4]: https://github.com/openbimrs/ids/releases/tag/v0.1.4
[0.1.3]: https://github.com/openbimrs/ids/releases/tag/v0.1.3
[0.1.2]: https://github.com/openbimrs/ids/releases/tag/v0.1.2
[0.1.1]: https://github.com/openbimrs/ids/releases/tag/v0.1.1
[0.1.0]: https://crates.io/crates/openbim-ids/0.1.0
