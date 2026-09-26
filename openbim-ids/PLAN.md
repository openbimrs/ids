# openbim-ids implementation plan

Status: namespace, version, occurrence and version-evidence contracts and the
IDS 1.0 reader implemented; writer and auditing not started.
Last updated: 2026-09-26

This is task state, not ambient context. Follow `AGENTS.md`; claim one task ID,
record blockers/decisions under it, and check it off only with executable
evidence.

## Established boundary

IDS consumes `openbim-core` and, when auditing is implemented, public IFC
contracts. IFC, core, and codec must never depend on IDS.

## Implemented scaffold

- `NAMESPACE` for the namespace shared by published IDS revisions
- `IdsVersion`, `CURRENT`, `ALL`, approved-version behavior, and text
  conversion (`as_str` / `Display` / `FromStr`)
- `Occurrence`, modelling the 0.9 `xs:occurs` and 1.0 `@cardinality` spellings
  as one type, with the schema default of *required* made explicit
- `VersionSignals` / `Signal` / `detect_version`, reporting version evidence as
  `openbim_core::Detected` and refusing to resolve a conflict

These are contracts, not a parser or validator. Nothing here reads XML.

## Work queue

- [x] `IDS-VERSION-EVIDENCE` - define version evidence and disagreement reporting
- [x] `IDS-PARSE` - parse IDS without silently guessing the schema version
- [ ] `IDS-WRITE` - write approved IDS 1.0 by default, with explicit legacy opt-in
- [ ] `IDS-AUDIT` - distinguish applicable/pass/fail/not-applicable outcomes
- [ ] `IDS-CORPUS` - verify buildingSMART pass/fail fixtures with licensed inputs

### `IDS-PARSE` notes

Carried from the issue thread, to be honoured when the reader is written:

- `elementFormDefault="qualified"`, `attributeFormDefault="unqualified"` —
  elements are namespaced, attributes never are.
- `<xs:restriction>` is `ref`'d from the XML Schema namespace, so a value
  restriction and its facets sit in a *different namespace* than the IDS
  element wrapping them.
- Schema defaults are load-bearing: `applicability/@minOccurs` and `@maxOccurs`
  default to 1, facet `@cardinality` to `required`. See `Occurrence::DEFAULT`.
- `<requirements>` is `<xs:sequence maxOccurs="unbounded">`, so facet types may
  interleave. Iterate in document order; do not group by type.
- `relation="IFCRELVOIDSELEMENT IFCRELFILLSELEMENT"` is one enumeration token
  containing a space. Do not split it.
- `ifcVersion` is an `xs:list`: `"IFC2X3 IFC4"` is one attribute, two values.
- `dataType` is `ids:upperCaseName`, pattern `[A-Z]+` — no digits, no
  underscores.
- IFC entity names match case-sensitively in upper case, but `propertySet`,
  `baseName` and attribute `name` are mixed case as they appear in the IFC file.

## Completion log

### `IDS-PARSE` — 2026-09-26

`read::from_str`/`from_slice` read IDS 1.0 into `model::Ids`. A lenient first
pass feeds `VersionSignals` and the `xsi:schemaLocation` declaration to
`detect_version`; drafts and contradictions are refused with the evidence. The
strict second pass follows `ids.xsd` 1.0.0. Every note above is honoured and
has a test in `tests/read.rs`.

Finding: the corpus `invalid-*` cases are schema-valid IDS whose content
contradicts IFC (`42.0` for an integer attribute, a subclass required where
the applicability names its parent). Rejecting them needs the IFC schema, so
it moves to `IDS-AUDIT`; the reader must, and does, read all of them.

Proof:

```
$ IDS_TEST_CASES=.../IDS/Documentation/ImplementersDocumentation/TestCases \
    cargo test --test corpus -- --ignored
test corpus_every_case_reads_as_declared_ids_1_0 ... ok   (334 cases)
```

Differential check against IfcOpenShell ifctester 0.8.5 over the same 334
files (specification name, releases, identifier, description, instructions,
occurrence bounds, every facet with its values, restrictions, cardinality,
uri and instructions): 0 differences once ifctester's `""` for an absent
description is read as absent.

Mutation-verified:

| Mutation | Test that caught it |
|---|---|
| absent `minOccurs` read as `0` | `schema_defaults_are_applied_explicitly` |
| absent `@cardinality` read as optional | `schema_defaults_are_applied_explicitly`, `requirement_facets_keep_document_order_and_their_attributes` |
| `partOf` accepts `optional` | `schema_violations_are_refused` |
| a second applicability `entity` accepted | `schema_violations_are_refused` |
| drafts read as 1.0 | `versions_are_detected_and_drafts_refused` |
| `xsi:` attributes accepted below the root | `schema_violations_are_refused` |

### `IDS-VERSION-EVIDENCE` — 2026-09-21

Version detection returns `Detected<IdsVersion>`; a 1.0 document carrying
`minOccurs` on a requirement facet is reported as a conflict rather than parsed;
occurrence is modelled where each revision puts it, so equal intent in 0.9 and
1.0 produces equal typed output; each rename is covered by a test.

Detection consumes observations rather than XML, so extracting signals from a
document lands with `IDS-PARSE`.

Proof:

```
$ bash scripts/gate.sh
test result: ok. 32 passed; 0 failed   (lib)
test result: ok. 6 passed; 0 failed    (doc)
```

Mutation-verified — each gate was shown to fail on a real defect before being
trusted:

| Mutation | Test that caught it |
|---|---|
| `as_str` for `Draft0_9_6` returns `"0.96"` | `every_version_round_trips_through_text` |
| `detect_version` always reports agreement | `declared_1_0_over_a_draft_shape_is_a_conflict`, `declared_draft_over_a_1_0_shape_is_a_conflict` |
| `Occurrence::DEFAULT` set to `Optional` | `default_is_required` |
