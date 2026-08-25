# openbim-ids instructions

Purpose: buildingSMART IDS contracts; future parsing and IFC-model auditing.
Follow `../AGENTS.md`. Read `PLAN.md` only for implementation or roadmap work;
keep progress, blockers, and verification evidence there.

## Boundary

- May depend on released `openbim-core` and public IFC contracts.
- Must remain independently buildable from this repository.
- Must never be depended on by IFC, core, or codec layers.
- Keep package and cross-repository dependency versions explicit; parent
  workspace inheritance changes meaning depending on the invoking workspace.
- Never vendor restricted standards schemas without verified redistribution
  rights.

## Status

Reserved scaffold. Namespace/version contracts and their tests exist; parsing,
writing, validation, and IFC auditing do not.
