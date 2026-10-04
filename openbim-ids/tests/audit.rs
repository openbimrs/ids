//! One small hand-written document per audit check, with a near-miss that
//! must stay clean. The standard set checks need `audit`; the rest run with
//! `audit-schema`.

#![cfg(feature = "audit-schema")]

use openbim_ids::{
    audit, Attribute, AuditCode, Entity, Facet, Ids, IfcVersion, Info, PartOf, Property, Relation,
    Restriction, Severity, Specification, Value,
};

fn ids(versions: &[IfcVersion], applicability: Vec<Facet>, requirements: Vec<Facet>) -> Ids {
    let mut spec = Specification::new("s", versions.iter().copied());
    spec.applicability.facets = applicability;
    for facet in requirements {
        spec.require(facet);
    }
    let mut ids = Ids::new(Info::new("t"));
    ids.specifications.push(spec);
    ids
}

/// The codes found, with the release each applies to.
fn codes(ids: &Ids) -> Vec<(AuditCode, Option<IfcVersion>)> {
    audit(ids)
        .into_iter()
        .map(|f| (f.code, f.ifc_version))
        .collect()
}

fn entity(name: &str) -> Facet {
    Entity::new(name).into()
}

fn attribute(name: impl Into<Value>, value: Option<Value>) -> Facet {
    Attribute {
        value,
        ..Attribute::new(name)
    }
    .into()
}

fn property(set: &str, name: &str, data_type: Option<&str>, value: Option<Value>) -> Facet {
    Property {
        data_type: data_type.map(Into::into),
        value,
        ..Property::new(set, name)
    }
    .into()
}

const IFC4: &[IfcVersion] = &[IfcVersion::Ifc4];
const IFC2X3: &[IfcVersion] = &[IfcVersion::Ifc2x3];

#[test]
fn entity_names_must_be_upper_case() {
    let found = codes(&ids(IFC4, vec![entity("IfcWall")], vec![]));
    assert_eq!(found, [(AuditCode::EntityNameCase, None)]);
    assert!(codes(&ids(IFC4, vec![entity("IFCWALL")], vec![])).is_empty());
}

#[test]
fn entities_must_exist_in_each_listed_release() {
    let found = codes(&ids(IFC4, vec![entity("IFCRABBIT")], vec![]));
    assert_eq!(found, [(AuditCode::EntityUnknown, Some(IfcVersion::Ifc4))]);
    // IfcAlignment is new in IFC4.3: only the IFC4 listing is wrong.
    let found = codes(&ids(
        &[IfcVersion::Ifc4, IfcVersion::Ifc4x3Add2],
        vec![entity("IFCALIGNMENT")],
        vec![],
    ));
    assert_eq!(found, [(AuditCode::EntityUnknown, Some(IfcVersion::Ifc4))]);
}

#[test]
fn ifc2x3_accepts_the_mapped_ifc4_occurrence_names() {
    let mut terminal = Entity::new("IFCAIRTERMINAL");
    terminal.predefined_type = Some("DIFFUSER".into());
    assert!(codes(&ids(IFC2X3, vec![terminal.into()], vec![])).is_empty());
}

#[test]
fn entity_patterns_must_match_something() {
    let found = codes(&ids(
        IFC4,
        vec![Entity::new(Restriction::pattern("IFCZZ.*")).into()],
        vec![],
    ));
    assert_eq!(
        found,
        [(
            AuditCode::EntityPatternMatchesNothing,
            Some(IfcVersion::Ifc4)
        )]
    );
    let pattern = Entity::new(Restriction::pattern("IFCWALL.*")).into();
    assert!(codes(&ids(IFC4, vec![pattern], vec![])).is_empty());
}

#[test]
fn predefined_types_need_an_entity_that_has_one() {
    let inventory = |predefined: &str| {
        let mut e = Entity::new("IFCINVENTORY");
        e.predefined_type = Some(predefined.into());
        Facet::from(e)
    };
    let found = codes(&ids(IFC2X3, vec![inventory("BUNNY")], vec![]));
    assert_eq!(
        found,
        [(
            AuditCode::PredefinedTypeUnavailable,
            Some(IfcVersion::Ifc2x3)
        )]
    );
    // IFC4 gives IfcInventory a PredefinedType; user-defined values are fine.
    assert!(codes(&ids(IFC4, vec![inventory("BUNNY")], vec![])).is_empty());
    // IfcWall has none in IFC2X3, but its type entity does.
    let mut wall = Entity::new("IFCWALL");
    wall.predefined_type = Some("WALDO".into());
    assert!(codes(&ids(IFC2X3, vec![wall.into()], vec![])).is_empty());
}

#[test]
fn a_required_entity_must_be_one_the_applicability_selects() {
    let found = codes(&ids(IFC4, vec![entity("IFCSLAB")], vec![entity("IFCWALL")]));
    assert_eq!(
        found,
        [(
            AuditCode::EntityRequirementContradictsApplicability,
            Some(IfcVersion::Ifc4)
        )]
    );
    let either = Entity::new(Restriction::enumeration(["IFCWALL", "IFCSLAB"])).into();
    assert!(codes(&ids(IFC4, vec![entity("IFCSLAB")], vec![either])).is_empty());
}

#[test]
fn ifc2x3_mapped_names_overlap_their_occurrence_class() {
    // openbimrs/ids#15: checkable exactly in IFC2X3, in both directions.
    for (applies, requires) in [
        ("IFCFLOWTERMINAL", "IFCAIRTERMINAL"),
        ("IFCAIRTERMINAL", "IFCFLOWTERMINAL"),
    ] {
        let found = codes(&ids(IFC2X3, vec![entity(applies)], vec![entity(requires)]));
        assert!(found.is_empty(), "{applies} -> {requires}: {found:?}");
    }
    // IFC4 defines IfcAirTerminal itself: no overlap with IfcFlowTerminal.
    let found = codes(&ids(
        IFC4,
        vec![entity("IFCFLOWTERMINAL")],
        vec![entity("IFCAIRTERMINAL")],
    ));
    assert_eq!(
        found,
        [(
            AuditCode::EntityRequirementContradictsApplicability,
            Some(IfcVersion::Ifc4)
        )]
    );
    // A different occurrence class, or two mapped names sharing one, stay
    // contradictions.
    for (applies, requires) in [("IFCWALL", "IFCAIRTERMINAL"), ("IFCLAMP", "IFCAIRTERMINAL")] {
        let found = codes(&ids(IFC2X3, vec![entity(applies)], vec![entity(requires)]));
        assert_eq!(
            found,
            [(
                AuditCode::EntityRequirementContradictsApplicability,
                Some(IfcVersion::Ifc2x3)
            )],
            "{applies} -> {requires}"
        );
    }
}

#[test]
fn attributes_must_be_explicit_attributes_of_the_entity() {
    let found = codes(&ids(
        IFC4,
        vec![entity("IFCWALL")],
        vec![attribute("ActingRole", None)],
    ));
    assert_eq!(
        found,
        [(AuditCode::AttributeUnknown, Some(IfcVersion::Ifc4))]
    );
    // Inverse attributes are not explicit either.
    let found = codes(&ids(
        IFC4,
        vec![entity("IFCPERSON")],
        vec![attribute("EngagedIn", None)],
    ));
    assert_eq!(
        found,
        [(AuditCode::AttributeUnknown, Some(IfcVersion::Ifc4))]
    );
    // Attribute names are case-sensitive.
    let found = codes(&ids(
        IFC4,
        vec![entity("IFCWALL")],
        vec![attribute("name", None)],
    ));
    assert_eq!(
        found,
        [(AuditCode::AttributeUnknown, Some(IfcVersion::Ifc4))]
    );
    let inherited = attribute("Name", None);
    assert!(codes(&ids(IFC4, vec![entity("IFCWALL")], vec![inherited])).is_empty());
}

#[test]
fn derived_attributes_are_reported() {
    let found = codes(&ids(
        IFC4,
        vec![entity("IFCCARTESIANPOINT")],
        vec![attribute("Dim", None)],
    ));
    assert_eq!(
        found,
        [(AuditCode::AttributeDerived, Some(IfcVersion::Ifc4))]
    );
}

#[test]
fn attribute_patterns_must_match_an_attribute() {
    let found = codes(&ids(
        IFC4,
        vec![entity("IFCWALL")],
        vec![attribute(Restriction::pattern("Zz.*"), None)],
    ));
    assert_eq!(
        found,
        [(
            AuditCode::AttributePatternMatchesNothing,
            Some(IfcVersion::Ifc4)
        )]
    );
}

#[test]
fn values_are_only_required_of_simple_attributes() {
    for (entity_name, name) in [
        ("IFCCARTESIANPOINT", "Coordinates"),          // a list
        ("IFCTASK", "TaskTime"),                       // an entity
        ("IFCSURFACESTYLERENDERING", "DiffuseColour"), // a select
    ] {
        let found = codes(&ids(
            IFC4,
            vec![entity(entity_name)],
            vec![attribute(name, Some("Foobar".into()))],
        ));
        assert_eq!(
            found,
            [(
                AuditCode::AttributeValueNotComparable,
                Some(IfcVersion::Ifc4)
            )],
            "{name}"
        );
    }
}

#[test]
fn values_must_cast_to_the_attribute_type() {
    let risers = |value: Value| {
        codes(&ids(
            IFC4,
            vec![entity("IFCSTAIRFLIGHT")],
            vec![attribute("NumberOfRisers", Some(value))],
        ))
    };
    for bad in ["42.0", "42.", "forty"] {
        assert_eq!(
            risers(bad.into()),
            [(AuditCode::ValueTypeMismatch, Some(IfcVersion::Ifc4))],
            "{bad}"
        );
    }
    assert!(risers("42".into()).is_empty());
    // A name pattern resolves to the attribute whose type is then checked.
    let found = codes(&ids(
        IFC4,
        vec![entity("IFCSTAIRFLIGHT")],
        vec![attribute(
            Restriction::pattern("NumberOfRiser(s)?"),
            Some("42.3".into()),
        )],
    ));
    assert_eq!(
        found,
        [(AuditCode::ValueTypeMismatch, Some(IfcVersion::Ifc4))]
    );
    // Enumeration values and bounds are checked too.
    let found = risers(
        Restriction {
            min_inclusive: Some("1.5".into()),
            ..Restriction::enumeration(["2", "x"])
        }
        .into(),
    );
    assert_eq!(found.len(), 2, "{found:?}");
}

#[test]
fn property_values_must_cast_to_the_data_type() {
    let flag = |value: &str| {
        codes(&ids(
            IFC4,
            vec![entity("IFCWALL")],
            vec![property(
                "Foo_Bar",
                "Foo",
                Some("IFCBOOLEAN"),
                Some(value.into()),
            )],
        ))
    };
    assert_eq!(
        flag("FALSE"),
        [(AuditCode::ValueTypeMismatch, Some(IfcVersion::Ifc4))]
    );
    assert!(flag("false").is_empty());
    let real = codes(&ids(
        IFC4,
        vec![entity("IFCWALL")],
        vec![property(
            "Foo_Bar",
            "Foo",
            Some("IFCREAL"),
            Some("42,3".into()),
        )],
    ));
    assert_eq!(
        real,
        [(AuditCode::ValueTypeMismatch, Some(IfcVersion::Ifc4))]
    );
}

#[test]
fn patterns_only_apply_to_strings() {
    let found = codes(&ids(
        IFC4,
        vec![entity("IFCSURFACESTYLEREFRACTION")],
        vec![attribute(
            "RefractionIndex",
            Some(Restriction::pattern(".*").into()),
        )],
    ));
    assert_eq!(
        found,
        [(AuditCode::PatternOnNonString, Some(IfcVersion::Ifc4))]
    );
    let on_string = attribute("Name", Some(Restriction::pattern(".*").into()));
    assert!(codes(&ids(IFC4, vec![entity("IFCWALL")], vec![on_string])).is_empty());
}

#[test]
fn patterns_the_audit_cannot_evaluate_are_warnings() {
    let findings = audit(&ids(
        IFC4,
        vec![entity("IFCWALL")],
        vec![attribute(
            "Name",
            Some(Restriction::pattern("\\i\\c*").into()),
        )],
    ));
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].code, AuditCode::PatternUnverified);
    assert_eq!(findings[0].severity(), Severity::Warning);
}

#[test]
fn contradictory_restrictions_are_reported() {
    for restriction in [
        Restriction {
            min_inclusive: Some("5".into()),
            max_inclusive: Some("1".into()),
            ..Restriction::new("double")
        },
        Restriction {
            min_exclusive: Some("1".into()),
            max_exclusive: Some("1".into()),
            ..Restriction::new("double")
        },
        Restriction {
            min_length: Some(3),
            max_length: Some(1),
            ..Restriction::new("string")
        },
    ] {
        let found = codes(&ids(
            IFC4,
            vec![entity("IFCWALL")],
            vec![property(
                "Foo_Bar",
                "Foo",
                None,
                Some(restriction.clone().into()),
            )],
        ));
        assert_eq!(
            found,
            [(AuditCode::RestrictionBoundsContradict, None)],
            "{restriction:?}"
        );
    }
    let touching = Restriction {
        min_inclusive: Some("1".into()),
        max_inclusive: Some("1".into()),
        ..Restriction::new("double")
    };
    let found = codes(&ids(
        IFC4,
        vec![entity("IFCWALL")],
        vec![property("Foo_Bar", "Foo", None, Some(touching.into()))],
    ));
    assert!(found.is_empty());
}

#[test]
fn data_types_must_be_ifc_types_of_the_release() {
    let found = codes(&ids(
        IFC4,
        vec![entity("IFCWALL")],
        vec![property("Foo_Bar", "Foo", Some("IFCRABBITMEASURE"), None)],
    ));
    assert_eq!(
        found,
        [(AuditCode::DataTypeUnknown, Some(IfcVersion::Ifc4))]
    );
    // Enumeration types are allowed for predefined properties.
    let found = codes(&ids(
        IFC4,
        vec![entity("IFCDOOR")],
        vec![property(
            "Foo_Bar",
            "Foo",
            Some("IFCDOORPANELOPERATIONENUM"),
            None,
        )],
    ));
    assert!(found.is_empty());
}

#[test]
#[cfg(feature = "audit")]
fn standard_sets_are_checked_against_their_templates() {
    let check = |set: &str, name: &str, data_type: Option<&str>| {
        codes(&ids(
            IFC4,
            vec![entity("IFCWALL")],
            vec![property(set, name, data_type, None)],
        ))
    };
    assert!(check("Pset_WallCommon", "IsExternal", Some("IFCBOOLEAN")).is_empty());
    assert!(check("Qto_WallBaseQuantities", "Length", Some("IFCLENGTHMEASURE")).is_empty());
    assert_eq!(
        check("Pset_WallCommon", "Rabbit", None),
        [(AuditCode::PropertyNotInStandardSet, Some(IfcVersion::Ifc4))]
    );
    assert_eq!(
        check("Pset_WallCommon", "IsExternal", Some("IFCLABEL")),
        [(AuditCode::PropertyDataTypeMismatch, Some(IfcVersion::Ifc4))]
    );
    assert_eq!(
        check("Qto_WallBaseQuantities", "Length", Some("IFCAREAMEASURE")),
        [(AuditCode::PropertyDataTypeMismatch, Some(IfcVersion::Ifc4))]
    );
    assert_eq!(
        check("Pset_RabbitCommon", "Ears", None),
        [(AuditCode::StandardSetUnknown, Some(IfcVersion::Ifc4))]
    );
    // Custom sets are not judged.
    assert!(check("ACME_Common", "Anything", None).is_empty());
}

#[test]
fn part_of_relations_need_a_fitting_whole() {
    let part_of = |relation, whole: &str| -> Facet {
        PartOf {
            relation: Some(relation),
            ..PartOf::new(Entity::new(whole))
        }
        .into()
    };
    let found = codes(&ids(
        IFC4,
        vec![entity("IFCWALL")],
        vec![part_of(Relation::AssignsToGroup, "IFCWALL")],
    ));
    assert_eq!(
        found,
        [(AuditCode::PartOfRelationEntity, Some(IfcVersion::Ifc4))]
    );
    for (relation, whole) in [
        (Relation::AssignsToGroup, "IFCGROUP"),
        (Relation::ContainedInSpatialStructure, "IFCBUILDINGSTOREY"),
        (Relation::Aggregates, "IFCELEMENTASSEMBLY"),
        (Relation::VoidsElementFillsElement, "IFCWALL"),
    ] {
        for versions in [IFC2X3, IFC4] {
            let found = codes(&ids(
                versions,
                vec![entity("IFCDOOR")],
                vec![part_of(relation, whole)],
            ));
            assert!(found.is_empty(), "{relation} {whole}: {found:?}");
        }
    }
}

#[test]
fn prohibited_specifications_cannot_have_requirements() {
    let mut ids = ids(IFC4, vec![entity("IFCWALL")], vec![attribute("Name", None)]);
    ids.specifications[0]
        .applicability
        .set_occurrence(openbim_ids::Occurrence::Prohibited);
    assert_eq!(codes(&ids), [(AuditCode::ProhibitedWithRequirements, None)]);
}

#[test]
fn occurrence_bounds_must_not_contradict() {
    let mut ids = ids(IFC4, vec![entity("IFCWALL")], vec![]);
    let applicability = &mut ids.specifications[0].applicability;
    (applicability.min_occurs, applicability.max_occurs) = (2, Some(1));
    assert_eq!(codes(&ids), [(AuditCode::OccurrenceBoundsContradict, None)]);
}

#[test]
fn findings_locate_the_offending_facet() {
    let mut ids = ids(IFC4, vec![entity("IFCWALL")], vec![]);
    ids.specifications.push(ids.specifications[0].clone());
    ids.specifications[1].require(attribute("ActingRole", None));
    let findings = audit(&ids);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].specification, 1);
    assert_eq!(findings[0].path, "requirements/facets[0]/name");
    assert_eq!(
        findings[0].to_string(),
        "error [attribute-unknown] specifications[1]/requirements/facets[0]/name (IFC4): \
         IFCWALL has no explicit attribute ActingRole in IFC4"
    );
}

/// Without the template catalog, standard sets are not judged at all.
#[test]
#[cfg(not(feature = "audit"))]
fn standard_sets_are_not_judged_without_templates() {
    let found = codes(&ids(
        IFC4,
        vec![entity("IFCWALL")],
        vec![property(
            "Pset_RabbitCommon",
            "Ears",
            Some("IFCLABEL"),
            None,
        )],
    ));
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn codes_are_stable_and_distinct() {
    let codes = [
        AuditCode::EntityNameCase,
        AuditCode::EntityUnknown,
        AuditCode::EntityPatternMatchesNothing,
        AuditCode::PredefinedTypeUnavailable,
        AuditCode::EntityRequirementContradictsApplicability,
        AuditCode::AttributeUnknown,
        AuditCode::AttributeDerived,
        AuditCode::AttributePatternMatchesNothing,
        AuditCode::AttributeValueNotComparable,
        AuditCode::ValueTypeMismatch,
        AuditCode::PatternOnNonString,
        AuditCode::PatternUnverified,
        AuditCode::RestrictionBoundsContradict,
        AuditCode::DataTypeUnknown,
        AuditCode::PropertyNotInStandardSet,
        AuditCode::PropertyDataTypeMismatch,
        AuditCode::StandardSetUnknown,
        AuditCode::PartOfRelationEntity,
        AuditCode::ProhibitedWithRequirements,
        AuditCode::OccurrenceBoundsContradict,
    ];
    let strings: std::collections::BTreeSet<&str> = codes.iter().map(|c| c.as_str()).collect();
    assert_eq!(strings.len(), codes.len());
    assert!(strings
        .iter()
        .all(|s| s.bytes().all(|b| b.is_ascii_lowercase() || b == b'-')));
    assert_eq!(AuditCode::EntityUnknown.as_str(), "entity-unknown");
}
