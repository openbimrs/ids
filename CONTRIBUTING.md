# Contributing

Contributions are welcome, especially those that turn reserved IDS contracts
into conformance-tested behavior.

## Before opening a pull request

1. Read `AGENTS.md` and the affected crate's nested instructions.
2. Keep IDS as a consumer of core and IFC contracts; do not introduce reverse
   dependencies.
3. Add tests before claiming parsing, writing, or audit behavior.
4. Prefer public, redistributable fixtures. Do not commit restricted standards
   schemas.
5. Run:

```bash
./scripts/gate.sh
```

6. Update README capability status, rustdoc, and `CHANGELOG.md` when behavior is
   user-visible.

## Conformance work

An IDS parser or auditor is not complete because representative examples pass.
Coverage should use the buildingSMART IDS pass/fail corpus where licensing and
redistribution permit it, distinguish not-applicable from passed, and report
version-detection evidence rather than infer a version from the shared namespace.

## Commits

Use focused commits with imperative subjects. Cross-repository changes publish
lower-level dependencies first and update the `openbimrs/openbim` submodule pin
last.
