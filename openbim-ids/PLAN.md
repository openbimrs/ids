# openbim-ids implementation plan

Status: name reserved; namespace/version contracts implemented; parser not started.
Last updated: 2026-08-25

This is task state, not ambient context. Follow `AGENTS.md`; claim one task ID,
record blockers/decisions under it, and check it off only with executable
evidence.

## Established boundary

IDS consumes `openbim-core` and, when auditing is implemented, public IFC
contracts. IFC, core, and codec must never depend on IDS.

## Implemented scaffold

- `NAMESPACE` for the namespace shared by published IDS revisions
- `IdsVersion`, `CURRENT`, and approved-version behavior
- unit tests for version ordering and approval

These are contracts, not a parser or validator.

## Work queue

- [ ] `IDS-VERSION-EVIDENCE` - define version evidence and disagreement reporting
- [ ] `IDS-PARSE` - parse IDS without silently guessing the schema version
- [ ] `IDS-WRITE` - write approved IDS 1.0 by default, with explicit legacy opt-in
- [ ] `IDS-AUDIT` - distinguish applicable/pass/fail/not-applicable outcomes
- [ ] `IDS-CORPUS` - verify buildingSMART pass/fail fixtures with licensed inputs

## Completion log

No behavioral capability completed yet. Record the proof command and result here
when an item above is checked off.
