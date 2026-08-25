# Architecture

## Repository role

`openbimrs/ids` is the canonical source repository for the IDS family.
`openbimrs/openbim` pins a verified commit of this repository at `packages/ids`
and provides ecosystem-level integration tests and the feature-gated `openbim`
facade.

The child repository must remain buildable without cloning the integration
workspace. Its published crate therefore uses versioned registry dependencies,
not paths into sibling repositories.

## Dependency direction

```text
codec/core  <-  IFC  <-  IDS
     ^                 |
     +-----------------+

openbim facade  ->  IDS
```

More precisely:

- IDS may use `openbim-core` for shared vocabulary.
- IDS may use public IFC crates when IFC-model auditing is implemented.
- IFC, core, and codec crates must never depend on IDS.
- The `openbim` facade may optionally re-export IDS.

This direction keeps IDS policy and validation semantics out of the IFC model
and serialization layers.

## Workspace independence

Cargo permits the crate to be selected as a member of the parent integration
workspace even though this repository also has its own workspace manifest.
However, `version.workspace = true` and `dependency.workspace = true` resolve
against the workspace invoking Cargo. That can silently produce different
release metadata in standalone and integrated builds.

For that reason, the published crate declares these fields explicitly:

- package version, edition, MSRV, license, authors, and repository
- cross-repository dependency versions

The parent can test local source through path-plus-version integration entries,
while the standalone repository resolves released dependencies from crates.io.

## Version handling invariant

The IDS XML namespace identifies IDS but does not uniquely identify a schema
revision. Parsing must report evidence for a detected version, and conflicting
evidence must remain visible to callers. New documents target approved IDS 1.0;
older drafts are read-compatibility concerns.

## Standards artifacts

This repository does not vendor ISO, CEN, or other restricted schema documents.
A local or CI fixture is admitted only when its redistribution terms are known
and compatible with the repository license and intended use.

## Cross-repository delivery

Changes spanning repositories follow dependency order:

1. land and publish lower-level contract changes;
2. update IDS and verify it standalone;
3. publish the IDS commit;
4. update and verify the `openbim` submodule pin;
5. publish the integration commit.

The superproject pin is the compatibility declaration and rollback point.
