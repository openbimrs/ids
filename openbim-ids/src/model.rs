//! The typed content of an IDS 1.0 document.
//!
//! These types mirror `ids.xsd` 1.0.0 closely enough that every schema-valid
//! document has exactly one representation, and nothing the schema leaves to
//! the author is decided here: a value that the schema types as `xs:string`
//! stays the string the document wrote, and a restriction keeps its facets
//! rather than being compiled into a matcher. Matching values against a model
//! is auditing, which belongs to a later layer.
//!
//! What the reader *does* settle is everything the schema itself settles:
//! attribute defaults are applied ([`Occurrence::DEFAULT`] for a requirement
//! facet without `@cardinality`, `1` for an absent `minOccurs`/`maxOccurs`),
//! enumerated tokens become enums, and list-typed attributes are split.

use core::fmt;

use openbim_core::Detected;

use crate::{IdsVersion, Occurrence};

/// A whole IDS document.
#[derive(Debug, Clone, PartialEq)]
pub struct Ids {
    /// Which IDS revision the document is, and how that was established.
    ///
    /// Always resolves to [`IdsVersion::Ids1_0`] for a document the reader
    /// accepted; [`Detected::Inferred`] means the document did not say so and
    /// was read as 1.0 because its shape is 1.0 (see
    /// [`read`](crate::read)).
    pub version: Detected<IdsVersion>,
    /// Document metadata.
    pub info: Info,
    /// The specifications, in document order. The schema requires at least one.
    pub specifications: Vec<Specification>,
}

/// The `<info>` block. Only `title` is required by the schema.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Info {
    /// `<title>`.
    pub title: String,
    /// `<copyright>`.
    pub copyright: Option<String>,
    /// `<version>`, the document's own version, not the IDS revision.
    pub version: Option<String>,
    /// `<description>`.
    pub description: Option<String>,
    /// `<author>`, which the schema constrains to look like an e-mail address.
    pub author: Option<String>,
    /// `<date>`, an `xs:date` in its lexical form, e.g. `2024-06-01`.
    pub date: Option<String>,
    /// `<purpose>`.
    pub purpose: Option<String>,
    /// `<milestone>`.
    pub milestone: Option<String>,
}

/// One `<specification>`.
#[derive(Debug, Clone, PartialEq)]
pub struct Specification {
    /// `@name`, required.
    pub name: String,
    /// `@ifcVersion`, an `xs:list`: `"IFC2X3 IFC4"` is two releases.
    pub ifc_versions: Vec<IfcVersion>,
    /// `@identifier`. Not unique across documents, by the schema's own note.
    pub identifier: Option<String>,
    /// `@description`.
    pub description: Option<String>,
    /// `@instructions`, meant for the model author.
    pub instructions: Option<String>,
    /// Which objects the specification is about.
    pub applicability: Applicability,
    /// What those objects must satisfy. `None` when `<requirements>` is
    /// absent, which is distinct from an empty `<requirements/>` only in the
    /// description it can carry.
    pub requirements: Option<Requirements>,
}

impl Specification {
    /// How many applicable objects the specification demands.
    ///
    /// Lowered from the applicability's `minOccurs`/`maxOccurs`: `0..0` is
    /// prohibited, `0..n` optional, anything with a non-zero minimum required.
    #[must_use]
    pub fn occurrence(&self) -> Occurrence {
        self.applicability.occurrence()
    }
}

/// A release named in `@ifcVersion`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IfcVersion {
    /// `IFC2X3`.
    Ifc2x3,
    /// `IFC4`.
    Ifc4,
    /// `IFC4X3_ADD2`.
    Ifc4x3Add2,
}

impl IfcVersion {
    /// Every release the schema enumerates.
    pub const ALL: [IfcVersion; 3] = [IfcVersion::Ifc2x3, IfcVersion::Ifc4, IfcVersion::Ifc4x3Add2];

    /// The token as the schema spells it.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            IfcVersion::Ifc2x3 => "IFC2X3",
            IfcVersion::Ifc4 => "IFC4",
            IfcVersion::Ifc4x3Add2 => "IFC4X3_ADD2",
        }
    }

    /// Parses one schema token. Case-sensitive, as the enumeration is.
    #[must_use]
    pub fn from_token(token: &str) -> Option<IfcVersion> {
        IfcVersion::ALL.into_iter().find(|v| v.as_str() == token)
    }
}

impl fmt::Display for IfcVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// `<applicability>`: the facets an object must match to be checked.
#[derive(Debug, Clone, PartialEq)]
pub struct Applicability {
    /// `@minOccurs`, default `1`.
    pub min_occurs: u32,
    /// `@maxOccurs`, default `1`; `None` is `unbounded`.
    pub max_occurs: Option<u32>,
    /// The facets, in document order. An object is applicable when it
    /// matches all of them.
    pub facets: Vec<Facet>,
}

impl Applicability {
    /// The occurrence the bounds spell, see [`Specification::occurrence`].
    #[must_use]
    pub fn occurrence(&self) -> Occurrence {
        Occurrence::from_xs_occurs(self.min_occurs, self.max_occurs)
    }
}

/// `<requirements>`.
#[derive(Debug, Clone, PartialEq)]
pub struct Requirements {
    /// `@description`.
    pub description: Option<String>,
    /// The requirement facets in document order. The schema lets facet kinds
    /// interleave, so the order is the document's, not grouped by kind.
    pub facets: Vec<Requirement>,
}

/// One requirement facet with the attributes only requirements carry.
#[derive(Debug, Clone, PartialEq)]
pub struct Requirement {
    /// What is required.
    pub facet: Facet,
    /// `@cardinality`, [`Occurrence::DEFAULT`] when absent. An entity facet
    /// has no cardinality and is always required; a `partOf` facet cannot be
    /// optional.
    pub occurrence: Occurrence,
    /// `@uri`, allowed on classification, property and material facets.
    pub uri: Option<String>,
    /// `@instructions`, meant for the model author.
    pub instructions: Option<String>,
}

/// A facet: one condition on an object.
#[derive(Debug, Clone, PartialEq)]
pub enum Facet {
    /// `<entity>`: the object's IFC class and predefined type.
    Entity(Entity),
    /// `<partOf>`: a relationship to another object.
    PartOf(PartOf),
    /// `<classification>`: a classification reference.
    Classification(Classification),
    /// `<attribute>`: a direct IFC attribute.
    Attribute(Attribute),
    /// `<property>`: a property in a property or quantity set.
    Property(Property),
    /// `<material>`: an assigned material.
    Material(Material),
}

impl Facet {
    /// The element name of the facet, e.g. `"partOf"`.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Facet::Entity(_) => "entity",
            Facet::PartOf(_) => "partOf",
            Facet::Classification(_) => "classification",
            Facet::Attribute(_) => "attribute",
            Facet::Property(_) => "property",
            Facet::Material(_) => "material",
        }
    }
}

/// `<entity>`.
#[derive(Debug, Clone, PartialEq)]
pub struct Entity {
    /// `<name>`, an upper-case IFC class name such as `IFCWALL`.
    pub name: Value,
    /// `<predefinedType>`.
    pub predefined_type: Option<Value>,
}

/// `<partOf>`.
#[derive(Debug, Clone, PartialEq)]
pub struct PartOf {
    /// The entity at the other end of the relationship.
    pub entity: Entity,
    /// `@relation`; `None` means any of them.
    pub relation: Option<Relation>,
}

/// A relationship a `partOf` facet can follow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Relation {
    /// `IFCRELAGGREGATES`.
    Aggregates,
    /// `IFCRELASSIGNSTOGROUP`.
    AssignsToGroup,
    /// `IFCRELCONTAINEDINSPATIALSTRUCTURE`.
    ContainedInSpatialStructure,
    /// `IFCRELNESTS`.
    Nests,
    /// `IFCRELVOIDSELEMENT IFCRELFILLSELEMENT`: an element filling an opening
    /// that voids another element. One token that contains a space.
    VoidsElementFillsElement,
}

impl Relation {
    /// Every relation the schema enumerates.
    pub const ALL: [Relation; 5] = [
        Relation::Aggregates,
        Relation::AssignsToGroup,
        Relation::ContainedInSpatialStructure,
        Relation::Nests,
        Relation::VoidsElementFillsElement,
    ];

    /// The token as the schema spells it.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Relation::Aggregates => "IFCRELAGGREGATES",
            Relation::AssignsToGroup => "IFCRELASSIGNSTOGROUP",
            Relation::ContainedInSpatialStructure => "IFCRELCONTAINEDINSPATIALSTRUCTURE",
            Relation::Nests => "IFCRELNESTS",
            Relation::VoidsElementFillsElement => "IFCRELVOIDSELEMENT IFCRELFILLSELEMENT",
        }
    }

    /// Parses one schema token, exactly as the enumeration spells it.
    #[must_use]
    pub fn from_token(token: &str) -> Option<Relation> {
        Relation::ALL.into_iter().find(|r| r.as_str() == token)
    }
}

impl fmt::Display for Relation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// `<classification>`.
#[derive(Debug, Clone, PartialEq)]
pub struct Classification {
    /// `<value>`, the reference's identification. `None` means any.
    pub value: Option<Value>,
    /// `<system>`, the classification system's name. Required in 1.0.
    pub system: Value,
}

/// `<attribute>`.
#[derive(Debug, Clone, PartialEq)]
pub struct Attribute {
    /// `<name>`, the attribute name as the IFC schema spells it, e.g. `Name`.
    pub name: Value,
    /// `<value>`. `None` means any non-empty value.
    pub value: Option<Value>,
}

/// `<property>`.
#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    /// `<propertySet>`, e.g. `Pset_WallCommon`.
    pub property_set: Value,
    /// `<baseName>`, the property name as stored in the file.
    pub base_name: Value,
    /// `<value>`. `None` means any non-empty value.
    pub value: Option<Value>,
    /// `@dataType`, an upper-case IFC defined type such as `IFCLABEL`.
    pub data_type: Option<String>,
}

/// `<material>`.
#[derive(Debug, Clone, PartialEq)]
pub struct Material {
    /// `<value>`, a material or material category name. `None` means any.
    pub value: Option<Value>,
}

/// An `ids:idsValue`: a literal or an XML Schema restriction.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// `<simpleValue>`, verbatim. An `xs:string`, so surrounding whitespace
    /// is part of the value.
    Simple(String),
    /// `<xs:restriction>`.
    Restriction(Box<Restriction>),
}

impl Value {
    /// The literal, when this is a `<simpleValue>`.
    #[must_use]
    pub fn as_simple(&self) -> Option<&str> {
        match self {
            Value::Simple(value) => Some(value),
            Value::Restriction(_) => None,
        }
    }
}

/// An `<xs:restriction>` and its constraining facets.
///
/// Facet values stay in the lexical form the document wrote: whether
/// `"2.50"` is a number depends on `base` and on what it is compared with,
/// which is an audit decision.
///
/// Within one restriction, several `enumeration`s are alternatives and so
/// are several `pattern`s, as in XML Schema.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Restriction {
    /// The local name of the XML Schema built-in type in `@base`, e.g.
    /// `string` for `xs:string`. The prefix is resolved, so `xsd:string` is
    /// `string` too.
    pub base: String,
    /// `<xs:enumeration>` values.
    pub enumeration: Vec<String>,
    /// `<xs:pattern>` values: XML Schema regular expressions, which match the
    /// whole value (they are implicitly anchored).
    pub patterns: Vec<String>,
    /// `<xs:minInclusive>`.
    pub min_inclusive: Option<String>,
    /// `<xs:maxInclusive>`.
    pub max_inclusive: Option<String>,
    /// `<xs:minExclusive>`.
    pub min_exclusive: Option<String>,
    /// `<xs:maxExclusive>`.
    pub max_exclusive: Option<String>,
    /// `<xs:length>`.
    pub length: Option<u64>,
    /// `<xs:minLength>`.
    pub min_length: Option<u64>,
    /// `<xs:maxLength>`.
    pub max_length: Option<u64>,
    /// `<xs:totalDigits>`.
    pub total_digits: Option<u64>,
    /// `<xs:fractionDigits>`.
    pub fraction_digits: Option<u64>,
}
