//! Writing IDS 1.0 documents.
//!
//! [`to_string`] and [`to_writer`] turn the typed [`Ids`] model into an IDS
//! 1.0 document that validates against `ids.xsd` 1.0.0, or refuse it with a
//! [`WriteError`] naming the offending part of the model. Nothing is written
//! that the schema would reject: the writer checks the whole model before a
//! byte reaches the output.
//!
//! # What the output looks like
//!
//! - The root declares the IDS namespace as default, `xs` for XML Schema and
//!   `xsi` with the 1.0 `xsi:schemaLocation`, so the reader records the
//!   revision as [`Detected::Declared`].
//! - Elements follow the order `ids.xsd` requires. Requirement facets keep
//!   the model's order, since the schema lets them interleave.
//! - A [`Value`] is written as `<simpleValue>` or `<xs:restriction>`, exactly
//!   as it holds it.
//! - A requirement facet always states its `@cardinality`; the applicability
//!   writes `minOccurs`/`maxOccurs` only where they differ from the schema
//!   default of `1`.
//! - Optional elements and attributes are omitted when `None`.
//! - Two-space indentation and `\n` line ends. Character data is never
//!   indented, so a value with surrounding whitespace reads back unchanged.
//!
//! # What is refused
//!
//! Beyond what the schema forbids, the writer refuses two schema-valid shapes
//! that express nothing: an applicability without facets, which would make
//! every object applicable by accident, and an `<xs:restriction>` without
//! facets, which accepts any value while looking like a constraint.
//!
//! Reading what this module writes gives back the model it was given; see
//! [`to_string`].

use core::fmt;

use openbim_core::Detected;

use crate::model::{
    Applicability, Entity, Facet, Info, Requirement, Requirements, Restriction, Specification,
    Value,
};
use crate::read::{is_author, is_xs_date, XSD_NAMESPACE, XSD_TYPES};
use crate::{Ids, IdsVersion, Occurrence, NAMESPACE};

const XSI_NAMESPACE: &str = "http://www.w3.org/2001/XMLSchema-instance";

/// Where the IDS 1.0 schema is published, as `xsi:schemaLocation` names it.
pub const SCHEMA_LOCATION: &str = "http://standards.buildingsmart.org/IDS/1.0/ids.xsd";

/// Why a model was refused, and where in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteError {
    kind: WriteErrorKind,
    location: String,
}

impl WriteError {
    /// What was wrong.
    #[must_use]
    pub fn kind(&self) -> &WriteErrorKind {
        &self.kind
    }

    /// The path to the offending part of the model, such as
    /// `specifications[0]/requirements/facets[2]/value`. Indices are
    /// 0-based positions in the model's vectors.
    #[must_use]
    pub fn location(&self) -> &str {
        &self.location
    }
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.location.is_empty() {
            write!(f, "{}", self.kind)
        } else {
            write!(f, "{}: {}", self.location, self.kind)
        }
    }
}

impl std::error::Error for WriteError {}

/// The kinds of [`WriteError`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum WriteErrorKind {
    /// The model is not IDS 1.0, or its version is a conflict.
    UnsupportedVersion(Detected<IdsVersion>),
    /// The schema requires at least one specification.
    NoSpecifications,
    /// A specification lists no IFC release; `@ifcVersion` needs one.
    NoIfcVersions,
    /// An applicability without facets.
    EmptyApplicability,
    /// Applicability facets out of the order the schema requires: at most one
    /// `entity`, then `partOf`, `classification`, `attribute`, `property`,
    /// `material`.
    ApplicabilityOrder {
        /// The facet that stands where it may not.
        found: &'static str,
    },
    /// A requirement attribute the schema does not allow on this facet.
    AttributeNotAllowed {
        /// The facet's element name.
        facet: &'static str,
        /// The attribute.
        attribute: &'static str,
    },
    /// An occurrence that this facet cannot state: an `entity` requirement is
    /// always required, a `partOf` requirement cannot be optional.
    OccurrenceNotAllowed {
        /// The facet's element name.
        facet: &'static str,
        /// The refused occurrence.
        occurrence: Occurrence,
    },
    /// An `<xs:restriction>` without facets.
    EmptyRestriction,
    /// A value outside the lexical space the schema allows.
    InvalidValue {
        /// The value.
        found: String,
        /// What the schema allows.
        expected: &'static str,
    },
    /// A character that XML 1.0 cannot represent, even escaped.
    InvalidCharacter(char),
    /// Writing to the output failed.
    Io {
        /// The I/O error kind.
        kind: std::io::ErrorKind,
        /// The I/O error message.
        message: String,
    },
}

impl fmt::Display for WriteErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WriteErrorKind::UnsupportedVersion(detected) => match detected {
                Detected::Conflict { declared, observed } => write!(
                    f,
                    "the version is a conflict (declared {declared}, observed {observed}); only 1.0 is written"
                ),
                Detected::Declared(version) | Detected::Inferred(version) => {
                    write!(f, "the model is IDS {version}; only 1.0 is written")
                }
            },
            WriteErrorKind::NoSpecifications => {
                f.write_str("IDS 1.0 requires at least one specification")
            }
            WriteErrorKind::NoIfcVersions => {
                f.write_str("a specification must list at least one IFC release")
            }
            WriteErrorKind::EmptyApplicability => {
                f.write_str("an applicability without facets matches every object")
            }
            WriteErrorKind::ApplicabilityOrder { found } => write!(
                f,
                "<{found}> is out of order; applicability takes at most one entity, then partOf, \
                 classification, attribute, property and material facets"
            ),
            WriteErrorKind::AttributeNotAllowed { facet, attribute } => {
                write!(f, "a <{facet}> requirement cannot carry @{attribute}")
            }
            WriteErrorKind::OccurrenceNotAllowed { facet, occurrence } => {
                write!(f, "a <{facet}> requirement cannot be {occurrence}")
            }
            WriteErrorKind::EmptyRestriction => {
                f.write_str("an xs:restriction without facets constrains nothing")
            }
            WriteErrorKind::InvalidValue { found, expected } => {
                write!(f, "{found:?} is not {expected}")
            }
            WriteErrorKind::InvalidCharacter(c) => {
                write!(f, "U+{:04X} cannot be represented in XML 1.0", u32::from(*c))
            }
            WriteErrorKind::Io { message, .. } => write!(f, "writing failed: {message}"),
        }
    }
}

/// Writes an IDS 1.0 document.
///
/// For every model `x` that [`read`](crate::read) returns,
/// `from_str(&to_string(&x)?)` is `x` again, and the text validates against
/// `ids.xsd` 1.0.0.
///
/// ```
/// use openbim_ids::{Entity, Ids, IfcVersion, Info, Specification};
///
/// let mut spec = Specification::new("Walls", [IfcVersion::Ifc4]);
/// spec.applicability.facets.push(Entity::new("IFCWALL").into());
/// let mut ids = Ids::new(Info::new("Example"));
/// ids.specifications.push(spec);
///
/// let xml = openbim_ids::to_string(&ids).unwrap();
/// assert_eq!(openbim_ids::from_str(&xml).unwrap(), ids);
/// ```
///
/// # Errors
///
/// Returns a [`WriteError`] when the model is not IDS 1.0 or holds something
/// IDS 1.0 cannot express; see the [module documentation](self).
pub fn to_string(ids: &Ids) -> Result<String, WriteError> {
    let mut writer = Writer::default();
    writer.ids(ids)?;
    Ok(writer.out)
}

/// Writes an IDS 1.0 document to `output` as UTF-8.
///
/// The whole document is checked before anything is written, so a refused
/// model leaves `output` untouched.
///
/// # Errors
///
/// The errors of [`to_string`], and [`WriteErrorKind::Io`] when `output`
/// fails.
pub fn to_writer(ids: &Ids, mut output: impl std::io::Write) -> Result<(), WriteError> {
    let text = to_string(ids)?;
    output
        .write_all(text.as_bytes())
        .and_then(|()| output.flush())
        .map_err(|error| WriteError {
            kind: WriteErrorKind::Io {
                kind: error.kind(),
                message: error.to_string(),
            },
            location: String::new(),
        })
}

type Result<T, E = WriteError> = core::result::Result<T, E>;

#[derive(Default)]
struct Writer {
    out: String,
    depth: usize,
    path: Vec<String>,
}

/// An attribute to write, `None` meaning absent.
type Attr<'a> = (&'static str, Option<&'a str>);

impl Writer {
    fn error(&self, kind: WriteErrorKind) -> WriteError {
        WriteError {
            kind,
            location: self.path.join("/"),
        }
    }

    fn error_at(&self, step: &str, kind: WriteErrorKind) -> WriteError {
        let mut error = self.error(kind);
        if !error.location.is_empty() {
            error.location.push('/');
        }
        error.location.push_str(step);
        error
    }

    fn at<T>(&mut self, step: String, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        self.path.push(step);
        let result = f(self)?;
        self.path.pop();
        Ok(result)
    }

    fn indent(&mut self) {
        for _ in 0..self.depth {
            self.out.push_str("  ");
        }
    }

    fn open_tag(&mut self, name: &str, attributes: &[Attr<'_>]) -> Result<()> {
        self.indent();
        self.out.push('<');
        self.out.push_str(name);
        for (attribute, value) in attributes {
            if let Some(value) = value {
                self.path.push(format!("@{attribute}"));
                check_chars(value).map_err(|c| self.error(WriteErrorKind::InvalidCharacter(c)))?;
                self.path.pop();
                self.out.push(' ');
                self.out.push_str(attribute);
                self.out.push_str("=\"");
                escape(&mut self.out, value, true);
                self.out.push('"');
            }
        }
        Ok(())
    }

    /// `<name attrs>` followed by children written by `f`, then `</name>`.
    fn element(
        &mut self,
        name: &str,
        attributes: &[Attr<'_>],
        f: impl FnOnce(&mut Self) -> Result<()>,
    ) -> Result<()> {
        self.open_tag(name, attributes)?;
        self.out.push_str(">\n");
        self.depth += 1;
        f(self)?;
        self.depth -= 1;
        self.indent();
        self.out.push_str("</");
        self.out.push_str(name);
        self.out.push_str(">\n");
        Ok(())
    }

    fn empty(&mut self, name: &str, attributes: &[Attr<'_>]) -> Result<()> {
        self.open_tag(name, attributes)?;
        self.out.push_str("/>\n");
        Ok(())
    }

    fn text(&mut self, name: &'static str, text: &str) -> Result<()> {
        self.at(name.to_owned(), |w| {
            check_chars(text).map_err(|c| w.error(WriteErrorKind::InvalidCharacter(c)))?;
            w.open_tag(name, &[])?;
            w.out.push('>');
            escape(&mut w.out, text, false);
            w.out.push_str("</");
            w.out.push_str(name);
            w.out.push_str(">\n");
            Ok(())
        })
    }

    fn optional_text(&mut self, name: &'static str, text: Option<&str>) -> Result<()> {
        text.map_or(Ok(()), |text| self.text(name, text))
    }

    fn ids(&mut self, ids: &Ids) -> Result<()> {
        if ids.version.resolved() != Some(IdsVersion::Ids1_0) {
            return Err(self.error(WriteErrorKind::UnsupportedVersion(ids.version.clone())));
        }
        if ids.specifications.is_empty() {
            return Err(self.error(WriteErrorKind::NoSpecifications));
        }
        self.out
            .push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        let location = format!("{NAMESPACE} {SCHEMA_LOCATION}");
        self.element(
            "ids",
            &[
                ("xmlns", Some(NAMESPACE)),
                ("xmlns:xs", Some(XSD_NAMESPACE)),
                ("xmlns:xsi", Some(XSI_NAMESPACE)),
                ("xsi:schemaLocation", Some(&location)),
            ],
            |w| {
                w.at("info".into(), |w| w.info(&ids.info))?;
                w.element("specifications", &[], |w| {
                    for (index, specification) in ids.specifications.iter().enumerate() {
                        w.at(format!("specifications[{index}]"), |w| {
                            w.specification(specification)
                        })?;
                    }
                    Ok(())
                })
            },
        )
    }

    fn info(&mut self, info: &Info) -> Result<()> {
        if let Some(author) = &info.author {
            if !is_author(author) {
                return Err(self.error_at(
                    "author",
                    WriteErrorKind::InvalidValue {
                        found: author.clone(),
                        expected: "an e-mail address (pattern [^@]+@[^\\.]+\\..+)",
                    },
                ));
            }
        }
        if let Some(date) = &info.date {
            if !is_xs_date(date) {
                return Err(self.error_at(
                    "date",
                    WriteErrorKind::InvalidValue {
                        found: date.clone(),
                        expected: "an xs:date such as 2024-06-01",
                    },
                ));
            }
        }
        self.element("info", &[], |w| {
            w.text("title", &info.title)?;
            w.optional_text("copyright", info.copyright.as_deref())?;
            w.optional_text("version", info.version.as_deref())?;
            w.optional_text("description", info.description.as_deref())?;
            w.optional_text("author", info.author.as_deref())?;
            w.optional_text("date", info.date.as_deref())?;
            w.optional_text("purpose", info.purpose.as_deref())?;
            w.optional_text("milestone", info.milestone.as_deref())
        })
    }

    fn specification(&mut self, specification: &Specification) -> Result<()> {
        if specification.ifc_versions.is_empty() {
            return Err(self.error(WriteErrorKind::NoIfcVersions));
        }
        let versions = specification
            .ifc_versions
            .iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        self.element(
            "specification",
            &[
                ("name", Some(&specification.name)),
                ("ifcVersion", Some(&versions)),
                ("identifier", specification.identifier.as_deref()),
                ("description", specification.description.as_deref()),
                ("instructions", specification.instructions.as_deref()),
            ],
            |w| {
                w.at("applicability".into(), |w| {
                    w.applicability(&specification.applicability)
                })?;
                if let Some(requirements) = &specification.requirements {
                    w.at("requirements".into(), |w| w.requirements(requirements))?;
                }
                Ok(())
            },
        )
    }

    fn applicability(&mut self, applicability: &Applicability) -> Result<()> {
        if applicability.facets.is_empty() {
            return Err(self.error(WriteErrorKind::EmptyApplicability));
        }
        let mut rank = 0;
        for (index, facet) in applicability.facets.iter().enumerate() {
            let this = applicability_rank(facet);
            // Only the entity may not repeat; every other kind is unbounded.
            if this < rank || (index > 0 && this == 0) {
                return Err(self.error_at(
                    &format!("facets[{index}]"),
                    WriteErrorKind::ApplicabilityOrder {
                        found: facet.kind(),
                    },
                ));
            }
            rank = this;
        }
        let min = (applicability.min_occurs != 1).then(|| applicability.min_occurs.to_string());
        let max = match applicability.max_occurs {
            Some(1) => None,
            Some(max) => Some(max.to_string()),
            None => Some("unbounded".to_owned()),
        };
        self.element(
            "applicability",
            &[("minOccurs", min.as_deref()), ("maxOccurs", max.as_deref())],
            |w| {
                for (index, facet) in applicability.facets.iter().enumerate() {
                    w.at(format!("facets[{index}]"), |w| w.facet(facet, None))?;
                }
                Ok(())
            },
        )
    }

    fn requirements(&mut self, requirements: &Requirements) -> Result<()> {
        let attributes = [("description", requirements.description.as_deref())];
        if requirements.facets.is_empty() {
            return self.empty("requirements", &attributes);
        }
        self.element("requirements", &attributes, |w| {
            for (index, requirement) in requirements.facets.iter().enumerate() {
                w.at(format!("facets[{index}]"), |w| {
                    w.facet(&requirement.facet, Some(requirement))
                })?;
            }
            Ok(())
        })
    }

    /// Writes a facet; `requirement` carries the requirement-only attributes.
    fn facet(&mut self, facet: &Facet, requirement: Option<&Requirement>) -> Result<()> {
        let kind = facet.kind();
        let mut cardinality = None;
        let mut uri = None;
        let mut instructions = None;
        if let Some(requirement) = requirement {
            let occurrence = requirement.occurrence;
            let refused = match facet {
                Facet::Entity(_) => occurrence != Occurrence::Required,
                Facet::PartOf(_) => occurrence == Occurrence::Optional,
                _ => false,
            };
            if refused {
                return Err(self.error(WriteErrorKind::OccurrenceNotAllowed {
                    facet: kind,
                    occurrence,
                }));
            }
            if !matches!(facet, Facet::Entity(_)) {
                cardinality = Some(occurrence.as_cardinality());
            }
            if requirement.uri.is_some() {
                if !matches!(
                    facet,
                    Facet::Classification(_) | Facet::Property(_) | Facet::Material(_)
                ) {
                    return Err(self.error(WriteErrorKind::AttributeNotAllowed {
                        facet: kind,
                        attribute: "uri",
                    }));
                }
                uri = requirement.uri.as_deref();
            }
            instructions = requirement.instructions.as_deref();
        }
        match facet {
            Facet::Entity(entity) => {
                self.element("entity", &[("instructions", instructions)], |w| {
                    w.entity_content(entity)
                })
            }
            Facet::PartOf(part_of) => self.element(
                "partOf",
                &[
                    ("relation", part_of.relation.map(|r| r.as_str())),
                    ("cardinality", cardinality),
                    ("instructions", instructions),
                ],
                |w| {
                    w.at("entity".into(), |w| {
                        w.element("entity", &[], |w| w.entity_content(&part_of.entity))
                    })
                },
            ),
            Facet::Classification(classification) => self.element(
                "classification",
                &[
                    ("uri", uri),
                    ("cardinality", cardinality),
                    ("instructions", instructions),
                ],
                |w| {
                    w.optional_value("value", classification.value.as_ref())?;
                    w.value("system", &classification.system)
                },
            ),
            Facet::Attribute(attribute) => self.element(
                "attribute",
                &[("cardinality", cardinality), ("instructions", instructions)],
                |w| {
                    w.value("name", &attribute.name)?;
                    w.optional_value("value", attribute.value.as_ref())
                },
            ),
            Facet::Property(property) => {
                if let Some(data_type) = &property.data_type {
                    if data_type.is_empty() || !data_type.bytes().all(|b| b.is_ascii_uppercase()) {
                        return Err(self.error_at(
                            "@dataType",
                            WriteErrorKind::InvalidValue {
                                found: data_type.clone(),
                                expected: "an upper-case name matching [A-Z]+",
                            },
                        ));
                    }
                }
                self.element(
                    "property",
                    &[
                        ("dataType", property.data_type.as_deref()),
                        ("uri", uri),
                        ("cardinality", cardinality),
                        ("instructions", instructions),
                    ],
                    |w| {
                        w.value("propertySet", &property.property_set)?;
                        w.value("baseName", &property.base_name)?;
                        w.optional_value("value", property.value.as_ref())
                    },
                )
            }
            Facet::Material(material) => {
                let attributes = [
                    ("uri", uri),
                    ("cardinality", cardinality),
                    ("instructions", instructions),
                ];
                match &material.value {
                    Some(value) => {
                        self.element("material", &attributes, |w| w.value("value", value))
                    }
                    None => self.empty("material", &attributes),
                }
            }
        }
    }

    fn entity_content(&mut self, entity: &Entity) -> Result<()> {
        self.value("name", &entity.name)?;
        self.optional_value("predefinedType", entity.predefined_type.as_ref())
    }

    fn optional_value(&mut self, name: &'static str, value: Option<&Value>) -> Result<()> {
        value.map_or(Ok(()), |value| self.value(name, value))
    }

    fn value(&mut self, name: &'static str, value: &Value) -> Result<()> {
        self.at(name.to_owned(), |w| {
            w.element(name, &[], |w| match value {
                Value::Simple(text) => w.text("simpleValue", text),
                Value::Restriction(restriction) => {
                    w.at("xs:restriction".into(), |w| w.restriction(restriction))
                }
            })
        })
    }

    fn restriction(&mut self, restriction: &Restriction) -> Result<()> {
        if !XSD_TYPES.contains(&restriction.base.as_str()) {
            return Err(self.error_at(
                "@base",
                WriteErrorKind::InvalidValue {
                    found: restriction.base.clone(),
                    expected:
                        "the local name of an XML Schema built-in simple type, such as string",
                },
            ));
        }
        if restriction.total_digits == Some(0) {
            return Err(self.error_at(
                "xs:totalDigits",
                WriteErrorKind::InvalidValue {
                    found: "0".into(),
                    expected: "a positive integer",
                },
            ));
        }
        let counts = [
            ("xs:length", restriction.length),
            ("xs:minLength", restriction.min_length),
            ("xs:maxLength", restriction.max_length),
            ("xs:totalDigits", restriction.total_digits),
            ("xs:fractionDigits", restriction.fraction_digits),
        ];
        let bounds = [
            ("xs:minInclusive", restriction.min_inclusive.as_deref()),
            ("xs:maxInclusive", restriction.max_inclusive.as_deref()),
            ("xs:minExclusive", restriction.min_exclusive.as_deref()),
            ("xs:maxExclusive", restriction.max_exclusive.as_deref()),
        ];
        let has_facets = !restriction.enumeration.is_empty()
            || !restriction.patterns.is_empty()
            || counts.iter().any(|(_, v)| v.is_some())
            || bounds.iter().any(|(_, v)| v.is_some());
        if !has_facets {
            return Err(self.error(WriteErrorKind::EmptyRestriction));
        }
        let base = format!("xs:{}", restriction.base);
        self.element("xs:restriction", &[("base", Some(&base))], |w| {
            for value in &restriction.enumeration {
                w.at("xs:enumeration".into(), |w| {
                    w.empty("xs:enumeration", &[("value", Some(value))])
                })?;
            }
            for value in &restriction.patterns {
                w.at("xs:pattern".into(), |w| {
                    w.empty("xs:pattern", &[("value", Some(value))])
                })?;
            }
            for (name, value) in bounds {
                if let Some(value) = value {
                    w.at(name.into(), |w| w.empty(name, &[("value", Some(value))]))?;
                }
            }
            for (name, value) in counts {
                if let Some(value) = value {
                    w.empty(name, &[("value", Some(&value.to_string()))])?;
                }
            }
            Ok(())
        })
    }
}

/// The position of a facet kind in the applicability sequence.
fn applicability_rank(facet: &Facet) -> u8 {
    match facet {
        Facet::Entity(_) => 0,
        Facet::PartOf(_) => 1,
        Facet::Classification(_) => 2,
        Facet::Attribute(_) => 3,
        Facet::Property(_) => 4,
        Facet::Material(_) => 5,
    }
}

/// The first character XML 1.0 cannot carry, if any.
fn check_chars(text: &str) -> Result<(), char> {
    match text.chars().find(|&c| {
        !matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..)
    }) {
        Some(c) => Err(c),
        None => Ok(()),
    }
}

/// Escapes `text` so that a reader returns it unchanged.
///
/// Attribute values additionally escape the quote and the whitespace that
/// attribute-value normalization would turn into spaces; character data
/// escapes `\r`, which end-of-line handling would drop.
fn escape(out: &mut String, text: &str, attribute: bool) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\r' => out.push_str("&#13;"),
            '"' if attribute => out.push_str("&quot;"),
            '\t' if attribute => out.push_str("&#9;"),
            '\n' if attribute => out.push_str("&#10;"),
            c => out.push(c),
        }
    }
}
