# openbim-ids

buildingSMART IDS (Information Delivery Specification) for Rust.

The standard, machine-readable way to state *"this model must contain these things, with these properties"* and audit a model against it.

Every published IDS version from 0.2 to 1.0 declares the **same** XML namespace, and the differences are in attribute names and cardinality — so a reader that guesses the version wrong silently produces a *different* specification. Version detection here reports its evidence rather than guessing.

## Status

**Reserved.** This release establishes the crate name and its place in the
layering. It does not parse files yet — see the crate documentation for what is
implemented versus reserved.

No ISO/CEN schema is vendored in this crate. Types are written *from* the
schemas; the schema files themselves are referenced out of tree, because
possessing a copy of a standard does not establish the right to redistribute it.

## Part of nehirde

A pure-Rust IFC and openBIM toolchain: <https://github.com/GeneralPawz/nehirde>

Design rationale for the crate layout: `docs/adr/0015`.

## License

MIT
