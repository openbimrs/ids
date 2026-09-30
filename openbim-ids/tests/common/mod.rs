//! Documents built with the producer constructors only, shared by tests.

#![allow(dead_code)]

use openbim_ids::{
    Attribute, Classification, Entity, Facet, Ids, IfcVersion, Info, Material, Occurrence, PartOf,
    Property, Relation, Requirement, Restriction, Specification, Value,
};

/// Awkward text: markup characters, quotes, every XML whitespace character,
/// surrounding blanks and non-ASCII.
pub const AWKWARD: &str =
    "  a & b < c > d \"q\" 'a' ]]> \t tab \n line \r\n crlf \u{e9}\u{1F600}  ";

/// A document exercising every construct the model can hold.
pub fn kitchen_sink() -> Ids {
    let mut info = Info::new(AWKWARD);
    info.copyright = Some("\u{a9} OpenBIM.rs".into());
    info.version = Some("1.2.3".into());
    info.description = Some(AWKWARD.into());
    info.author = Some("someone@example.org".into());
    info.date = Some("2026-09-30".into());
    info.purpose = Some("Tests".into());
    info.milestone = Some("Design".into());
    let mut ids = Ids::new(info);

    let mut walls = Specification::new(AWKWARD, [IfcVersion::Ifc2x3, IfcVersion::Ifc4]);
    walls.identifier = Some("W-1".into());
    walls.description = Some(AWKWARD.into());
    walls.instructions = Some("Model walls".into());
    walls.applicability.set_occurrence(Occurrence::Required);
    let mut wall = Entity::new("IFCWALL");
    wall.predefined_type = Some(Restriction::enumeration(["SOLIDWALL", "SHEAR"]).into());
    walls.applicability.facets.extend([
        Facet::from(wall),
        PartOf {
            relation: Some(Relation::ContainedInSpatialStructure),
            ..PartOf::new(Entity::new("IFCBUILDINGSTOREY"))
        }
        .into(),
        Classification {
            value: Some("21.22".into()),
            ..Classification::new("NL-SfB")
        }
        .into(),
        Attribute::new("Name").into(),
        Property {
            data_type: Some("IFCLABEL".into()),
            ..Property::new("Pset_WallCommon", "Reference")
        }
        .into(),
        Material::default().into(),
    ]);
    walls
        .require(Entity::new("IFCWALL"))
        .require(Requirement {
            instructions: Some("Exact name".into()),
            ..Requirement::new(Attribute {
                value: Some(Value::Simple(AWKWARD.into())),
                ..Attribute::new("Name")
            })
        })
        .require(Requirement {
            uri: Some("https://identifier.buildingsmart.org/uri/x".into()),
            ..Requirement::with_occurrence(
                Property {
                    data_type: Some("IFCLENGTHMEASURE".into()),
                    value: Some(
                        Restriction {
                            min_inclusive: Some("0.1".into()),
                            max_exclusive: Some("2.5".into()),
                            min_exclusive: Some("0".into()),
                            max_inclusive: Some("2.4".into()),
                            total_digits: Some(4),
                            fraction_digits: Some(2),
                            ..Restriction::new("double")
                        }
                        .into(),
                    ),
                    ..Property::new(Restriction::pattern("Pset_.*"), "Width")
                },
                Occurrence::Optional,
            )
        })
        .require(Requirement::with_occurrence(
            Classification::new(Restriction {
                length: Some(3),
                min_length: Some(1),
                max_length: Some(5),
                ..Restriction::new("string")
            }),
            Occurrence::Prohibited,
        ))
        .require(Requirement {
            uri: Some("urn:material".into()),
            ..Requirement::new(Material {
                value: Some("concrete".into()),
            })
        })
        .require(Requirement::with_occurrence(
            PartOf {
                relation: Some(Relation::VoidsElementFillsElement),
                ..PartOf::new(Entity::new("IFCWALL"))
            },
            Occurrence::Prohibited,
        ))
        // Requirement facets interleave; order is kept.
        .require(Attribute::new("Description"));
    walls
        .requirements
        .as_mut()
        .expect("requirements")
        .description = Some(AWKWARD.into());
    ids.specifications.push(walls);

    let mut none = Specification::new("No proxies", [IfcVersion::Ifc4x3Add2]);
    none.applicability.set_occurrence(Occurrence::Prohibited);
    none.applicability
        .facets
        .push(Entity::new("IFCBUILDINGELEMENTPROXY").into());
    ids.specifications.push(none);

    let mut bounded = Specification::new("Two to five doors", [IfcVersion::Ifc4]);
    bounded.applicability.min_occurs = 2;
    bounded.applicability.max_occurs = Some(5);
    bounded
        .applicability
        .facets
        .push(Entity::new("IFCDOOR").into());
    // An empty <requirements/> is distinct from none in the model.
    bounded.requirements = Some(Default::default());
    ids.specifications.push(bounded);

    let mut spaces = Specification::new("Spaces", [IfcVersion::Ifc4]);
    spaces
        .applicability
        .facets
        .push(Entity::new("IFCSPACE").into());
    for relation in Relation::ALL {
        spaces.require(PartOf {
            relation: Some(relation),
            ..PartOf::new(Entity::new("IFCBUILDING"))
        });
    }
    ids.specifications.push(spaces);
    ids
}
