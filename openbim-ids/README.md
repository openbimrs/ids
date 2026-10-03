# openbim-ids

buildingSMART IDS (Information Delivery Specification) contracts for Rust.

The standard, machine-readable way to state *"this model must contain these
things, with these properties"* and audit a model against it.

## Status

Published releases up to `0.1.2` are a reserved scaffold. `0.1.3` onwards
provides:

- the XML namespace shared by published IDS revisions;
- an `IdsVersion` model, occurrence and version-evidence contracts;
- an IDS 1.0 reader (`read::from_str`, `read::from_slice`) into a typed model,
  which refuses drafts and contradictory declarations with their evidence.

`0.1.4` adds an IDS 1.0 writer (`to_string`, `to_writer`) whose output
validates against `ids.xsd` and reads back as the model it was given, and
constructors for building a document from scratch.

`0.2.0` adds `audit()`: it reports, with stable codes, what in an IDS
document cannot work for its listed IFC releases, such as unknown entities or
attributes, values that cannot be cast to the IFC type, and properties that
contradict the standard property set templates. Two features enable it:

| Feature | Checks | Adds dependencies under |
| --- | --- | --- |
| `audit-schema` (0.2.1+) | everything except standard `Pset_`/`Qto_` sets | AGPL-3.0-or-later (`ifc-schema`) |
| `audit` | everything | AGPL-3.0-or-later and CC BY-ND 4.0 (`ifc-template-catalog` template data) |

```toml
openbim-ids = { version = "0.2.1", features = ["audit-schema"] }
```

**Licence note:** enabling either feature puts those terms on the resulting
work; without them the crate and its dependencies are MIT.

It does not check an IFC model against IDS. Requires Rust 1.88.

See the [repository capability table](https://github.com/openbimrs/ids#status)
before relying on a feature. Future parsing must report version-detection
evidence rather than infer a schema revision from the shared namespace.

## Example

```rust
use openbim_ids::{Entity, Ids, IfcVersion, Info, Property, Restriction, Specification};

let mut spec = Specification::new("Walls have a fire rating", [IfcVersion::Ifc4]);
spec.applicability.facets.push(Entity::new("IFCWALL").into());
spec.require(Property {
    value: Some(Restriction::enumeration(["REI30", "REI60"]).into()),
    ..Property::new("Pset_WallCommon", "FireRating")
});
let mut ids = Ids::new(Info::new("Fire safety"));
ids.specifications.push(spec);

let xml = openbim_ids::to_string(&ids).unwrap();
assert_eq!(openbim_ids::from_str(&xml).unwrap(), ids);
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
