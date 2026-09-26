# openbim-ids

buildingSMART IDS (Information Delivery Specification) contracts for Rust.

The standard, machine-readable way to state *"this model must contain these
things, with these properties"* and audit a model against it.

## Status

Published releases up to `0.1.2` are a reserved scaffold. Unreleased `main`
provides:

- the XML namespace shared by published IDS revisions;
- an `IdsVersion` model, occurrence and version-evidence contracts;
- an IDS 1.0 reader (`read::from_str`, `read::from_slice`) into a typed model,
  which refuses drafts and contradictory declarations with their evidence.

It does not write IDS or validate a model against it.

See the [repository capability table](https://github.com/openbimrs/ids#status)
before relying on a feature. Future parsing must report version-detection
evidence rather than infer a schema revision from the shared namespace.

## Example

```rust
use openbim_ids::{IdsVersion, NAMESPACE};

assert_eq!(NAMESPACE, "http://standards.buildingsmart.org/IDS");
assert_eq!(IdsVersion::CURRENT, IdsVersion::Ids1_0);
assert!(IdsVersion::CURRENT.is_approved());
```

## Architecture

IDS consumes shared openBIM and, eventually, IFC contracts. IFC must never
depend on IDS. See the
[architecture documentation](https://github.com/openbimrs/ids/blob/main/docs/architecture.md).

No ISO/CEN schema is vendored in this crate. Types may be written from legally
accessed specifications, but standards possession does not establish a right to
redistribute the source schema.

## OpenBIM.rs

- IDS repository: <https://github.com/openbimrs/ids>
- Integration workspace: <https://github.com/openbimrs/openbim>
- API documentation: <https://docs.rs/openbim-ids>

## License

MIT
