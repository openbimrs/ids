//! The IDS 1.0 writer: round trips, output shape and refusals.

mod common;

use openbim_core::Detected;
use openbim_ids::{
    from_str, to_string, to_writer, Attribute, Entity, Facet, Ids, IdsVersion, IfcVersion, Info,
    Material, Occurrence, PartOf, Property, Requirement, Restriction, Specification, Value,
    WriteErrorKind,
};

fn minimal() -> Ids {
    let mut spec = Specification::new("Walls", [IfcVersion::Ifc4]);
    spec.applicability
        .facets
        .push(Entity::new("IFCWALL").into());
    let mut ids = Ids::new(Info::new("Minimal"));
    ids.specifications.push(spec);
    ids
}

fn refusal(ids: &Ids) -> (WriteErrorKind, String) {
    let error = to_string(ids).expect_err("the writer must refuse this model");
    (error.kind().clone(), error.location().to_owned())
}

#[test]
fn constructed_documents_read_back_unchanged() {
    for ids in [minimal(), common::kitchen_sink()] {
        let xml = to_string(&ids).expect("writes");
        assert_eq!(from_str(&xml).expect("reads back"), ids, "{xml}");
    }
}

#[test]
fn constructors_state_ids_1_0_and_optional_applicability() {
    let ids = Ids::new(Info::new("t"));
    assert_eq!(ids.version, Detected::Declared(IdsVersion::Ids1_0));
    let spec = Specification::new("s", [IfcVersion::Ifc4]);
    assert_eq!(spec.occurrence(), Occurrence::Optional);
    assert_eq!(
        (spec.applicability.min_occurs, spec.applicability.max_occurs),
        (0, None)
    );
    assert_eq!(
        Requirement::new(Attribute::new("Name")).occurrence,
        Occurrence::Required
    );
}

#[test]
fn output_declares_namespaces_and_schema_location() {
    let xml = to_string(&minimal()).unwrap();
    assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<ids "));
    for declaration in [
        r#"xmlns="http://standards.buildingsmart.org/IDS""#,
        r#"xmlns:xs="http://www.w3.org/2001/XMLSchema""#,
        r#"xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance""#,
        r#"xsi:schemaLocation="http://standards.buildingsmart.org/IDS http://standards.buildingsmart.org/IDS/1.0/ids.xsd""#,
    ] {
        assert!(xml.contains(declaration), "missing {declaration}\n{xml}");
    }
}

#[test]
fn output_shape_is_canonical() {
    let mut ids = minimal();
    ids.specifications[0].require(Requirement {
        instructions: Some("i".into()),
        ..Requirement::with_occurrence(
            Property {
                value: Some(Restriction::enumeration(["A", "B"]).into()),
                ..Property::new("Pset_WallCommon", "Status")
            },
            Occurrence::Optional,
        )
    });
    let expected = r#"<?xml version="1.0" encoding="UTF-8"?>
<ids xmlns="http://standards.buildingsmart.org/IDS" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:schemaLocation="http://standards.buildingsmart.org/IDS http://standards.buildingsmart.org/IDS/1.0/ids.xsd">
  <info>
    <title>Minimal</title>
  </info>
  <specifications>
    <specification name="Walls" ifcVersion="IFC4">
      <applicability minOccurs="0" maxOccurs="unbounded">
        <entity>
          <name>
            <simpleValue>IFCWALL</simpleValue>
          </name>
        </entity>
      </applicability>
      <requirements>
        <property cardinality="optional" instructions="i">
          <propertySet>
            <simpleValue>Pset_WallCommon</simpleValue>
          </propertySet>
          <baseName>
            <simpleValue>Status</simpleValue>
          </baseName>
          <value>
            <xs:restriction base="xs:string">
              <xs:enumeration value="A"/>
              <xs:enumeration value="B"/>
            </xs:restriction>
          </value>
        </property>
      </requirements>
    </specification>
  </specifications>
</ids>
"#;
    assert_eq!(to_string(&ids).unwrap(), expected);
}

#[test]
fn schema_default_bounds_are_omitted_and_others_written() {
    let mut ids = minimal();
    let applicability = &mut ids.specifications[0].applicability;
    (applicability.min_occurs, applicability.max_occurs) = (1, Some(1));
    assert!(to_string(&ids).unwrap().contains("<applicability>\n"));
    ids.specifications[0]
        .applicability
        .set_occurrence(Occurrence::Prohibited);
    assert!(to_string(&ids)
        .unwrap()
        .contains(r#"<applicability minOccurs="0" maxOccurs="0">"#));
}

#[test]
fn awkward_text_survives_everywhere() {
    let ids = common::kitchen_sink();
    let xml = to_string(&ids).unwrap();
    // Attribute whitespace is escaped so normalization cannot eat it.
    assert!(xml.contains("&#9; tab &#10; line &#13;&#10; crlf"));
    let back = from_str(&xml).unwrap();
    assert_eq!(back.info.title, common::AWKWARD);
    assert_eq!(back.specifications[0].name, common::AWKWARD);
}

#[test]
fn to_writer_matches_to_string_and_leaves_output_untouched_on_refusal() {
    let mut out = Vec::new();
    to_writer(&minimal(), &mut out).unwrap();
    assert_eq!(out, to_string(&minimal()).unwrap().into_bytes());

    let mut out = Vec::new();
    let mut ids = minimal();
    ids.specifications.clear();
    assert!(to_writer(&ids, &mut out).is_err());
    assert!(out.is_empty());
}

#[test]
fn io_failures_are_reported() {
    struct Broken;
    impl std::io::Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("disk on fire"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let error = to_writer(&minimal(), Broken).unwrap_err();
    assert!(
        matches!(error.kind(), WriteErrorKind::Io { message, .. } if message == "disk on fire")
    );
}

#[test]
fn only_ids_1_0_is_written() {
    for version in [
        Detected::Declared(IdsVersion::Draft0_9_7),
        Detected::Inferred(IdsVersion::Draft0_9),
        Detected::Conflict {
            declared: IdsVersion::Ids1_0,
            observed: IdsVersion::Draft0_9,
        },
    ] {
        let mut ids = minimal();
        ids.version = version.clone();
        assert_eq!(
            refusal(&ids),
            (WriteErrorKind::UnsupportedVersion(version), String::new())
        );
    }
    // An inferred 1.0 is written, and declared from then on.
    let mut ids = minimal();
    ids.version = Detected::Inferred(IdsVersion::Ids1_0);
    let back = from_str(&to_string(&ids).unwrap()).unwrap();
    assert_eq!(back.version, Detected::Declared(IdsVersion::Ids1_0));
}

#[test]
fn schema_violations_are_refused_with_their_location() {
    let mut ids = minimal();
    ids.specifications.clear();
    assert_eq!(refusal(&ids).0, WriteErrorKind::NoSpecifications);

    let mut ids = minimal();
    ids.specifications[0].ifc_versions.clear();
    assert_eq!(
        refusal(&ids),
        (WriteErrorKind::NoIfcVersions, "specifications[0]".into())
    );

    let mut ids = minimal();
    ids.info.author = Some("nobody".into());
    assert!(matches!(
        refusal(&ids),
        (WriteErrorKind::InvalidValue { .. }, location) if location == "info/author"
    ));

    let mut ids = minimal();
    ids.info.date = Some("2026-02-30".into());
    assert!(matches!(
        refusal(&ids),
        (WriteErrorKind::InvalidValue { .. }, location) if location == "info/date"
    ));

    for data_type in ["IfcLabel", "IFC_LABEL", ""] {
        let mut ids = minimal();
        ids.specifications[0].require(Property {
            data_type: Some(data_type.into()),
            ..Property::new("P", "N")
        });
        assert!(matches!(
            refusal(&ids),
            (WriteErrorKind::InvalidValue { .. }, location)
                if location == "specifications[0]/requirements/facets[0]/@dataType"
        ));
    }

    let mut ids = minimal();
    ids.specifications[0].applicability.facets[0] = Entity::new(Restriction {
        patterns: vec![".*".into()],
        ..Restriction::new("notAType")
    })
    .into();
    assert!(matches!(
        refusal(&ids).0,
        WriteErrorKind::InvalidValue { .. }
    ));

    let mut ids = minimal();
    ids.specifications[0].applicability.facets[0] = Entity::new(Restriction {
        total_digits: Some(0),
        ..Restriction::new("decimal")
    })
    .into();
    assert!(matches!(
        refusal(&ids).0,
        WriteErrorKind::InvalidValue { .. }
    ));
}

#[test]
fn inexpressible_models_are_refused() {
    let mut ids = minimal();
    ids.specifications[0].applicability.facets.clear();
    assert_eq!(
        refusal(&ids),
        (
            WriteErrorKind::EmptyApplicability,
            "specifications[0]/applicability".into()
        )
    );

    let mut ids = minimal();
    ids.specifications[0].applicability.facets[0] = Entity::new(Restriction::new("string")).into();
    assert_eq!(
        refusal(&ids),
        (
            WriteErrorKind::EmptyRestriction,
            "specifications[0]/applicability/facets[0]/name/xs:restriction".into()
        )
    );

    let mut ids = minimal();
    ids.specifications[0]
        .applicability
        .facets
        .insert(0, Material::default().into());
    assert_eq!(
        refusal(&ids),
        (
            WriteErrorKind::ApplicabilityOrder { found: "entity" },
            "specifications[0]/applicability/facets[1]".into()
        )
    );

    let mut ids = minimal();
    ids.specifications[0]
        .applicability
        .facets
        .push(Entity::new("IFCSLAB").into());
    assert!(matches!(
        refusal(&ids).0,
        WriteErrorKind::ApplicabilityOrder { found: "entity" }
    ));

    for (facet, occurrence) in [
        (Facet::from(Entity::new("IFCWALL")), Occurrence::Optional),
        (Facet::from(Entity::new("IFCWALL")), Occurrence::Prohibited),
        (
            Facet::from(PartOf::new(Entity::new("IFCSLAB"))),
            Occurrence::Optional,
        ),
    ] {
        let mut ids = minimal();
        let kind = facet.kind();
        ids.specifications[0].require(Requirement::with_occurrence(facet, occurrence));
        assert_eq!(
            refusal(&ids).0,
            WriteErrorKind::OccurrenceNotAllowed {
                facet: kind,
                occurrence
            }
        );
    }

    for facet in [
        Facet::from(Entity::new("IFCWALL")),
        Attribute::new("Name").into(),
        PartOf::new(Entity::new("IFCSLAB")).into(),
    ] {
        let mut ids = minimal();
        let kind = facet.kind();
        ids.specifications[0].require(Requirement {
            uri: Some("urn:x".into()),
            ..Requirement::new(facet)
        });
        assert_eq!(
            refusal(&ids).0,
            WriteErrorKind::AttributeNotAllowed {
                facet: kind,
                attribute: "uri"
            }
        );
    }
}

#[test]
fn characters_xml_cannot_carry_are_refused() {
    for bad in ["\u{0}", "a\u{1b}b", "\u{FFFE}"] {
        let mut ids = minimal();
        ids.info.title = bad.into();
        assert!(matches!(
            refusal(&ids),
            (WriteErrorKind::InvalidCharacter(_), location) if location == "info/title"
        ));
        let mut ids = minimal();
        ids.specifications[0].name = bad.into();
        assert!(matches!(
            refusal(&ids),
            (WriteErrorKind::InvalidCharacter(_), location) if location == "specifications[0]/@name"
        ));
        let mut ids = minimal();
        ids.specifications[0].applicability.facets[0] =
            Entity::new(Value::Simple(bad.into())).into();
        assert!(matches!(
            refusal(&ids).0,
            WriteErrorKind::InvalidCharacter(_)
        ));
    }
}
