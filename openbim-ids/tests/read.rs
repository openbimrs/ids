//! The reader against hand-written documents, one schema rule at a time.

use openbim_core::Detected;
use openbim_ids::{
    from_slice, from_str, ErrorKind, Facet, Ids, IdsVersion, IfcVersion, Occurrence, Relation,
    Value,
};

const DECLARED: &str = r#"xmlns="http://standards.buildingsmart.org/IDS" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:schemaLocation="http://standards.buildingsmart.org/IDS http://standards.buildingsmart.org/IDS/1.0/ids.xsd""#;

/// A declared 1.0 document with one specification built from its parts.
fn document(specification: &str, applicability: &str, requirements: &str) -> String {
    format!(
        "<ids {DECLARED}><info><title>T</title></info><specifications>\
         <specification name=\"S\" ifcVersion=\"IFC4\" {specification}>\
         <applicability>{applicability}</applicability>{requirements}\
         </specification></specifications></ids>"
    )
}

fn wall() -> &'static str {
    "<entity><name><simpleValue>IFCWALL</simpleValue></name></entity>"
}

fn read(specification: &str, applicability: &str, requirements: &str) -> Ids {
    let text = document(specification, applicability, requirements);
    from_str(&text).unwrap_or_else(|error| panic!("{error}\n{text}"))
}

fn refuse(specification: &str, applicability: &str, requirements: &str) -> ErrorKind {
    let text = document(specification, applicability, requirements);
    match from_str(&text) {
        Ok(ids) => panic!("accepted {ids:?}\n{text}"),
        Err(error) => error.kind().clone(),
    }
}

fn simple(value: &str) -> Value {
    Value::Simple(value.to_owned())
}

#[test]
fn schema_defaults_are_applied_explicitly() {
    let ids = read(
        "",
        wall(),
        "<requirements><attribute><name><simpleValue>Name</simpleValue></name></attribute></requirements>",
    );
    let specification = &ids.specifications[0];
    // No minOccurs/maxOccurs means exactly one, which is required.
    assert_eq!(specification.applicability.min_occurs, 1);
    assert_eq!(specification.applicability.max_occurs, Some(1));
    assert_eq!(specification.occurrence(), Occurrence::Required);
    // No cardinality means required, not optional.
    let requirements = specification.requirements.as_ref().unwrap();
    assert_eq!(requirements.facets[0].occurrence, Occurrence::Required);
}

#[test]
fn applicability_bounds_spell_the_specification_occurrence() {
    for (bounds, occurrence, max) in [
        (
            r#"minOccurs="0" maxOccurs="unbounded""#,
            Occurrence::Optional,
            None,
        ),
        (
            r#"minOccurs="1" maxOccurs="unbounded""#,
            Occurrence::Required,
            None,
        ),
        (
            r#"minOccurs="0" maxOccurs="0""#,
            Occurrence::Prohibited,
            Some(0),
        ),
    ] {
        let text = document("", wall(), "")
            .replace("<applicability>", &format!("<applicability {bounds}>"));
        let ids = from_str(&text).unwrap();
        assert_eq!(ids.specifications[0].occurrence(), occurrence, "{bounds}");
        assert_eq!(
            ids.specifications[0].applicability.max_occurs, max,
            "{bounds}"
        );
    }
}

#[test]
fn requirement_facets_keep_document_order_and_their_attributes() {
    let ids = read(
        "",
        wall(),
        r#"<requirements description="d">
             <property dataType="IFCLABEL" cardinality="optional" uri="https://example.org/p" instructions="i">
               <propertySet><simpleValue>Pset_WallCommon</simpleValue></propertySet>
               <baseName><simpleValue>FireRating</simpleValue></baseName>
             </property>
             <material cardinality="prohibited"/>
             <entity instructions="e"><name><simpleValue>IFCWALL</simpleValue></name></entity>
             <property>
               <propertySet><simpleValue>Pset_WallCommon</simpleValue></propertySet>
               <baseName><simpleValue>IsExternal</simpleValue></baseName>
             </property>
           </requirements>"#,
    );
    let requirements = ids.specifications[0].requirements.as_ref().unwrap();
    assert_eq!(requirements.description.as_deref(), Some("d"));
    let kinds: Vec<&str> = requirements.facets.iter().map(|r| r.facet.kind()).collect();
    assert_eq!(kinds, ["property", "material", "entity", "property"]);
    let first = &requirements.facets[0];
    assert_eq!(first.occurrence, Occurrence::Optional);
    assert_eq!(first.uri.as_deref(), Some("https://example.org/p"));
    assert_eq!(first.instructions.as_deref(), Some("i"));
    let Facet::Property(property) = &first.facet else {
        panic!("{first:?}")
    };
    assert_eq!(property.data_type.as_deref(), Some("IFCLABEL"));
    assert_eq!(property.base_name, simple("FireRating"));
    assert_eq!(requirements.facets[1].occurrence, Occurrence::Prohibited);
    assert_eq!(requirements.facets[2].occurrence, Occurrence::Required);
}

#[test]
fn list_and_token_attributes_are_read_as_the_schema_types_them() {
    let ids = read(
        "",
        &format!(
            "{}<partOf relation=\"IFCRELVOIDSELEMENT IFCRELFILLSELEMENT\"><entity><name><simpleValue>IFCWALL</simpleValue></name></entity></partOf>",
            wall()
        ),
        "",
    )
    .specifications
    .remove(0);
    let Facet::PartOf(part_of) = &ids.applicability.facets[1] else {
        panic!("{:?}", ids.applicability.facets)
    };
    // One enumeration token that happens to contain a space.
    assert_eq!(part_of.relation, Some(Relation::VoidsElementFillsElement));

    let text = document("", wall(), "").replace(
        "ifcVersion=\"IFC4\"",
        "ifcVersion=\" IFC2X3\tIFC4\nIFC4X3_ADD2 \"",
    );
    let ids = from_str(&text).unwrap();
    assert_eq!(
        ids.specifications[0].ifc_versions,
        [IfcVersion::Ifc2x3, IfcVersion::Ifc4, IfcVersion::Ifc4x3Add2]
    );
}

#[test]
fn restrictions_resolve_their_prefix_and_keep_facets_verbatim() {
    let ids = read(
        "",
        r#"<entity><name><xsd:restriction xmlns:xsd="http://www.w3.org/2001/XMLSchema" base="xsd:string">
             <xsd:annotation><xsd:documentation>walls</xsd:documentation></xsd:annotation>
             <xsd:enumeration value="IFCWALL"/><xsd:enumeration value="IFCSLAB"/>
             <xsd:pattern value="IFC.*"/>
           </xsd:restriction></name></entity>
           <property>
             <propertySet><simpleValue>Qto_WallBaseQuantities</simpleValue></propertySet>
             <baseName><simpleValue>Width</simpleValue></baseName>
             <value><xs:restriction base="xs:double">
               <xs:minInclusive value="0.2"/><xs:maxExclusive value="1e1"/>
               <xs:minLength value="+1"/><xs:totalDigits value="5"/>
             </xs:restriction></value>
           </property>"#,
        "",
    );
    let facets = &ids.specifications[0].applicability.facets;
    let Facet::Entity(entity) = &facets[0] else {
        panic!("{facets:?}")
    };
    let Value::Restriction(name) = &entity.name else {
        panic!("{entity:?}")
    };
    assert_eq!(name.base, "string");
    assert_eq!(name.enumeration, ["IFCWALL", "IFCSLAB"]);
    assert_eq!(name.patterns, ["IFC.*"]);
    let Facet::Property(property) = &facets[1] else {
        panic!("{facets:?}")
    };
    let Some(Value::Restriction(value)) = &property.value else {
        panic!("{property:?}")
    };
    assert_eq!(value.base, "double");
    // Bounds stay in the lexical form the document wrote.
    assert_eq!(value.min_inclusive.as_deref(), Some("0.2"));
    assert_eq!(value.max_exclusive.as_deref(), Some("1e1"));
    assert_eq!(value.min_length, Some(1));
    assert_eq!(value.total_digits, Some(5));
}

#[test]
fn simple_values_are_verbatim_strings() {
    let ids = read(
        "",
        "<entity><name><simpleValue>IFCWALL</simpleValue></name></entity>\
         <attribute><name><simpleValue>Name</simpleValue></name><value><simpleValue> a &amp; b </simpleValue></value></attribute>\
         <attribute><name><simpleValue>Description</simpleValue></name><value><simpleValue/></value></attribute>",
        "",
    );
    let facets = &ids.specifications[0].applicability.facets;
    let values: Vec<_> = facets[1..]
        .iter()
        .map(|facet| match facet {
            Facet::Attribute(attribute) => attribute.value.clone(),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(values, [Some(simple(" a & b ")), Some(simple(""))]);
}

#[test]
fn info_fields_are_read_and_checked() {
    let text = document("", wall(), "").replace(
        "<info><title>T</title></info>",
        "<info><title>T</title><copyright>C</copyright><version>2</version><description>D</description>\
         <author>a@b.org</author><date> 2024-06-01 </date><purpose>P</purpose><milestone>M</milestone></info>",
    );
    let info = from_str(&text).unwrap().info;
    assert_eq!(info.author.as_deref(), Some("a@b.org"));
    // xs:date collapses whitespace.
    assert_eq!(info.date.as_deref(), Some("2024-06-01"));
    assert_eq!(info.milestone.as_deref(), Some("M"));

    for (from, to) in [
        (
            "<title>T</title>",
            "<title>T</title><author>nobody</author>",
        ),
        (
            "<title>T</title>",
            "<title>T</title><date>2024-02-30</date>",
        ),
        // Order is part of the schema.
        (
            "<title>T</title>",
            "<title>T</title><purpose>P</purpose><version>2</version>",
        ),
        ("<title>T</title>", ""),
    ] {
        let text = document("", wall(), "").replace(from, to);
        assert!(from_str(&text).is_err(), "{to}");
    }
}

#[test]
fn versions_are_detected_and_drafts_refused() {
    // Nothing declared, no revision-specific shape: read as 1.0, but say so.
    let undeclared = document("", wall(), "").replace(
        r#"xsi:schemaLocation="http://standards.buildingsmart.org/IDS http://standards.buildingsmart.org/IDS/1.0/ids.xsd""#,
        "",
    );
    assert_eq!(
        from_str(&undeclared).unwrap().version,
        Detected::Inferred(IdsVersion::Ids1_0)
    );
    assert_eq!(
        read("", wall(), "").version,
        Detected::Declared(IdsVersion::Ids1_0)
    );

    let draft_property = "<requirements><property minOccurs=\"1\"><propertySet><simpleValue>P</simpleValue></propertySet><name><simpleValue>N</simpleValue></name></property></requirements>";
    // A 1.0 header over a draft body is a conflict, not a 1.0 document.
    assert_eq!(
        refuse("", wall(), draft_property),
        ErrorKind::UnsupportedVersion(Detected::Conflict {
            declared: IdsVersion::Ids1_0,
            observed: IdsVersion::Draft0_9
        })
    );
    // An undeclared draft is recognised as one rather than failing on `<name>`.
    let draft = undeclared.replace(
        "</applicability>",
        &format!("</applicability>{draft_property}"),
    );
    assert_eq!(
        from_str(&draft).unwrap_err().kind(),
        &ErrorKind::UnsupportedVersion(Detected::Inferred(IdsVersion::Draft0_9))
    );
    // A declared draft is refused, and with nothing to contradict it the
    // declaration is the evidence.
    let declared_draft = document("", wall(), "").replace("/IDS/1.0/", "/IDS/0.9.7/");
    assert_eq!(
        from_str(&declared_draft).unwrap_err().kind(),
        &ErrorKind::UnsupportedVersion(Detected::Declared(IdsVersion::Draft0_9_7))
    );
    // A draft header over a 1.0 body is a conflict.
    let rewritten = document(
        "",
        wall(),
        "<requirements><material cardinality=\"optional\"/></requirements>",
    )
    .replace("/IDS/1.0/", "/IDS/0.9.7/");
    assert_eq!(
        from_str(&rewritten).unwrap_err().kind(),
        &ErrorKind::UnsupportedVersion(Detected::Conflict {
            declared: IdsVersion::Draft0_9_7,
            observed: IdsVersion::Ids1_0
        })
    );
}

#[test]
fn schema_violations_are_refused() {
    let facet_in_requirements = |facet: &str| format!("<requirements>{facet}</requirements>");
    type Case = (&'static str, String, String, fn(&ErrorKind) -> bool);
    let cases: Vec<Case> = vec![
        (
            "applicability facets are ordered",
            format!(
                "<property><propertySet><simpleValue>P</simpleValue></propertySet><baseName><simpleValue>N</simpleValue></baseName></property>{}",
                wall()
            ),
            String::new(),
            |k| matches!(k, ErrorKind::UnexpectedElement { .. }),
        ),
        (
            "at most one applicability entity",
            format!("{}{}", wall(), wall()),
            String::new(),
            |k| matches!(k, ErrorKind::UnexpectedElement { .. }),
        ),
        (
            "partOf cannot be optional",
            wall().to_owned(),
            facet_in_requirements("<partOf cardinality=\"optional\"><entity><name><simpleValue>IFCBUILDING</simpleValue></name></entity></partOf>"),
            |k| matches!(k, ErrorKind::InvalidValue { .. }),
        ),
        (
            "cardinality is a lower-case token",
            wall().to_owned(),
            facet_in_requirements("<material cardinality=\"Required\"/>"),
            |k| matches!(k, ErrorKind::InvalidValue { .. }),
        ),
        (
            "an entity requirement has no cardinality",
            wall().to_owned(),
            facet_in_requirements("<entity cardinality=\"required\"><name><simpleValue>IFCWALL</simpleValue></name></entity>"),
            |k| matches!(k, ErrorKind::UnexpectedAttribute { .. }),
        ),
        (
            "an attribute facet has no uri",
            wall().to_owned(),
            facet_in_requirements("<attribute uri=\"https://example.org\"><name><simpleValue>Name</simpleValue></name></attribute>"),
            |k| matches!(k, ErrorKind::UnexpectedAttribute { .. }),
        ),
        (
            "applicability facets carry no instructions",
            "<material instructions=\"x\"/>".to_owned(),
            String::new(),
            |k| matches!(k, ErrorKind::UnexpectedAttribute { .. }),
        ),
        (
            "dataType is [A-Z]+",
            wall().to_owned(),
            facet_in_requirements("<property dataType=\"IfcLabel\"><propertySet><simpleValue>P</simpleValue></propertySet><baseName><simpleValue>N</simpleValue></baseName></property>"),
            |k| matches!(k, ErrorKind::InvalidValue { .. }),
        ),
        (
            "relation is enumerated",
            format!("{}<partOf relation=\"IFCRELVOIDSELEMENT\"><entity><name><simpleValue>IFCWALL</simpleValue></name></entity></partOf>", wall()),
            String::new(),
            |k| matches!(k, ErrorKind::InvalidValue { .. }),
        ),
        (
            "a value is a simpleValue or a restriction, not both",
            "<entity><name><simpleValue>IFCWALL</simpleValue><xs:restriction base=\"xs:string\"/></name></entity>".to_owned(),
            String::new(),
            |k| matches!(k, ErrorKind::UnexpectedElement { .. }),
        ),
        (
            "a value is not empty",
            "<entity><name/></entity>".to_owned(),
            String::new(),
            |k| matches!(k, ErrorKind::MissingElement { .. }),
        ),
        (
            "a bound appears once",
            "<entity><name><xs:restriction base=\"xs:string\"><xs:minLength value=\"1\"/><xs:minLength value=\"2\"/></xs:restriction></name></entity>".to_owned(),
            String::new(),
            |k| matches!(k, ErrorKind::UnexpectedElement { .. }),
        ),
        (
            "totalDigits is positive",
            "<entity><name><xs:restriction base=\"xs:decimal\"><xs:totalDigits value=\"0\"/></xs:restriction></name></entity>".to_owned(),
            String::new(),
            |k| matches!(k, ErrorKind::InvalidValue { .. }),
        ),
        (
            "whiteSpace changes matching and is not represented",
            "<entity><name><xs:restriction base=\"xs:string\"><xs:whiteSpace value=\"collapse\"/></xs:restriction></name></entity>".to_owned(),
            String::new(),
            |k| matches!(k, ErrorKind::Unsupported { .. }),
        ),
        (
            "base names an XML Schema type",
            "<entity><name><xs:restriction base=\"string\"/></name></entity>".to_owned(),
            String::new(),
            |k| matches!(k, ErrorKind::InvalidValue { .. }),
        ),
        (
            "schema-instance attributes belong on the root only",
            "<entity xsi:type=\"xs:string\"><name><simpleValue>IFCWALL</simpleValue></name></entity>".to_owned(),
            String::new(),
            |k| matches!(k, ErrorKind::UnexpectedAttribute { .. }),
        ),
        (
            "element-only content has no text",
            format!("{}stray", wall()),
            String::new(),
            |k| matches!(k, ErrorKind::UnexpectedText { .. }),
        ),
        (
            "facets are IDS elements",
            "<ids:entity xmlns:ids=\"urn:other\"><name><simpleValue>IFCWALL</simpleValue></name></ids:entity>".to_owned(),
            String::new(),
            |k| matches!(k, ErrorKind::UnexpectedElement { .. }),
        ),
    ];
    for (why, applicability, requirements, expected) in cases {
        let kind = refuse("", &applicability, &requirements);
        assert!(expected(&kind), "{why}: {kind:?}");
    }

    for (why, specification) in [
        ("ifcVersion is enumerated", "ifcVersion=\"IFC5\""),
        ("unknown attributes are refused", "vendor=\"x\""),
    ] {
        let text = document(specification, wall(), "")
            .replace(" ifcVersion=\"IFC4\" ifcVersion", " ifcVersion");
        assert!(from_str(&text).is_err(), "{why}");
    }
}

#[test]
fn a_document_needs_a_specification_and_the_ids_root() {
    let empty = format!("<ids {DECLARED}><info><title>T</title></info><specifications/></ids>");
    assert_eq!(
        from_str(&empty).unwrap_err().kind(),
        &ErrorKind::MissingElement {
            parent: "specifications".into(),
            expected: "specification"
        }
    );
    let foreign = "<ids xmlns=\"urn:not-ids\"/>";
    assert!(matches!(
        from_str(foreign).unwrap_err().kind(),
        ErrorKind::NotIds { .. }
    ));
}

#[test]
fn byte_input_is_utf8_with_an_optional_bom() {
    let text = document("", wall(), "");
    let mut bom = b"\xEF\xBB\xBF".to_vec();
    bom.extend_from_slice(text.as_bytes());
    assert!(from_slice(&bom).is_ok());
    let latin1 = text.replace("name=\"S\"", "name=\"S\u{e9}\"");
    let latin1: Vec<u8> = latin1.chars().map(|c| c as u8).collect();
    assert_eq!(from_slice(&latin1).unwrap_err().kind(), &ErrorKind::NotUtf8);
}

#[test]
fn a_dtd_is_refused() {
    let text = format!(
        "<!DOCTYPE ids [<!ENTITY x \"y\">]>{}",
        document("", wall(), "")
    );
    assert!(matches!(
        from_str(&text).unwrap_err().kind(),
        ErrorKind::Xml(_)
    ));
}

#[test]
fn errors_point_at_the_offending_node() {
    let text =
        document("", wall(), "").replace("<applicability>", "\n  <applicability>\n    <bogus/>");
    let error = from_str(&text).unwrap_err();
    assert_eq!((error.line(), error.column()), (3, 5), "{error}");
    assert!(error.to_string().starts_with("3:5: "), "{error}");
}
