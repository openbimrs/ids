//! Auditing IDS documents against the IFC schemas of their listed releases.
//!
//! A schema-valid IDS document can still be meaningless for the releases it
//! names: an entity the release does not define, an attribute the entity
//! does not have, a value that no IFC value of the attribute's type could
//! equal. A checker finds those late, as silent non-matches. [`audit`] finds
//! them up front, per specification and per listed `ifcVersion`.
//!
//! # Features
//!
//! - `audit-schema` runs every check against the IFC schema tables of
//!   `ifc-schema` (AGPL-3.0-or-later).
//! - `audit` adds the standard property and quantity set checks
//!   ([`AuditCode::PropertyNotInStandardSet`],
//!   [`AuditCode::PropertyDataTypeMismatch`],
//!   [`AuditCode::StandardSetUnknown`]) against the official templates of
//!   `ifc-template-catalog`, which embeds buildingSMART template data under
//!   CC BY-ND 4.0 in addition to AGPL-3.0-or-later.
//!
//! Enabling either puts those terms on the resulting work. The reader and
//! writer need neither.
//!
//! # What is checked
//!
//! Each [`AuditCode`] documents its rule. In short:
//!
//! - entity names: upper case, defined by every listed release (IFC2X3 also
//!   accepts the IFC4 names of the implementers' occurrence/type mapping
//!   table), and a pattern matches at least one;
//! - a predefined type is only stated for an entity that has one, itself or
//!   through its type entity. Its value is not checked against the
//!   enumeration: a user-defined value matches `ObjectType`;
//! - a requirement `entity` can be met by an object the applicability
//!   selects;
//! - attribute names are explicit attributes of the applicable entity, not
//!   derived or inverse ones, and a value is only required of an attribute
//!   holding a single simple value;
//! - values can be cast to the IFC type they are compared with: an
//!   attribute's declared type or a property's `dataType`. `xs:pattern` only
//!   applies to strings;
//! - property `dataType`s are IFC defined (or enumeration) types of the
//!   release; with `audit`, in a standard `Pset_`/`Qto_` set the property
//!   exists and has that type;
//! - `partOf` names a whole the relation can have;
//! - a prohibited specification carries no requirements, and occurrence and
//!   restriction bounds are not contradictory.
//!
//! Findings for one release carry it in [`AuditFinding::ifc_version`]; a
//! document listing `IFC2X3 IFC4` can be wrong for only one of them.
//!
//! # What is not checked
//!
//! An entity is not required to be an `IfcObject` or type: attribute facets
//! legitimately apply to resource entities such as `IfcPerson`. Enumerated
//! attribute values and patterns that use XSD-only constructs (`\i`, `\c`,
//! class subtraction, `\p{Is…}`) are not evaluated; the latter are reported
//! as [`AuditCode::PatternUnverified`] warnings.

mod mapping;
mod pattern;

use core::fmt;
use std::collections::BTreeSet;
use std::sync::OnceLock;

use ifc_schema::{Schema, TypeKind};
#[cfg(feature = "audit")]
use ifc_template_catalog::catalog::Catalog;
#[cfg(feature = "audit")]
use ifc_template_catalog::definition::{
    CatalogEdition, PropertyKind, QuantityKind, SetTemplateKind,
};

use crate::model::{
    Attribute, Entity, Facet, PartOf, Property, Relation, Restriction, Specification, Value,
};
use crate::{Ids, IfcVersion};

/// How serious a finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// The specification cannot work as written for the release.
    Error,
    /// The audit could not decide, or the document is suspicious.
    Warning,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        })
    }
}

/// What a finding is about. [`AuditCode::as_str`] is the stable code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum AuditCode {
    /// `entity-name-case`: an entity name is not upper case. IDS compares
    /// entity names case-sensitively against the upper-case IFC name.
    EntityNameCase,
    /// `entity-unknown`: the release does not define the entity.
    EntityUnknown,
    /// `entity-pattern-matches-nothing`: no entity of the release matches.
    EntityPatternMatchesNothing,
    /// `predefined-type-unavailable`: a predefined type is stated for an
    /// entity that has none, neither itself nor through its type entity.
    PredefinedTypeUnavailable,
    /// `entity-requirement-contradicts-applicability`: no object the
    /// applicability selects can be the entity the requirement demands.
    /// Subtypes do not count; IDS matches entity names exactly. In IFC2X3 a
    /// name of the occurrence/type mapping table overlaps its occurrence
    /// class (`IFCAIRTERMINAL` is an `IFCFLOWTERMINAL` with an
    /// `IFCAIRTERMINALTYPE`).
    EntityRequirementContradictsApplicability,
    /// `attribute-unknown`: the applicable entity has no such explicit
    /// attribute (inverse attributes are not checkable).
    AttributeUnknown,
    /// `attribute-derived`: the attribute is derived, so a model never
    /// stores it.
    AttributeDerived,
    /// `attribute-pattern-matches-nothing`: no attribute of the applicable
    /// entity matches the name pattern.
    AttributePatternMatchesNothing,
    /// `attribute-value-not-comparable`: a value is required of an attribute
    /// holding a list, an entity or a select, which never equals a literal.
    AttributeValueNotComparable,
    /// `value-type-mismatch`: a value cannot be cast to the IFC type it is
    /// compared with, e.g. `42.0` for an integer or `FALSE` for a boolean.
    ValueTypeMismatch,
    /// `pattern-on-non-string`: an `xs:pattern` is applied to a non-string
    /// value, which never matches.
    PatternOnNonString,
    /// `pattern-unverified` (warning): the pattern uses an XSD construct this
    /// audit cannot evaluate, so checks depending on it were skipped.
    PatternUnverified,
    /// `restriction-bounds-contradict`: the restriction's bounds or lengths
    /// admit no value.
    RestrictionBoundsContradict,
    /// `data-type-unknown`: a property `dataType` is not an IFC defined or
    /// enumeration type of the release.
    DataTypeUnknown,
    /// `property-not-in-standard-set`: a standard property or quantity set of
    /// the release does not define the property.
    PropertyNotInStandardSet,
    /// `property-data-type-mismatch`: the standard set defines the property
    /// with a different data type.
    PropertyDataTypeMismatch,
    /// `standard-set-unknown`: a set named with the reserved `Pset_` or
    /// `Qto_` prefix is not a standard set of the release.
    StandardSetUnknown,
    /// `part-of-relation-entity`: the relation cannot have this entity as
    /// its whole, e.g. `IFCRELASSIGNSTOGROUP` with a non-group.
    PartOfRelationEntity,
    /// `prohibited-with-requirements`: a prohibited specification
    /// (`maxOccurs="0"`) has requirements, which can never be checked.
    ProhibitedWithRequirements,
    /// `occurrence-bounds-contradict`: `minOccurs` exceeds `maxOccurs`.
    OccurrenceBoundsContradict,
}

impl AuditCode {
    /// The stable code, e.g. `entity-unknown`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            AuditCode::EntityNameCase => "entity-name-case",
            AuditCode::EntityUnknown => "entity-unknown",
            AuditCode::EntityPatternMatchesNothing => "entity-pattern-matches-nothing",
            AuditCode::PredefinedTypeUnavailable => "predefined-type-unavailable",
            AuditCode::EntityRequirementContradictsApplicability => {
                "entity-requirement-contradicts-applicability"
            }
            AuditCode::AttributeUnknown => "attribute-unknown",
            AuditCode::AttributeDerived => "attribute-derived",
            AuditCode::AttributePatternMatchesNothing => "attribute-pattern-matches-nothing",
            AuditCode::AttributeValueNotComparable => "attribute-value-not-comparable",
            AuditCode::ValueTypeMismatch => "value-type-mismatch",
            AuditCode::PatternOnNonString => "pattern-on-non-string",
            AuditCode::PatternUnverified => "pattern-unverified",
            AuditCode::RestrictionBoundsContradict => "restriction-bounds-contradict",
            AuditCode::DataTypeUnknown => "data-type-unknown",
            AuditCode::PropertyNotInStandardSet => "property-not-in-standard-set",
            AuditCode::PropertyDataTypeMismatch => "property-data-type-mismatch",
            AuditCode::StandardSetUnknown => "standard-set-unknown",
            AuditCode::PartOfRelationEntity => "part-of-relation-entity",
            AuditCode::ProhibitedWithRequirements => "prohibited-with-requirements",
            AuditCode::OccurrenceBoundsContradict => "occurrence-bounds-contradict",
        }
    }

    /// The severity findings with this code have.
    #[must_use]
    pub fn severity(self) -> Severity {
        match self {
            AuditCode::PatternUnverified => Severity::Warning,
            _ => Severity::Error,
        }
    }
}

impl fmt::Display for AuditCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One problem in a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditFinding {
    /// What is wrong.
    pub code: AuditCode,
    /// The index of the specification in [`Ids::specifications`].
    pub specification: usize,
    /// The path to the offending part within the specification, such as
    /// `requirements/facets[1]/name`. Empty for the specification itself.
    pub path: String,
    /// The release the finding applies to, `None` when it holds for all.
    pub ifc_version: Option<IfcVersion>,
    /// A human-readable explanation.
    pub message: String,
}

impl AuditFinding {
    /// The finding's severity, see [`AuditCode::severity`].
    #[must_use]
    pub fn severity(&self) -> Severity {
        self.code.severity()
    }
}

impl fmt::Display for AuditFinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} [{}] specifications[{}]",
            self.severity(),
            self.code,
            self.specification
        )?;
        if !self.path.is_empty() {
            write!(f, "/{}", self.path)?;
        }
        if let Some(version) = self.ifc_version {
            write!(f, " ({version})")?;
        }
        write!(f, ": {}", self.message)
    }
}

/// Audits every specification of `ids` against its listed IFC releases.
///
/// Findings come in document order. An empty result means no problem was
/// found, not that the document is fit for every purpose.
///
/// ```
/// use openbim_ids::audit::{audit, AuditCode};
/// use openbim_ids::{Entity, Ids, IfcVersion, Info, Specification};
///
/// let mut spec = Specification::new("Rabbits", [IfcVersion::Ifc4]);
/// spec.applicability.facets.push(Entity::new("IFCRABBIT").into());
/// let mut ids = Ids::new(Info::new("Zoo"));
/// ids.specifications.push(spec);
///
/// let findings = audit(&ids);
/// assert_eq!(findings[0].code, AuditCode::EntityUnknown);
/// ```
#[must_use]
pub fn audit(ids: &Ids) -> Vec<AuditFinding> {
    let mut auditor = Auditor::default();
    for (index, specification) in ids.specifications.iter().enumerate() {
        auditor.specification = index;
        auditor.audit_specification(specification);
    }
    auditor.findings
}

/// One IFC release: its schema, its standard sets, and its entity names.
struct Release {
    version: IfcVersion,
    schema: &'static Schema,
    #[cfg(feature = "audit")]
    templates: Option<Catalog>,
    /// Upper-case entity names, plus for IFC2X3 the mapped IFC4 names.
    entities: Vec<String>,
}

fn release(version: IfcVersion) -> &'static Release {
    static RELEASES: [OnceLock<Release>; 3] = [OnceLock::new(), OnceLock::new(), OnceLock::new()];
    let (slot, schema): (usize, fn() -> &'static Schema) = match version {
        IfcVersion::Ifc2x3 => (0, ifc_schema::ifc2x3),
        IfcVersion::Ifc4 => (1, ifc_schema::ifc4),
        IfcVersion::Ifc4x3Add2 => (2, ifc_schema::ifc4x3),
    };
    RELEASES[slot].get_or_init(|| {
        let schema = schema();
        let mut entities: BTreeSet<String> =
            schema.entity_names().map(str::to_ascii_uppercase).collect();
        if version == IfcVersion::Ifc2x3 {
            entities.extend(
                mapping::IFC2X3_OCCURRENCE_TYPES
                    .iter()
                    .map(|(name, _, _)| (*name).to_owned()),
            );
        }
        Release {
            version,
            schema,
            #[cfg(feature = "audit")]
            templates: templates(version),
            entities: entities.into_iter().collect(),
        }
    })
}

/// The official standard set templates of a release.
#[cfg(feature = "audit")]
fn templates(version: IfcVersion) -> Option<Catalog> {
    let edition = match version {
        IfcVersion::Ifc2x3 => CatalogEdition::Ifc2x3Tc1,
        IfcVersion::Ifc4 => CatalogEdition::Ifc4Add2Tc1,
        IfcVersion::Ifc4x3Add2 => CatalogEdition::Ifc4x3Add2,
    };
    ifc_template_catalog::embedded::official_catalog(edition).ok()
}

impl Release {
    /// Whether one object can match both entity names exactly.
    ///
    /// Equal names can. In IFC2X3, a name of the occurrence/type mapping
    /// table is an occurrence class narrowed by its type entity, so it and
    /// that occurrence class (`IFCAIRTERMINAL`, `IFCFLOWTERMINAL`) describe
    /// overlapping objects. Two mapped names never do, even with a shared
    /// occurrence class: their type entities differ.
    fn can_be(&self, one: &str, other: &str) -> bool {
        if one == other {
            return true;
        }
        if self.version != IfcVersion::Ifc2x3 {
            return false;
        }
        let occurrence = |name: &str| {
            mapping::IFC2X3_OCCURRENCE_TYPES
                .iter()
                .find(|(mapped, _, _)| *mapped == name)
                .map(|(_, occurrence, _)| *occurrence)
        };
        match (occurrence(one), occurrence(other)) {
            (Some(occurrence), None) => occurrence == other,
            (None, Some(occurrence)) => occurrence == one,
            _ => false,
        }
    }

    fn has_entity(&self, name: &str) -> bool {
        self.entities
            .binary_search_by(|e| e.as_str().cmp(name))
            .is_ok()
    }

    /// The schema entity an IDS entity name stands for, and its type entity.
    fn resolve(&self, name: &str) -> Option<(String, Option<String>)> {
        if self.version == IfcVersion::Ifc2x3 {
            if let Some((_, occurrence, type_entity)) = mapping::IFC2X3_OCCURRENCE_TYPES
                .iter()
                .find(|(mapped, _, _)| *mapped == name)
            {
                return Some(((*occurrence).to_owned(), Some((*type_entity).to_owned())));
            }
        }
        let entity = self.schema.entity(name)?;
        let type_entity = format!("{name}TYPE");
        let type_entity = (!name.ends_with("TYPE") && self.schema.entity(&type_entity).is_some())
            .then_some(type_entity);
        Some((entity.name.to_ascii_uppercase(), type_entity))
    }

    fn has_attribute(&self, entity: &str, attribute: &str) -> bool {
        self.schema.attribute_names(entity).contains(&attribute)
    }

    fn has_predefined_type(&self, name: &str) -> bool {
        self.resolve(name).is_some_and(|(occurrence, type_entity)| {
            self.has_attribute(&occurrence, "PredefinedType")
                || type_entity.is_some_and(|t| self.has_attribute(&t, "PredefinedType"))
        })
    }

    /// Whether `attribute` is derived in `entity` or a supertype.
    fn is_derived(&self, entity: &str, attribute: &str) -> bool {
        std::iter::once(entity)
            .chain(self.schema.supertypes(entity))
            .filter_map(|name| self.schema.entity(name))
            .any(|def| def.derived.iter().any(|d| d == attribute))
    }

    fn is_defined_type(&self, name: &str) -> bool {
        self.schema
            .type_def(name)
            .is_some_and(|def| matches!(def.kind, TypeKind::Defined(_) | TypeKind::Enumeration(_)))
    }

    /// What a value compared with the named IFC type must look like.
    fn kind_of_type(&self, type_name: &str) -> Kind {
        if self.schema.entity(type_name).is_some() {
            return Kind::NotComparable("an entity");
        }
        match self.schema.type_def(type_name).map(|def| &def.kind) {
            Some(TypeKind::Select(_)) => return Kind::NotComparable("a select"),
            Some(TypeKind::Enumeration(_)) => return Kind::Unchecked,
            _ => {}
        }
        let base = self.schema.resolve_defined(type_name).to_ascii_uppercase();
        match base.as_str() {
            "INTEGER" => Kind::Integer,
            "REAL" | "NUMBER" => Kind::Real,
            "BOOLEAN" => Kind::Boolean,
            "LOGICAL" => Kind::Logical,
            "STRING" | "BINARY" => Kind::String,
            other
                if other.starts_with("LIST")
                    || other.starts_with("SET")
                    || other.starts_with("ARRAY")
                    || other.starts_with("BAG") =>
            {
                Kind::NotComparable("a list")
            }
            _ => match self.schema.type_def(&base).map(|def| &def.kind) {
                Some(TypeKind::Select(_)) => Kind::NotComparable("a select"),
                _ => Kind::Unchecked,
            },
        }
    }
}

/// What literal values an IFC type can equal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Integer,
    Real,
    Boolean,
    Logical,
    String,
    /// An enumeration or a type this audit does not judge.
    Unchecked,
    /// Lists, entities and selects never equal a literal.
    NotComparable(&'static str),
}

impl Kind {
    fn accepts(self, value: &str) -> bool {
        let value = value.trim_matches(|c| matches!(c, ' ' | '\t' | '\n' | '\r'));
        match self {
            Kind::Integer => is_xs_integer(value),
            Kind::Real => is_xs_double(value),
            Kind::Boolean => matches!(value, "true" | "false"),
            Kind::Logical => matches!(value, "true" | "false" | "unknown"),
            Kind::String | Kind::Unchecked | Kind::NotComparable(_) => true,
        }
    }

    fn is_string(self) -> bool {
        matches!(self, Kind::String | Kind::Unchecked)
    }

    fn describe(self) -> &'static str {
        match self {
            Kind::Integer => "an integer",
            Kind::Real => "a real number",
            Kind::Boolean => "a boolean (true or false)",
            Kind::Logical => "a logical (true, false or unknown)",
            Kind::String => "a string",
            Kind::Unchecked => "an enumerated value",
            Kind::NotComparable(what) => what,
        }
    }
}

/// The names a value admits, as far as the audit can tell.
enum Names {
    /// A `simpleValue` or an enumeration: exactly these.
    Listed(Vec<String>),
    /// A pattern, filtered against a known universe: these matched.
    Matched(Vec<String>),
    /// Something the audit cannot enumerate.
    Unknown,
}

fn names<'u>(value: &Value, universe: impl Iterator<Item = &'u str>) -> Names {
    match value {
        Value::Simple(value) => Names::Listed(vec![value.clone()]),
        Value::Restriction(restriction) if !restriction.enumeration.is_empty() => {
            Names::Listed(restriction.enumeration.clone())
        }
        Value::Restriction(restriction) if !restriction.patterns.is_empty() => {
            let Ok(patterns) = restriction
                .patterns
                .iter()
                .map(|p| pattern::compile(p))
                .collect::<Result<Vec<_>, _>>()
            else {
                return Names::Unknown;
            };
            Names::Matched(
                universe
                    .filter(|name| patterns.iter().any(|p| p.is_match(name)))
                    .map(str::to_owned)
                    .collect(),
            )
        }
        Value::Restriction(_) => Names::Unknown,
    }
}

#[derive(Default)]
struct Auditor {
    findings: Vec<AuditFinding>,
    specification: usize,
}

impl Auditor {
    fn report(
        &mut self,
        code: AuditCode,
        path: &str,
        version: Option<IfcVersion>,
        message: String,
    ) {
        self.findings.push(AuditFinding {
            code,
            specification: self.specification,
            path: path.to_owned(),
            ifc_version: version,
            message,
        });
    }

    fn audit_specification(&mut self, spec: &Specification) {
        let applicability = &spec.applicability;
        if applicability
            .max_occurs
            .is_some_and(|max| max < applicability.min_occurs)
        {
            self.report(
                AuditCode::OccurrenceBoundsContradict,
                "applicability",
                None,
                format!(
                    "minOccurs {} exceeds maxOccurs {}",
                    applicability.min_occurs,
                    applicability.max_occurs.unwrap_or_default()
                ),
            );
        }
        if applicability.max_occurs == Some(0)
            && spec
                .requirements
                .as_ref()
                .is_some_and(|r| !r.facets.is_empty())
        {
            self.report(
                AuditCode::ProhibitedWithRequirements,
                "requirements",
                None,
                "a prohibited specification fails on any applicable object, so its \
                 requirements are never checked"
                    .into(),
            );
        }

        // Release-independent checks, once per facet.
        let facets = facets(spec);
        for (path, facet) in &facets {
            self.facet_independent(path, facet);
        }

        // Release-dependent checks.
        for &version in &spec.ifc_versions {
            let release = release(version);
            let applicable = applicability.facets.iter().find_map(|facet| match facet {
                Facet::Entity(entity) => Some(entity),
                _ => None,
            });
            let applicable = applicable
                .and_then(|entity| self.entity(release, "applicability/facets[0]", entity))
                .map(|(names, _)| names);
            for (path, facet) in &facets {
                match facet {
                    Facet::Entity(entity) if path.starts_with("requirements") => {
                        let required = self.entity(release, path, entity).map(|(names, _)| names);
                        if let (Some(required), Some(applicable)) = (&required, &applicable) {
                            if !required.is_empty()
                                && !applicable.is_empty()
                                && !required
                                    .iter()
                                    .any(|r| applicable.iter().any(|a| release.can_be(r, a)))
                            {
                                self.report(
                                    AuditCode::EntityRequirementContradictsApplicability,
                                    path,
                                    Some(version),
                                    format!(
                                        "requires {} but applies to {}; IDS does not match subtypes",
                                        alternatives(required),
                                        alternatives(applicable)
                                    ),
                                );
                            }
                        }
                    }
                    Facet::Entity(_) => {}
                    Facet::PartOf(part_of) => self.part_of(release, path, part_of),
                    Facet::Attribute(attribute) => {
                        self.attribute(release, path, attribute, applicable.as_deref());
                    }
                    Facet::Property(property) => self.property(release, path, property),
                    Facet::Classification(_) | Facet::Material(_) => {}
                }
            }
        }
    }

    fn facet_independent(&mut self, path: &str, facet: &Facet) {
        let entities: Vec<(&Entity, String)> = match facet {
            Facet::Entity(entity) => vec![(entity, path.to_owned())],
            Facet::PartOf(part_of) => vec![(&part_of.entity, format!("{path}/entity"))],
            _ => Vec::new(),
        };
        for (entity, entity_path) in entities {
            if let Names::Listed(names) = names(&entity.name, std::iter::empty()) {
                for name in names.iter().filter(|n| **n != n.to_ascii_uppercase()) {
                    self.report(
                        AuditCode::EntityNameCase,
                        &format!("{entity_path}/name"),
                        None,
                        format!(
                            "{name:?} is not upper case; IFC entity names are written as {:?}",
                            name.to_ascii_uppercase()
                        ),
                    );
                }
            }
        }
        for (value_path, value) in values(facet) {
            if let Value::Restriction(restriction) = value {
                self.restriction(&format!("{path}/{value_path}"), restriction);
            }
        }
    }

    /// Checks an entity facet against a release; returns the known names it
    /// can match, or `None` when they cannot be enumerated.
    fn entity(
        &mut self,
        release: &Release,
        path: &str,
        entity: &Entity,
    ) -> Option<(Vec<String>, bool)> {
        let version = Some(release.version);
        let known = match names(&entity.name, release.entities.iter().map(String::as_str)) {
            Names::Listed(names) => {
                let mut known = Vec::new();
                for name in names {
                    if name != name.to_ascii_uppercase() {
                        continue; // reported once as EntityNameCase
                    }
                    if release.has_entity(&name) {
                        known.push(name);
                    } else {
                        self.report(
                            AuditCode::EntityUnknown,
                            &format!("{path}/name"),
                            version,
                            format!("{} does not define {name}", release.version),
                        );
                    }
                }
                if entity.predefined_type.is_some() {
                    for name in &known {
                        if !release.has_predefined_type(name) {
                            self.report(
                                AuditCode::PredefinedTypeUnavailable,
                                &format!("{path}/predefinedType"),
                                version,
                                format!(
                                    "{name} has no PredefinedType in {}, neither itself nor \
                                     through a type entity",
                                    release.version
                                ),
                            );
                        }
                    }
                }
                (known, true)
            }
            Names::Matched(names) => {
                if names.is_empty() {
                    self.report(
                        AuditCode::EntityPatternMatchesNothing,
                        &format!("{path}/name"),
                        version,
                        format!("the pattern matches no entity of {}", release.version),
                    );
                }
                (names, false)
            }
            Names::Unknown => return None,
        };
        Some(known)
    }

    fn attribute(
        &mut self,
        release: &Release,
        path: &str,
        attribute: &Attribute,
        applicable: Option<&[String]>,
    ) {
        let Some(applicable) = applicable.filter(|a| !a.is_empty()) else {
            return;
        };
        let version = Some(release.version);
        let entities: Vec<String> = applicable
            .iter()
            .filter_map(|name| release.resolve(name).map(|(occurrence, _)| occurrence))
            .collect();
        let mut universe: BTreeSet<&str> = BTreeSet::new();
        for entity in &entities {
            universe.extend(release.schema.attribute_names(entity));
        }
        let name_path = format!("{path}/name");
        let matched: Vec<String> = match names(&attribute.name, universe.iter().copied()) {
            Names::Listed(names) => names
                .into_iter()
                .filter(|name| {
                    let explicit = entities
                        .iter()
                        .any(|e| release.has_attribute(e, name) && !release.is_derived(e, name));
                    if !explicit {
                        let derived = entities.iter().any(|e| release.is_derived(e, name));
                        let (code, message) = if derived {
                            (
                                AuditCode::AttributeDerived,
                                format!("{name} is derived on {}", alternatives(applicable)),
                            )
                        } else {
                            (
                                AuditCode::AttributeUnknown,
                                format!(
                                    "{} has no explicit attribute {name} in {}",
                                    alternatives(applicable),
                                    release.version
                                ),
                            )
                        };
                        self.report(code, &name_path, version, message);
                    }
                    explicit
                })
                .collect(),
            Names::Matched(names) => {
                let names: Vec<String> = names
                    .into_iter()
                    .filter(|n| !entities.iter().any(|e| release.is_derived(e, n)))
                    .collect();
                if names.is_empty() {
                    self.report(
                        AuditCode::AttributePatternMatchesNothing,
                        &name_path,
                        version,
                        format!(
                            "the pattern matches no explicit attribute of {}",
                            alternatives(applicable)
                        ),
                    );
                }
                names
            }
            Names::Unknown => return,
        };
        let Some(value) = &attribute.value else {
            return;
        };
        let kinds: Vec<Kind> = matched
            .iter()
            .flat_map(|name| {
                entities.iter().flat_map(move |entity| {
                    release
                        .schema
                        .attributes(entity)
                        .into_iter()
                        .filter(move |a| a.name == *name)
                        .map(|a| {
                            if a.aggregate {
                                Kind::NotComparable("a list")
                            } else {
                                release.kind_of_type(&a.type_name)
                            }
                        })
                })
            })
            .collect();
        if kinds.is_empty() {
            return;
        }
        if let Some(Kind::NotComparable(what)) = kinds
            .iter()
            .copied()
            .find(|k| matches!(k, Kind::NotComparable(_)))
            .filter(|_| kinds.iter().all(|k| matches!(k, Kind::NotComparable(_))))
        {
            self.report(
                AuditCode::AttributeValueNotComparable,
                &format!("{path}/value"),
                version,
                format!("the attribute holds {what}, which never equals a value"),
            );
            return;
        }
        let comparable: Vec<Kind> = kinds
            .into_iter()
            .filter(|k| !matches!(k, Kind::NotComparable(_)))
            .collect();
        self.value(&format!("{path}/value"), version, value, &comparable);
    }

    fn property(&mut self, release: &Release, path: &str, property: &Property) {
        let version = Some(release.version);
        let mut kinds = Vec::new();
        if let Some(data_type) = &property.data_type {
            if release.is_defined_type(data_type) {
                kinds.push(release.kind_of_type(data_type));
            } else {
                self.report(
                    AuditCode::DataTypeUnknown,
                    &format!("{path}/@dataType"),
                    version,
                    format!("{data_type} is not a defined type of {}", release.version),
                );
            }
        }
        if let Some(value) = &property.value {
            self.value(&format!("{path}/value"), version, value, &kinds);
        }
        #[cfg(feature = "audit")]
        self.standard_set(release, path, property);
    }

    #[cfg(feature = "audit")]
    fn standard_set(&mut self, release: &Release, path: &str, property: &Property) {
        let Some(catalog) = &release.templates else {
            return;
        };
        let Names::Listed(sets) = names(&property.property_set, std::iter::empty()) else {
            return;
        };
        let version = Some(release.version);
        for set in sets
            .iter()
            .filter(|s| s.starts_with("Pset_") || s.starts_with("Qto_"))
        {
            let Some(template) = catalog.get(set) else {
                self.report(
                    AuditCode::StandardSetUnknown,
                    &format!("{path}/propertySet"),
                    version,
                    format!(
                        "{set} uses a reserved prefix but is not a standard set of {}",
                        release.version
                    ),
                );
                continue;
            };
            let members: Vec<(String, Option<String>)> = match &template.kind {
                SetTemplateKind::Property { properties, .. } => properties
                    .iter()
                    .map(|p| (p.name.clone(), property_data_type(&p.kind)))
                    .collect(),
                SetTemplateKind::Quantity { quantities, .. } => quantities
                    .iter()
                    .map(|q| (q.name.clone(), quantity_data_type(q.kind)))
                    .collect(),
                _ => continue,
            };
            let Names::Listed(base_names) = names(&property.base_name, std::iter::empty()) else {
                continue;
            };
            for base_name in base_names {
                match members.iter().find(|(name, _)| *name == base_name) {
                    None => self.report(
                        AuditCode::PropertyNotInStandardSet,
                        &format!("{path}/baseName"),
                        version,
                        format!("{set} of {} has no property {base_name}", release.version),
                    ),
                    Some((_, Some(expected))) => {
                        if let Some(data_type) = &property.data_type {
                            if !data_type.eq_ignore_ascii_case(expected) {
                                self.report(
                                    AuditCode::PropertyDataTypeMismatch,
                                    &format!("{path}/@dataType"),
                                    version,
                                    format!(
                                        "{set}.{base_name} is {} in {}, not {data_type}",
                                        expected.to_ascii_uppercase(),
                                        release.version
                                    ),
                                );
                            }
                        }
                    }
                    Some((_, None)) => {}
                }
            }
        }
    }

    fn part_of(&mut self, release: &Release, path: &str, part_of: &PartOf) {
        let entity_path = format!("{path}/entity");
        // Only named wholes are judged: a pattern may well match entities
        // the relation cannot have alongside ones it can.
        let Some((wholes, true)) = self.entity(release, &entity_path, &part_of.entity) else {
            return;
        };
        let Some(relation) = part_of.relation else {
            return;
        };
        let required = match (relation, release.version) {
            (Relation::Aggregates | Relation::Nests, _) => "IfcObjectDefinition",
            (Relation::AssignsToGroup, _) => "IfcGroup",
            (Relation::ContainedInSpatialStructure, IfcVersion::Ifc2x3) => {
                "IfcSpatialStructureElement"
            }
            (Relation::ContainedInSpatialStructure, _) => "IfcSpatialElement",
            (Relation::VoidsElementFillsElement, _) => "IfcElement",
        };
        for whole in &wholes {
            let Some((occurrence, _)) = release.resolve(whole) else {
                continue;
            };
            if !release.schema.is_a(&occurrence, required) {
                self.report(
                    AuditCode::PartOfRelationEntity,
                    &entity_path,
                    Some(release.version),
                    format!(
                        "{relation} needs a whole that is an {required}; {whole} is not in {}",
                        release.version
                    ),
                );
            }
        }
    }

    /// Checks `value` against the IFC kinds it is compared with; it must fit
    /// at least one of them.
    fn value(&mut self, path: &str, version: Option<IfcVersion>, value: &Value, kinds: &[Kind]) {
        if kinds.is_empty() {
            return;
        }
        let expected = || {
            kinds
                .iter()
                .map(|k| k.describe())
                .collect::<Vec<_>>()
                .join(" or ")
        };
        match value {
            Value::Simple(literal) => {
                if !kinds.iter().any(|k| k.accepts(literal)) {
                    self.report(
                        AuditCode::ValueTypeMismatch,
                        path,
                        version,
                        format!("{literal:?} is not {}", expected()),
                    );
                }
            }
            Value::Restriction(restriction) => {
                if !restriction.patterns.is_empty() && !kinds.iter().any(|k| k.is_string()) {
                    self.report(
                        AuditCode::PatternOnNonString,
                        path,
                        version,
                        format!("a pattern never matches {}", expected()),
                    );
                }
                let literals = restriction.enumeration.iter().chain(
                    [
                        &restriction.min_inclusive,
                        &restriction.max_inclusive,
                        &restriction.min_exclusive,
                        &restriction.max_exclusive,
                    ]
                    .into_iter()
                    .flatten(),
                );
                for literal in literals {
                    if !kinds.iter().any(|k| k.accepts(literal)) {
                        self.report(
                            AuditCode::ValueTypeMismatch,
                            path,
                            version,
                            format!("{literal:?} is not {}", expected()),
                        );
                    }
                }
            }
        }
    }

    fn restriction(&mut self, path: &str, restriction: &Restriction) {
        for pattern in &restriction.patterns {
            if let Err(reason) = pattern::compile(pattern) {
                self.report(
                    AuditCode::PatternUnverified,
                    path,
                    None,
                    format!("the pattern {pattern:?} was not evaluated: {reason}"),
                );
            }
        }
        let number = |v: &Option<String>| v.as_deref().and_then(|v| v.trim().parse::<f64>().ok());
        let lower = [
            (number(&restriction.min_inclusive), false),
            (number(&restriction.min_exclusive), true),
        ];
        let upper = [
            (number(&restriction.max_inclusive), false),
            (number(&restriction.max_exclusive), true),
        ];
        let mut contradiction = None;
        for (low, low_exclusive) in lower {
            for (high, high_exclusive) in upper {
                if let (Some(low), Some(high)) = (low, high) {
                    let empty = if low_exclusive || high_exclusive {
                        low >= high
                    } else {
                        low > high
                    };
                    if empty {
                        contradiction = Some(format!("no value lies between {low} and {high}"));
                    }
                }
            }
        }
        let lengths = (
            restriction.length,
            restriction.min_length,
            restriction.max_length,
        );
        match lengths {
            (_, Some(min), Some(max)) if min > max => {
                contradiction = Some(format!("minLength {min} exceeds maxLength {max}"));
            }
            (Some(length), Some(min), _) if length < min => {
                contradiction = Some(format!("length {length} is below minLength {min}"));
            }
            (Some(length), _, Some(max)) if length > max => {
                contradiction = Some(format!("length {length} exceeds maxLength {max}"));
            }
            _ => {}
        }
        if let Some(message) = contradiction {
            self.report(AuditCode::RestrictionBoundsContradict, path, None, message);
        }
    }
}

/// `A or B or C`, shortened when long.
fn alternatives(names: &[String]) -> String {
    const SHOWN: usize = 5;
    let mut text = names
        .iter()
        .take(SHOWN)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(" or ");
    if names.len() > SHOWN {
        text.push_str(&format!(" (and {} more)", names.len() - SHOWN));
    }
    text
}

/// Every facet of a specification with its path, in document order.
fn facets(spec: &Specification) -> Vec<(String, &Facet)> {
    let mut facets: Vec<(String, &Facet)> = spec
        .applicability
        .facets
        .iter()
        .enumerate()
        .map(|(i, facet)| (format!("applicability/facets[{i}]"), facet))
        .collect();
    if let Some(requirements) = &spec.requirements {
        facets.extend(
            requirements
                .facets
                .iter()
                .enumerate()
                .map(|(i, r)| (format!("requirements/facets[{i}]"), &r.facet)),
        );
    }
    facets
}

/// Every value of a facet with its path relative to the facet.
fn values(facet: &Facet) -> Vec<(&'static str, &Value)> {
    let values: Vec<(&'static str, Option<&Value>)> = match facet {
        Facet::Entity(entity) => vec![
            ("name", Some(&entity.name)),
            ("predefinedType", entity.predefined_type.as_ref()),
        ],
        Facet::PartOf(part_of) => vec![
            ("entity/name", Some(&part_of.entity.name)),
            (
                "entity/predefinedType",
                part_of.entity.predefined_type.as_ref(),
            ),
        ],
        Facet::Classification(classification) => vec![
            ("value", classification.value.as_ref()),
            ("system", Some(&classification.system)),
        ],
        Facet::Attribute(attribute) => vec![
            ("name", Some(&attribute.name)),
            ("value", attribute.value.as_ref()),
        ],
        Facet::Property(property) => vec![
            ("propertySet", Some(&property.property_set)),
            ("baseName", Some(&property.base_name)),
            ("value", property.value.as_ref()),
        ],
        Facet::Material(material) => vec![("value", material.value.as_ref())],
    };
    values
        .into_iter()
        .filter_map(|(path, value)| value.map(|value| (path, value)))
        .collect()
}

/// The IDS `dataType` a standard property template states, upper case.
#[cfg(feature = "audit")]
fn property_data_type(kind: &PropertyKind) -> Option<String> {
    let data_type = match kind {
        PropertyKind::SingleValue { data_type }
        | PropertyKind::BoundedValue { data_type }
        | PropertyKind::ListValue { data_type } => Some(data_type),
        PropertyKind::EnumeratedValue { data_type, .. } => data_type.as_ref(),
        _ => None,
    }?;
    data_type.type_name.as_ref().map(|t| t.to_ascii_uppercase())
}

/// The IDS `dataType` of a standard quantity's value.
#[cfg(feature = "audit")]
fn quantity_data_type(kind: QuantityKind) -> Option<String> {
    let name = match kind {
        QuantityKind::Length => "IFCLENGTHMEASURE",
        QuantityKind::Area => "IFCAREAMEASURE",
        QuantityKind::Volume => "IFCVOLUMEMEASURE",
        QuantityKind::Weight => "IFCMASSMEASURE",
        QuantityKind::Time => "IFCTIMEMEASURE",
        QuantityKind::Count => "IFCCOUNTMEASURE",
        QuantityKind::Number => "IFCNUMERICMEASURE",
        _ => return None,
    };
    Some(name.to_owned())
}

/// `xs:integer`'s lexical space.
fn is_xs_integer(value: &str) -> bool {
    let digits = value.strip_prefix(['+', '-']).unwrap_or(value);
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

/// `xs:double`'s lexical space, which includes `xs:decimal`'s.
fn is_xs_double(value: &str) -> bool {
    if matches!(value, "INF" | "+INF" | "-INF" | "NaN") {
        return true;
    }
    let unsigned = value.strip_prefix(['+', '-']).unwrap_or(value);
    let (mantissa, exponent) = match unsigned.find(['e', 'E']) {
        Some(at) => (&unsigned[..at], Some(&unsigned[at + 1..])),
        None => (unsigned, None),
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    let mantissa_ok =
        (!whole.is_empty() || !fraction.is_empty()) && digits(whole) && digits(fraction);
    mantissa_ok && exponent.is_none_or(is_xs_integer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_follow_the_xsd_lexical_spaces() {
        for good in ["42", "+42", "-0", "007"] {
            assert!(is_xs_integer(good), "{good}");
        }
        for bad in ["42.0", "42.", "4 2", "", "+", "1e3"] {
            assert!(!is_xs_integer(bad), "{bad}");
        }
        for good in ["42", "42.", ".5", "-1.5e-3", "1E10", "INF", "-INF", "NaN"] {
            assert!(is_xs_double(good), "{good}");
        }
        for bad in ["42,3", "123,4.5", "", ".", "1e", "e3", "inf", "1.2.3"] {
            assert!(!is_xs_double(bad), "{bad}");
        }
    }
}
