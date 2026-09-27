//! Reading IDS 1.0 documents.
//!
//! [`from_str`] and [`from_slice`] turn a document into the typed [`Ids`]
//! model or refuse it with a [`ReadError`] that names the line and column.
//!
//! # Two passes
//!
//! The first pass is lenient. It walks every element and records the
//! [`Signal`]s that tell IDS revisions apart, reads the revision the document
//! declares in `xsi:schemaLocation`, and asks [`detect_version`] for a
//! verdict. A pre-1.0 draft, or a document whose declaration contradicts its
//! shape, is refused here with [`ErrorKind::UnsupportedVersion`] carrying the
//! evidence, instead of failing later on the first draft-only attribute with
//! an error that hides the real cause.
//!
//! The second pass is strict and follows `ids.xsd` 1.0.0: element order,
//! required elements and attributes, enumerated tokens, and the lexical forms
//! of numbers and dates. Anything the schema would reject is refused.
//!
//! # A document that says nothing about its revision
//!
//! A file with no `xsi:schemaLocation` naming a revision, and none of the
//! shapes that distinguish revisions, is read as 1.0 only when the strict
//! pass accepts the whole document, and is reported as
//! [`Detected::Inferred`] so a caller that insists on a declaration can
//! refuse it.
//!
//! # What is not checked
//!
//! The reader checks the document against the IDS schema, not against IFC.
//! An entity name that no IFC release defines, a `dataType` that is not an
//! IFC defined type, or a value that cannot be cast to one is a well-formed
//! IDS document and is returned as such; judging it needs the IFC schema,
//! which is the auditor's job. Likewise the facet patterns of a restriction
//! are returned as written, not compiled.

use core::fmt;

use openbim_core::Detected;
use roxmltree::{Document, Node, ParsingOptions};

use crate::model::{
    Applicability, Attribute, Classification, Entity, Facet, IfcVersion, Info, Material, PartOf,
    Property, Relation, Requirement, Requirements, Restriction, Specification, Value,
};
use crate::{detect_version, Ids, IdsVersion, Occurrence, Signal, VersionSignals, NAMESPACE};

/// The XML Schema namespace, in which `<xs:restriction>` and its facets live.
pub const XSD_NAMESPACE: &str = "http://www.w3.org/2001/XMLSchema";
const XSI_NAMESPACE: &str = "http://www.w3.org/2001/XMLSchema-instance";
const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";

/// Why a document was refused, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadError {
    kind: ErrorKind,
    line: u32,
    column: u32,
}

impl ReadError {
    /// What was wrong.
    #[must_use]
    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }

    /// The 1-based line of the offending node.
    #[must_use]
    pub fn line(&self) -> u32 {
        self.line
    }

    /// The 1-based column of the offending node.
    #[must_use]
    pub fn column(&self) -> u32 {
        self.column
    }
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.column, self.kind)
    }
}

impl std::error::Error for ReadError {}

/// The kinds of [`ReadError`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorKind {
    /// The bytes are not UTF-8.
    NotUtf8,
    /// The text is not well-formed XML, or uses a DTD.
    Xml(String),
    /// The root element is not `<ids>` in the IDS namespace.
    NotIds {
        /// The root element as `{namespace}name`.
        found: String,
    },
    /// The document is not IDS 1.0, or its declaration contradicts its shape.
    UnsupportedVersion(Detected<IdsVersion>),
    /// An element that the schema does not allow here, or not in this order.
    UnexpectedElement {
        /// The parent element.
        parent: String,
        /// The unexpected child as `{namespace}name`.
        found: String,
    },
    /// A required child element is missing.
    MissingElement {
        /// The parent element.
        parent: String,
        /// The missing child.
        expected: &'static str,
    },
    /// An attribute that the schema does not allow on this element.
    UnexpectedAttribute {
        /// The element.
        element: String,
        /// The attribute as `{namespace}name`, or bare when unqualified.
        attribute: String,
    },
    /// A required attribute is missing.
    MissingAttribute {
        /// The element.
        element: String,
        /// The missing attribute.
        attribute: &'static str,
    },
    /// An element or attribute value outside its lexical space.
    InvalidValue {
        /// The element or `element/@attribute` holding the value.
        location: String,
        /// The value as written.
        found: String,
        /// What the schema allows.
        expected: &'static str,
    },
    /// Character data inside an element that may only contain elements.
    UnexpectedText {
        /// The element.
        element: String,
    },
    /// A valid XML Schema construct this reader does not represent.
    Unsupported {
        /// The element.
        element: String,
        /// What is not supported.
        construct: String,
    },
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorKind::NotUtf8 => f.write_str("the document is not UTF-8"),
            ErrorKind::Xml(message) => write!(f, "not well-formed XML: {message}"),
            ErrorKind::NotIds { found } => {
                write!(f, "the root element is {found}, not {{{NAMESPACE}}}ids")
            }
            ErrorKind::UnsupportedVersion(detected) => match detected {
                Detected::Conflict { declared, observed } => write!(
                    f,
                    "the document declares IDS {declared} but its shape is IDS {observed}"
                ),
                Detected::Declared(version) => {
                    write!(f, "the document declares IDS {version}; only 1.0 is read")
                }
                Detected::Inferred(version) => write!(
                    f,
                    "the document's shape is IDS {version} (pre-1.0); only 1.0 is read"
                ),
            },
            ErrorKind::UnexpectedElement { parent, found } => {
                write!(f, "<{parent}> does not allow {found} here")
            }
            ErrorKind::MissingElement { parent, expected } => {
                write!(f, "<{parent}> is missing <{expected}>")
            }
            ErrorKind::UnexpectedAttribute { element, attribute } => {
                write!(f, "<{element}> does not allow the attribute {attribute}")
            }
            ErrorKind::MissingAttribute { element, attribute } => {
                write!(f, "<{element}> is missing the attribute {attribute}")
            }
            ErrorKind::InvalidValue {
                location,
                found,
                expected,
            } => write!(f, "{location} is {found:?}; expected {expected}"),
            ErrorKind::UnexpectedText { element } => {
                write!(f, "<{element}> may only contain elements, not text")
            }
            ErrorKind::Unsupported { element, construct } => {
                write!(f, "<{element}> uses {construct}, which is not supported")
            }
        }
    }
}

/// Reads an IDS 1.0 document from bytes.
///
/// The bytes must be UTF-8; a leading byte-order mark is skipped.
///
/// # Errors
///
/// Returns [`ErrorKind::NotUtf8`] for other encodings, and otherwise the
/// errors of [`from_str`].
pub fn from_slice(bytes: &[u8]) -> Result<Ids, ReadError> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let text = core::str::from_utf8(bytes).map_err(|_| ReadError {
        kind: ErrorKind::NotUtf8,
        line: 1,
        column: 1,
    })?;
    from_str(text)
}

/// Reads an IDS 1.0 document.
///
/// ```
/// let ids = openbim_ids::read::from_str(r#"
///   <ids xmlns="http://standards.buildingsmart.org/IDS">
///     <info><title>Walls</title></info>
///     <specifications>
///       <specification name="Walls are named" ifcVersion="IFC4">
///         <applicability><entity><name><simpleValue>IFCWALL</simpleValue></name></entity></applicability>
///         <requirements><attribute><name><simpleValue>Name</simpleValue></name></attribute></requirements>
///       </specification>
///     </specifications>
///   </ids>"#).unwrap();
/// assert_eq!(ids.specifications[0].name, "Walls are named");
/// ```
///
/// # Errors
///
/// Returns a [`ReadError`] when the text is not well-formed XML, is not IDS
/// 1.0, or does not conform to the IDS 1.0 schema.
pub fn from_str(text: &str) -> Result<Ids, ReadError> {
    let options = ParsingOptions {
        allow_dtd: false,
        ..ParsingOptions::default()
    };
    let document = Document::parse_with_options(text, options).map_err(|error| {
        let position = error.pos();
        ReadError {
            kind: ErrorKind::Xml(error.to_string()),
            line: position.row,
            column: position.col,
        }
    })?;
    Reader {
        document: &document,
    }
    .ids()
}

/// Which facets and which attributes an element may carry.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Applicability,
    Requirement,
}

struct Reader<'d, 'input> {
    document: &'d Document<'input>,
}

type Result<T, E = ReadError> = core::result::Result<T, E>;

impl<'d, 'input> Reader<'d, 'input> {
    fn error(&self, node: Node<'_, '_>, kind: ErrorKind) -> ReadError {
        let position = self.document.text_pos_at(node.range().start);
        ReadError {
            kind,
            line: position.row,
            column: position.col,
        }
    }

    fn ids(&self) -> Result<Ids> {
        let root = self.document.root_element();
        if !is_ids(root, "ids") {
            return Err(self.error(
                root,
                ErrorKind::NotIds {
                    found: expanded(root),
                },
            ));
        }
        let version = match detect_version(declared_version(root), &signals(root)) {
            Some(
                detected @ (Detected::Declared(IdsVersion::Ids1_0)
                | Detected::Inferred(IdsVersion::Ids1_0)),
            ) => detected,
            None => Detected::Inferred(IdsVersion::Ids1_0),
            Some(other) => return Err(self.error(root, ErrorKind::UnsupportedVersion(other))),
        };
        self.attributes(root, &[])?;
        let mut children = self.sequence(root)?;
        let info = self.info(children.require("info")?)?;
        let specifications = self.specifications(children.require("specifications")?)?;
        children.finish()?;
        Ok(Ids {
            version,
            info,
            specifications,
        })
    }

    fn info(&self, node: Node<'d, 'input>) -> Result<Info> {
        self.attributes(node, &[])?;
        let mut children = self.sequence(node)?;
        let title = self.text(children.require("title")?)?;
        let copyright = children
            .optional("copyright")
            .map(|n| self.text(n))
            .transpose()?;
        let version = children
            .optional("version")
            .map(|n| self.text(n))
            .transpose()?;
        let description = children
            .optional("description")
            .map(|n| self.text(n))
            .transpose()?;
        let author = children
            .optional("author")
            .map(|n| self.author(n))
            .transpose()?;
        let date = children
            .optional("date")
            .map(|n| self.date(n))
            .transpose()?;
        let purpose = children
            .optional("purpose")
            .map(|n| self.text(n))
            .transpose()?;
        let milestone = children
            .optional("milestone")
            .map(|n| self.text(n))
            .transpose()?;
        children.finish()?;
        Ok(Info {
            title,
            copyright,
            version,
            description,
            author,
            date,
            purpose,
            milestone,
        })
    }

    fn author(&self, node: Node<'d, 'input>) -> Result<String> {
        let author = self.text(node)?;
        if is_author(&author) {
            Ok(author)
        } else {
            Err(self.error(
                node,
                ErrorKind::InvalidValue {
                    location: "author".into(),
                    found: author,
                    expected: "an e-mail address (pattern [^@]+@[^\\.]+\\..+)",
                },
            ))
        }
    }

    fn date(&self, node: Node<'d, 'input>) -> Result<String> {
        // `xs:date` collapses whitespace before checking the lexical form.
        let date = self.text(node)?.trim().to_owned();
        if is_xs_date(&date) {
            Ok(date)
        } else {
            Err(self.error(
                node,
                ErrorKind::InvalidValue {
                    location: "date".into(),
                    found: date,
                    expected: "an xs:date such as 2024-06-01",
                },
            ))
        }
    }

    fn specifications(&self, node: Node<'d, 'input>) -> Result<Vec<Specification>> {
        self.attributes(node, &[])?;
        let mut children = self.sequence(node)?;
        let mut specifications = vec![self.specification(children.require("specification")?)?];
        while let Some(child) = children.optional("specification") {
            specifications.push(self.specification(child)?);
        }
        children.finish()?;
        Ok(specifications)
    }

    fn specification(&self, node: Node<'d, 'input>) -> Result<Specification> {
        self.attributes(
            node,
            &[
                "name",
                "ifcVersion",
                "identifier",
                "description",
                "instructions",
            ],
        )?;
        let name = self.required_attribute(node, "name")?.to_owned();
        let ifc_versions = self.ifc_versions(node)?;
        let mut children = self.sequence(node)?;
        let applicability = self.applicability(children.require("applicability")?)?;
        let requirements = children
            .optional("requirements")
            .map(|n| self.requirements(n))
            .transpose()?;
        children.finish()?;
        Ok(Specification {
            name,
            ifc_versions,
            identifier: attribute(node, "identifier"),
            description: attribute(node, "description"),
            instructions: attribute(node, "instructions"),
            applicability,
            requirements,
        })
    }

    fn ifc_versions(&self, node: Node<'d, 'input>) -> Result<Vec<IfcVersion>> {
        let list = self.required_attribute(node, "ifcVersion")?;
        list.split(is_xml_space)
            .filter(|token| !token.is_empty())
            .map(|token| {
                IfcVersion::from_token(token).ok_or_else(|| {
                    self.error(
                        node,
                        ErrorKind::InvalidValue {
                            location: "specification/@ifcVersion".into(),
                            found: token.to_owned(),
                            expected: "IFC2X3, IFC4 or IFC4X3_ADD2",
                        },
                    )
                })
            })
            .collect()
    }

    fn applicability(&self, node: Node<'d, 'input>) -> Result<Applicability> {
        self.attributes(node, &["minOccurs", "maxOccurs"])?;
        let min_occurs = match node.attribute("minOccurs") {
            Some(value) => self.non_negative(node, "applicability/@minOccurs", value)?,
            None => 1,
        };
        let max_occurs = match node.attribute("maxOccurs") {
            Some(value) if value.trim() == "unbounded" => None,
            Some(value) => Some(self.non_negative(node, "applicability/@maxOccurs", value)?),
            None => Some(1),
        };
        let mut children = self.sequence(node)?;
        let mut facets = Vec::new();
        if let Some(child) = children.optional("entity") {
            facets.push(self.facet(child, Role::Applicability)?);
        }
        for kind in [
            "partOf",
            "classification",
            "attribute",
            "property",
            "material",
        ] {
            while let Some(child) = children.optional(kind) {
                facets.push(self.facet(child, Role::Applicability)?);
            }
        }
        children.finish()?;
        Ok(Applicability {
            min_occurs,
            max_occurs,
            facets,
        })
    }

    fn requirements(&self, node: Node<'d, 'input>) -> Result<Requirements> {
        self.attributes(node, &["description"])?;
        let facets = self
            .elements(node)?
            .into_iter()
            .map(|child| self.requirement(node, child))
            .collect::<Result<_>>()?;
        Ok(Requirements {
            description: attribute(node, "description"),
            facets,
        })
    }

    fn requirement(&self, parent: Node<'d, 'input>, node: Node<'d, 'input>) -> Result<Requirement> {
        if !FACETS.iter().any(|kind| is_ids(node, kind)) {
            return Err(self.error(
                node,
                ErrorKind::UnexpectedElement {
                    parent: local(parent),
                    found: expanded(node),
                },
            ));
        }
        let facet = self.facet(node, Role::Requirement)?;
        let occurrence = match node.attribute("cardinality") {
            None => Occurrence::DEFAULT,
            Some(token) => {
                let occurrence = Occurrence::from_cardinality(token);
                // partOf takes the simple cardinality, which has no `optional`.
                let allowed = match occurrence {
                    Some(Occurrence::Optional) => !matches!(facet, Facet::PartOf(_)),
                    Some(_) => true,
                    None => false,
                };
                if !allowed {
                    return Err(self.error(
                        node,
                        ErrorKind::InvalidValue {
                            location: format!("{}/@cardinality", local(node)),
                            found: token.to_owned(),
                            expected: if matches!(facet, Facet::PartOf(_)) {
                                "required or prohibited"
                            } else {
                                "required, prohibited or optional"
                            },
                        },
                    ));
                }
                occurrence.unwrap_or(Occurrence::DEFAULT)
            }
        };
        Ok(Requirement {
            facet,
            occurrence,
            uri: attribute(node, "uri"),
            instructions: attribute(node, "instructions"),
        })
    }

    fn facet(&self, node: Node<'d, 'input>, role: Role) -> Result<Facet> {
        let kind = node.tag_name().name();
        self.attributes(node, facet_attributes(kind, role))?;
        let mut children = self.sequence(node)?;
        let facet = match kind {
            "entity" => Facet::Entity(self.entity_content(&mut children)?),
            "partOf" => {
                let entity = children.require("entity")?;
                self.attributes(entity, &[])?;
                let mut inner = self.sequence(entity)?;
                let entity = self.entity_content(&mut inner)?;
                inner.finish()?;
                Facet::PartOf(PartOf {
                    entity,
                    relation: self.relation(node)?,
                })
            }
            "classification" => Facet::Classification(Classification {
                value: children
                    .optional("value")
                    .map(|n| self.value(n))
                    .transpose()?,
                system: self.value(children.require("system")?)?,
            }),
            "attribute" => Facet::Attribute(Attribute {
                name: self.value(children.require("name")?)?,
                value: children
                    .optional("value")
                    .map(|n| self.value(n))
                    .transpose()?,
            }),
            "property" => Facet::Property(Property {
                property_set: self.value(children.require("propertySet")?)?,
                base_name: self.value(children.require("baseName")?)?,
                value: children
                    .optional("value")
                    .map(|n| self.value(n))
                    .transpose()?,
                data_type: self.data_type(node)?,
            }),
            "material" => Facet::Material(Material {
                value: children
                    .optional("value")
                    .map(|n| self.value(n))
                    .transpose()?,
            }),
            _ => unreachable!("callers pass facet elements only"),
        };
        children.finish()?;
        Ok(facet)
    }

    fn entity_content(&self, children: &mut Sequence<'_, 'd, 'input>) -> Result<Entity> {
        Ok(Entity {
            name: self.value(children.require("name")?)?,
            predefined_type: children
                .optional("predefinedType")
                .map(|n| self.value(n))
                .transpose()?,
        })
    }

    fn relation(&self, node: Node<'d, 'input>) -> Result<Option<Relation>> {
        node.attribute("relation")
            .map(|token| {
                Relation::from_token(token).ok_or_else(|| {
                    self.error(
                        node,
                        ErrorKind::InvalidValue {
                            location: "partOf/@relation".into(),
                            found: token.to_owned(),
                            expected: "an IDS relation such as IFCRELAGGREGATES",
                        },
                    )
                })
            })
            .transpose()
    }

    fn data_type(&self, node: Node<'d, 'input>) -> Result<Option<String>> {
        node.attribute("dataType")
            .map(|value| {
                if !value.is_empty() && value.bytes().all(|b| b.is_ascii_uppercase()) {
                    Ok(value.to_owned())
                } else {
                    Err(self.error(
                        node,
                        ErrorKind::InvalidValue {
                            location: "property/@dataType".into(),
                            found: value.to_owned(),
                            expected: "an upper-case name matching [A-Z]+",
                        },
                    ))
                }
            })
            .transpose()
    }

    fn value(&self, node: Node<'d, 'input>) -> Result<Value> {
        self.attributes(node, &[])?;
        let elements = self.elements(node)?;
        let [child] = elements[..] else {
            return Err(match elements.get(1) {
                Some(extra) => self.error(
                    *extra,
                    ErrorKind::UnexpectedElement {
                        parent: local(node),
                        found: expanded(*extra),
                    },
                ),
                None => self.error(
                    node,
                    ErrorKind::MissingElement {
                        parent: local(node),
                        expected: "simpleValue or xs:restriction",
                    },
                ),
            });
        };
        if is_ids(child, "simpleValue") {
            self.attributes(child, &[])?;
            Ok(Value::Simple(self.text(child)?))
        } else if is_xsd(child, "restriction") {
            Ok(Value::Restriction(Box::new(self.restriction(child)?)))
        } else {
            Err(self.error(
                child,
                ErrorKind::UnexpectedElement {
                    parent: local(node),
                    found: expanded(child),
                },
            ))
        }
    }

    fn restriction(&self, node: Node<'d, 'input>) -> Result<Restriction> {
        self.attributes(node, &["base", "id"])?;
        let base = self.base(node)?;
        let mut restriction = Restriction {
            base,
            ..Restriction::default()
        };
        for child in self.elements(node)? {
            if !child.has_tag_name((XSD_NAMESPACE, child.tag_name().name())) {
                return Err(self.error(
                    child,
                    ErrorKind::UnexpectedElement {
                        parent: "xs:restriction".into(),
                        found: expanded(child),
                    },
                ));
            }
            let name = child.tag_name().name();
            if name == "annotation" {
                continue;
            }
            let Some(facet) = RestrictionFacet::from_name(name) else {
                let kind = if name == "simpleType" || name == "whiteSpace" || name == "assertion" {
                    ErrorKind::Unsupported {
                        element: "xs:restriction".into(),
                        construct: format!("xs:{name}"),
                    }
                } else {
                    ErrorKind::UnexpectedElement {
                        parent: "xs:restriction".into(),
                        found: expanded(child),
                    }
                };
                return Err(self.error(child, kind));
            };
            self.attributes(child, &["value", "id", "fixed"])?;
            for grandchild in self.elements(child)? {
                if !is_xsd(grandchild, "annotation") {
                    return Err(self.error(
                        grandchild,
                        ErrorKind::UnexpectedElement {
                            parent: format!("xs:{name}"),
                            found: expanded(grandchild),
                        },
                    ));
                }
            }
            let value = self.required_attribute(child, "value")?;
            self.apply(&mut restriction, facet, child, value)?;
        }
        Ok(restriction)
    }

    fn apply(
        &self,
        restriction: &mut Restriction,
        facet: RestrictionFacet,
        node: Node<'d, 'input>,
        value: &str,
    ) -> Result<()> {
        let location = || format!("xs:{}/@value", node.tag_name().name());
        let once_text = |slot: &mut Option<String>| -> Result<()> {
            if slot.is_some() {
                return Err(self.error(node, duplicate(node)));
            }
            *slot = Some(value.to_owned());
            Ok(())
        };
        let once_count = |slot: &mut Option<u64>, positive: bool| -> Result<()> {
            if slot.is_some() {
                return Err(self.error(node, duplicate(node)));
            }
            let count = parse_non_negative(value)
                .filter(|count| !positive || *count > 0)
                .ok_or_else(|| {
                    self.error(
                        node,
                        ErrorKind::InvalidValue {
                            location: location(),
                            found: value.to_owned(),
                            expected: if positive {
                                "a positive integer"
                            } else {
                                "a non-negative integer"
                            },
                        },
                    )
                })?;
            *slot = Some(count);
            Ok(())
        };
        match facet {
            RestrictionFacet::Enumeration => restriction.enumeration.push(value.to_owned()),
            RestrictionFacet::Pattern => restriction.patterns.push(value.to_owned()),
            RestrictionFacet::MinInclusive => once_text(&mut restriction.min_inclusive)?,
            RestrictionFacet::MaxInclusive => once_text(&mut restriction.max_inclusive)?,
            RestrictionFacet::MinExclusive => once_text(&mut restriction.min_exclusive)?,
            RestrictionFacet::MaxExclusive => once_text(&mut restriction.max_exclusive)?,
            RestrictionFacet::Length => once_count(&mut restriction.length, false)?,
            RestrictionFacet::MinLength => once_count(&mut restriction.min_length, false)?,
            RestrictionFacet::MaxLength => once_count(&mut restriction.max_length, false)?,
            RestrictionFacet::TotalDigits => once_count(&mut restriction.total_digits, true)?,
            RestrictionFacet::FractionDigits => {
                once_count(&mut restriction.fraction_digits, false)?;
            }
        }
        Ok(())
    }

    /// The local name of `@base`, which must name an XML Schema type.
    fn base(&self, node: Node<'d, 'input>) -> Result<String> {
        let base = self.required_attribute(node, "base")?.trim();
        let (prefix, name) = match base.split_once(':') {
            Some((prefix, name)) => (Some(prefix), name),
            None => (None, base),
        };
        if node.lookup_namespace_uri(prefix) == Some(XSD_NAMESPACE) && XSD_TYPES.contains(&name) {
            Ok(name.to_owned())
        } else {
            Err(self.error(
                node,
                ErrorKind::InvalidValue {
                    location: "xs:restriction/@base".into(),
                    found: base.to_owned(),
                    expected: "an XML Schema built-in simple type, such as xs:string",
                },
            ))
        }
    }

    fn non_negative(&self, node: Node<'d, 'input>, location: &str, value: &str) -> Result<u32> {
        parse_non_negative(value)
            .and_then(|count| u32::try_from(count).ok())
            .ok_or_else(|| {
                self.error(
                    node,
                    ErrorKind::InvalidValue {
                        location: location.to_owned(),
                        found: value.to_owned(),
                        expected: "a non-negative integer",
                    },
                )
            })
    }

    /// Checks that `node` carries only the given unqualified attributes.
    ///
    /// Attributes in the `xml` namespace are allowed anywhere; the root may
    /// also carry schema-location hints.
    fn attributes(&self, node: Node<'d, 'input>, allowed: &[&str]) -> Result<()> {
        let is_root = node == self.document.root_element();
        for attribute in node.attributes() {
            let accepted = match attribute.namespace() {
                None => allowed.contains(&attribute.name()),
                Some(XML_NAMESPACE) => true,
                Some(XSI_NAMESPACE) => {
                    is_root
                        && matches!(
                            attribute.name(),
                            "schemaLocation" | "noNamespaceSchemaLocation"
                        )
                }
                Some(_) => false,
            };
            if !accepted {
                let name = match attribute.namespace() {
                    Some(namespace) => format!("{{{namespace}}}{}", attribute.name()),
                    None => attribute.name().to_owned(),
                };
                return Err(self.error(
                    node,
                    ErrorKind::UnexpectedAttribute {
                        element: local(node),
                        attribute: name,
                    },
                ));
            }
        }
        Ok(())
    }

    fn required_attribute(&self, node: Node<'d, 'input>, name: &'static str) -> Result<&'d str> {
        node.attribute(name).ok_or_else(|| {
            self.error(
                node,
                ErrorKind::MissingAttribute {
                    element: local(node),
                    attribute: name,
                },
            )
        })
    }

    /// The element children of an element-only node.
    fn elements(&self, node: Node<'d, 'input>) -> Result<Vec<Node<'d, 'input>>> {
        let mut elements = Vec::new();
        for child in node.children() {
            if child.is_element() {
                elements.push(child);
            } else if child.is_text() && !child.text().unwrap_or("").chars().all(is_xml_space) {
                return Err(self.error(
                    child,
                    ErrorKind::UnexpectedText {
                        element: local(node),
                    },
                ));
            }
        }
        Ok(elements)
    }

    fn sequence(&self, node: Node<'d, 'input>) -> Result<Sequence<'_, 'd, 'input>> {
        Ok(Sequence {
            reader: self,
            parent: node,
            items: self.elements(node)?,
            next: 0,
        })
    }

    /// The character data of a text-only element, verbatim.
    fn text(&self, node: Node<'d, 'input>) -> Result<String> {
        self.attributes(node, &[])?;
        let mut text = String::new();
        for child in node.children() {
            if child.is_element() {
                return Err(self.error(
                    child,
                    ErrorKind::UnexpectedElement {
                        parent: local(node),
                        found: expanded(child),
                    },
                ));
            }
            if let Some(part) = child.text().filter(|_| child.is_text()) {
                text.push_str(part);
            }
        }
        Ok(text)
    }
}

/// An element's children consumed in schema order.
struct Sequence<'r, 'd, 'input> {
    reader: &'r Reader<'d, 'input>,
    parent: Node<'d, 'input>,
    items: Vec<Node<'d, 'input>>,
    next: usize,
}

impl<'d, 'input> Sequence<'_, 'd, 'input> {
    /// The next child, if it is the IDS element `name`.
    fn optional(&mut self, name: &str) -> Option<Node<'d, 'input>> {
        let node = *self.items.get(self.next)?;
        is_ids(node, name).then(|| {
            self.next += 1;
            node
        })
    }

    fn require(&mut self, name: &'static str) -> Result<Node<'d, 'input>> {
        self.optional(name)
            .ok_or_else(|| match self.items.get(self.next) {
                // Something else stands where `name` must: report that element,
                // it is either misplaced or misspelled.
                Some(found) => self.reader.error(
                    *found,
                    ErrorKind::UnexpectedElement {
                        parent: local(self.parent),
                        found: expanded(*found),
                    },
                ),
                None => self.reader.error(
                    self.parent,
                    ErrorKind::MissingElement {
                        parent: local(self.parent),
                        expected: name,
                    },
                ),
            })
    }

    /// Refuses any child left over: it is out of order or not allowed.
    fn finish(self) -> Result<()> {
        match self.items.get(self.next) {
            Some(found) => Err(self.reader.error(
                *found,
                ErrorKind::UnexpectedElement {
                    parent: local(self.parent),
                    found: expanded(*found),
                },
            )),
            None => Ok(()),
        }
    }
}

/// The built-in simple types of XML Schema 1.0, the revision `ids.xsd`
/// imports. A restriction can only derive from one of these.
const XSD_TYPES: [&str; 45] = [
    "anySimpleType",
    "string",
    "normalizedString",
    "token",
    "language",
    "Name",
    "NCName",
    "ID",
    "IDREF",
    "IDREFS",
    "ENTITY",
    "ENTITIES",
    "NMTOKEN",
    "NMTOKENS",
    "boolean",
    "decimal",
    "integer",
    "nonPositiveInteger",
    "negativeInteger",
    "long",
    "int",
    "short",
    "byte",
    "nonNegativeInteger",
    "unsignedLong",
    "unsignedInt",
    "unsignedShort",
    "unsignedByte",
    "positiveInteger",
    "float",
    "double",
    "duration",
    "dateTime",
    "time",
    "date",
    "gYearMonth",
    "gYear",
    "gMonthDay",
    "gDay",
    "gMonth",
    "hexBinary",
    "base64Binary",
    "anyURI",
    "QName",
    "NOTATION",
];

const FACETS: [&str; 6] = [
    "entity",
    "partOf",
    "classification",
    "attribute",
    "property",
    "material",
];

/// The unqualified attributes a facet element may carry in a role.
fn facet_attributes(kind: &str, role: Role) -> &'static [&'static str] {
    match (kind, role) {
        ("partOf", Role::Applicability) => &["relation"],
        ("property", Role::Applicability) => &["dataType"],
        ("entity", Role::Requirement) => &["instructions"],
        ("partOf", Role::Requirement) => &["relation", "cardinality", "instructions"],
        ("classification" | "material", Role::Requirement) => {
            &["uri", "cardinality", "instructions"]
        }
        ("attribute", Role::Requirement) => &["cardinality", "instructions"],
        ("property", Role::Requirement) => &["dataType", "uri", "cardinality", "instructions"],
        _ => &[],
    }
}

#[derive(Clone, Copy)]
enum RestrictionFacet {
    Enumeration,
    Pattern,
    MinInclusive,
    MaxInclusive,
    MinExclusive,
    MaxExclusive,
    Length,
    MinLength,
    MaxLength,
    TotalDigits,
    FractionDigits,
}

impl RestrictionFacet {
    fn from_name(name: &str) -> Option<RestrictionFacet> {
        Some(match name {
            "enumeration" => RestrictionFacet::Enumeration,
            "pattern" => RestrictionFacet::Pattern,
            "minInclusive" => RestrictionFacet::MinInclusive,
            "maxInclusive" => RestrictionFacet::MaxInclusive,
            "minExclusive" => RestrictionFacet::MinExclusive,
            "maxExclusive" => RestrictionFacet::MaxExclusive,
            "length" => RestrictionFacet::Length,
            "minLength" => RestrictionFacet::MinLength,
            "maxLength" => RestrictionFacet::MaxLength,
            "totalDigits" => RestrictionFacet::TotalDigits,
            "fractionDigits" => RestrictionFacet::FractionDigits,
            _ => return None,
        })
    }
}

fn duplicate(node: Node<'_, '_>) -> ErrorKind {
    ErrorKind::UnexpectedElement {
        parent: "xs:restriction".into(),
        found: format!("a second {}", expanded(node)),
    }
}

/// The revision `xsi:schemaLocation` names for the IDS namespace, if any.
///
/// Published schemas live at `.../IDS/<version>/ids.xsd`, so the segment
/// before the file name is the revision. A location of any other shape
/// declares nothing.
fn declared_version(root: Node<'_, '_>) -> Option<IdsVersion> {
    let hint = root.attribute((XSI_NAMESPACE, "schemaLocation"))?;
    let tokens: Vec<&str> = hint.split(is_xml_space).filter(|t| !t.is_empty()).collect();
    tokens.chunks(2).find_map(|pair| match pair {
        [namespace, location] if *namespace == NAMESPACE => {
            let mut segments = location.rsplit('/');
            let file = segments.next()?;
            if !file.to_ascii_lowercase().ends_with(".xsd") {
                return None;
            }
            segments.next()?.parse().ok()
        }
        _ => None,
    })
}

/// Every revision-distinguishing shape in the document, collected leniently.
fn signals(root: Node<'_, '_>) -> VersionSignals {
    let mut signals = VersionSignals::new();
    for node in root.descendants().filter(|n| n.is_element()) {
        if node.tag_name().namespace() != Some(NAMESPACE) {
            continue;
        }
        let name = node.tag_name().name();
        if name == "specification" {
            for token in node
                .attribute("ifcVersion")
                .unwrap_or("")
                .split(is_xml_space)
            {
                match token {
                    "IFC4X3" => signals.observe(Signal::Ifc4x3Unsuffixed),
                    "IFC4X3_ADD2" => signals.observe(Signal::Ifc4x3Add2),
                    _ => &mut signals,
                };
            }
        }
        let in_requirements = node
            .parent_element()
            .is_some_and(|parent| is_ids(parent, "requirements"));
        if in_requirements && FACETS.contains(&name) {
            if node.has_attribute("minOccurs") || node.has_attribute("maxOccurs") {
                signals.observe(Signal::OccursOnRequirementFacet);
            }
            if node.has_attribute("cardinality") {
                signals.observe(Signal::CardinalityOnRequirementFacet);
            }
        }
        if name == "property" {
            for child in node
                .children()
                .filter(|c| c.tag_name().namespace() == Some(NAMESPACE))
            {
                match child.tag_name().name() {
                    "name" => signals.observe(Signal::PropertyName),
                    "baseName" => signals.observe(Signal::PropertyBaseName),
                    _ => &mut signals,
                };
            }
            if node.has_attribute("measure") {
                signals.observe(Signal::MeasureAttribute);
            }
            if node.has_attribute("dataType") {
                signals.observe(Signal::DataTypeAttribute);
            }
        }
        if name == "classification" && !node.children().any(|c| is_ids(c, "system")) {
            signals.observe(Signal::ClassificationWithoutSystem);
        }
    }
    signals
}

fn is_ids(node: Node<'_, '_>, name: &str) -> bool {
    node.is_element() && node.has_tag_name((NAMESPACE, name))
}

fn is_xsd(node: Node<'_, '_>, name: &str) -> bool {
    node.is_element() && node.has_tag_name((XSD_NAMESPACE, name))
}

fn local(node: Node<'_, '_>) -> String {
    node.tag_name().name().to_owned()
}

fn expanded(node: Node<'_, '_>) -> String {
    match node.tag_name().namespace() {
        Some(namespace) => format!("{{{namespace}}}{}", node.tag_name().name()),
        None => node.tag_name().name().to_owned(),
    }
}

fn attribute(node: Node<'_, '_>, name: &str) -> Option<String> {
    node.attribute(name).map(ToOwned::to_owned)
}

fn is_xml_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r')
}

/// An `xs:nonNegativeInteger` after whitespace collapsing: an optional `+`
/// and at least one digit.
fn parse_non_negative(value: &str) -> Option<u64> {
    let digits = value.trim_matches(is_xml_space);
    let digits = digits.strip_prefix('+').unwrap_or(digits);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// The schema's `author` pattern `[^@]+@[^\.]+\..+`, which XML Schema
/// anchors at both ends.
fn is_author(value: &str) -> bool {
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    let Some((host, rest)) = domain.split_once('.') else {
        return false;
    };
    // `.` matches anything but a line break.
    !local.is_empty() && !host.is_empty() && !rest.is_empty() && !rest.contains(['\n', '\r'])
}

/// The lexical form of `xs:date`: `-?YYYY-MM-DD` and an optional zone.
fn is_xs_date(value: &str) -> bool {
    let unsigned = value.strip_prefix('-').unwrap_or(value);
    let (date, zone) = match unsigned.find(['Z', '+']).or_else(|| {
        // A zone's `-` comes after the day, so search past `YYYY-MM-DD`.
        unsigned
            .char_indices()
            .filter(|(_, c)| *c == '-')
            .nth(2)
            .map(|(i, _)| i)
    }) {
        Some(split) => unsigned.split_at(split),
        None => (unsigned, ""),
    };
    let parts: Vec<&str> = date.split('-').collect();
    let [year, month, day] = parts[..] else {
        return false;
    };
    let digits = |s: &str, len: usize| s.len() == len && s.bytes().all(|b| b.is_ascii_digit());
    let year_ok = year.len() >= 4
        && year.bytes().all(|b| b.is_ascii_digit())
        && (year.len() == 4 || !year.starts_with('0'));
    if !year_ok || !digits(month, 2) || !digits(day, 2) {
        return false;
    }
    let (Ok(year), Ok(month), Ok(day)) = (
        year.parse::<u64>(),
        month.parse::<u32>(),
        day.parse::<u32>(),
    ) else {
        return false;
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    if day == 0 || day > days {
        return false;
    }
    is_zone(zone)
}

fn is_zone(zone: &str) -> bool {
    if zone.is_empty() || zone == "Z" {
        return true;
    }
    let Some(offset) = zone.strip_prefix(['+', '-']) else {
        return false;
    };
    let Some((hours, minutes)) = offset.split_once(':') else {
        return false;
    };
    let two = |s: &str| s.len() == 2 && s.bytes().all(|b| b.is_ascii_digit());
    if !two(hours) || !two(minutes) {
        return false;
    }
    let (hours, minutes): (u32, u32) = (hours.parse().unwrap_or(99), minutes.parse().unwrap_or(99));
    (hours < 14 && minutes < 60) || (hours == 14 && minutes == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn author_follows_the_schema_pattern() {
        for good in ["a@b.c", "first.last@example.org", "x@y.z.w"] {
            assert!(is_author(good), "{good}");
        }
        for bad in ["", "a@b", "@b.c", "a@.c", "a@b.", "ab.c", "a@b.c\n"] {
            assert!(!is_author(bad), "{bad:?}");
        }
    }

    #[test]
    fn dates_follow_the_xs_date_lexical_form() {
        for good in [
            "2024-06-01",
            "2024-02-29",
            "2000-02-29",
            "2024-06-01Z",
            "2024-06-01+02:00",
            "2024-06-01-14:00",
            "-0044-03-15",
            "12024-01-01",
        ] {
            assert!(is_xs_date(good), "{good}");
        }
        for bad in [
            "",
            "2024-6-1",
            "2023-02-29",
            "1900-02-29",
            "2024-13-01",
            "2024-00-10",
            "2024-06-31",
            "24-06-01",
            "02024-01-01",
            "2024-06-01T00:00:00",
            "2024-06-01+2:00",
            "2024-06-01+14:30",
            "2024/06/01",
        ] {
            assert!(!is_xs_date(bad), "{bad}");
        }
    }

    #[test]
    fn non_negative_integers_follow_the_lexical_form() {
        assert_eq!(parse_non_negative("0"), Some(0));
        assert_eq!(parse_non_negative(" +007 "), Some(7));
        for bad in ["", "-1", "1.0", "one", "+", "1 2"] {
            assert_eq!(parse_non_negative(bad), None, "{bad:?}");
        }
    }
}
