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

## Releasing

Releases publish from CI through crates.io trusted publishing
(`.github/workflows/release.yml`); nobody needs a crates.io token.

1. Bump `version` in `openbim-ids/Cargo.toml` and run
   `cargo update -p openbim-ids`.
2. Move the `[Unreleased]` notes in `CHANGELOG.md` under a dated
   `## [<version>]` section and add its link reference.
3. Run `./scripts/gate.sh`, merge to `main`, then push an annotated tag
   `v<version>` on that commit.

The workflow refuses a tag that is not on `main`, does not match the
`Cargo.toml` version, or has no changelog section; runs the gate on the
tagged commit; publishes from the `crates.io` environment, which asks a
reviewer for approval; and creates the GitHub release from the changelog
section. Re-running a partly failed release skips what is already live.
Rehearse with *Actions -> Release -> Run workflow* and a tag name: everything
but publishing runs.

crates.io trusts exactly this repository, the file name `release.yml` and the
`crates.io` environment (crate Settings -> Trusted Publishing); renaming
either needs the same change there.

## Licensing contributions

Unless an explicitly signed agreement says otherwise, every contribution
submitted to this repository is licensed under `MIT`. Submit only
work that you have the right to license. Identify third-party material and
preserve its license, attribution, and provenance.
