//! Occurrence, modelled where each IDS revision puts it.
//!
//! # Why this type exists
//!
//! IDS moved occurrence between revisions rather than renaming it:
//!
//! - **0.9** carries `xs:occurs` (`minOccurs` / `maxOccurs`) on
//!   `specificationType` and on every requirement facet.
//! - **1.0** moved occurrence onto `<applicability>`, and requirement facets
//!   instead carry `@cardinality` of `required`, `prohibited` or `optional`.
//!
//! Both spell the same three intents. Keeping the revision's *spelling* in the
//! typed model would mean a 0.9 document and a 1.0 document stating identical
//! requirements produce unequal output, and every consumer would have to learn
//! both encodings. So both lower into this one type, and the revision-specific
//! spelling survives only as the function that produced it.

/// What a requirement demands of the elements a specification applies to.
///
/// This is the normalised form: [`from_xs_occurs`](Occurrence::from_xs_occurs)
/// lowers the 0.9 spelling and [`from_cardinality`](Occurrence::from_cardinality)
/// the 1.0 spelling, so equal intent compares equal across revisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Occurrence {
    /// The facet must be present. The 1.0 `required` cardinality, and the 0.9
    /// spelling `minOccurs >= 1`.
    Required,
    /// The facet must be absent. The 1.0 `prohibited` cardinality, and the 0.9
    /// spelling `maxOccurs = 0`.
    Prohibited,
    /// The facet may be present or absent. The 1.0 `optional` cardinality, and
    /// the 0.9 spelling `minOccurs = 0` with a non-zero `maxOccurs`.
    Optional,
}

impl Occurrence {
    /// The schema default.
    ///
    /// Load-bearing, and the reason it is named rather than left implicit: real
    /// IDS files lean on it. A facet writing no `@cardinality` at all means
    /// *required*, so a reader that treats "absent" as "optional" silently
    /// drops requirements.
    pub const DEFAULT: Occurrence = Occurrence::Required;

    /// Lowers the IDS 1.0 `@cardinality` attribute.
    ///
    /// ```
    /// use openbim_ids::Occurrence;
    /// assert_eq!(Occurrence::from_cardinality("prohibited"), Some(Occurrence::Prohibited));
    /// assert_eq!(Occurrence::from_cardinality("Required"), None);
    /// ```
    ///
    /// Matching is case-sensitive because the schema enumerates lower-case
    /// tokens; accepting other spellings would mean accepting documents the
    /// schema rejects.
    #[must_use]
    pub fn from_cardinality(value: &str) -> Option<Occurrence> {
        match value {
            "required" => Some(Occurrence::Required),
            "prohibited" => Some(Occurrence::Prohibited),
            "optional" => Some(Occurrence::Optional),
            _ => None,
        }
    }

    /// The IDS 1.0 `@cardinality` spelling of this occurrence.
    #[must_use]
    pub fn as_cardinality(self) -> &'static str {
        match self {
            Occurrence::Required => "required",
            Occurrence::Prohibited => "prohibited",
            Occurrence::Optional => "optional",
        }
    }

    /// Lowers the IDS 0.9 `xs:occurs` spelling.
    ///
    /// `max` is `None` for `maxOccurs="unbounded"`. Absent attributes must be
    /// passed as their schema defaults of `1`, not as `0` — see
    /// [`Occurrence::DEFAULT`].
    ///
    /// ```
    /// use openbim_ids::Occurrence;
    /// // The 0.9 and 1.0 spellings of the same intent agree.
    /// assert_eq!(
    ///     Occurrence::from_xs_occurs(0, Some(0)),
    ///     Occurrence::from_cardinality("prohibited").unwrap()
    /// );
    /// assert_eq!(Occurrence::from_xs_occurs(1, None), Occurrence::Required);
    /// ```
    #[must_use]
    pub fn from_xs_occurs(min: u32, max: Option<u32>) -> Occurrence {
        // Order matters: `minOccurs="0" maxOccurs="0"` is prohibited, not
        // optional. Testing the zero maximum first keeps that unambiguous.
        if max == Some(0) {
            Occurrence::Prohibited
        } else if min == 0 {
            Occurrence::Optional
        } else {
            Occurrence::Required
        }
    }

    /// Whether the facet is allowed to appear at all.
    #[must_use]
    pub fn permits_presence(self) -> bool {
        !matches!(self, Occurrence::Prohibited)
    }
}

impl Default for Occurrence {
    fn default() -> Self {
        Occurrence::DEFAULT
    }
}

impl core::fmt::Display for Occurrence {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_cardinality())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Occurrence; 3] = [
        Occurrence::Required,
        Occurrence::Prohibited,
        Occurrence::Optional,
    ];

    #[test]
    fn cardinality_round_trips() {
        for occurrence in ALL {
            assert_eq!(
                Occurrence::from_cardinality(occurrence.as_cardinality()),
                Some(occurrence)
            );
        }
    }

    #[test]
    fn unknown_cardinality_is_rejected() {
        for bad in ["", "Required", "REQUIRED", "mandatory", "forbidden"] {
            assert!(
                Occurrence::from_cardinality(bad).is_none(),
                "`{bad}` must not parse"
            );
        }
    }

    /// The defining property: revisions disagree on spelling, not on meaning.
    #[test]
    fn revisions_agree_on_equal_intent() {
        // required: 0.9 `minOccurs=1`, 1.0 `cardinality="required"`
        assert_eq!(
            Occurrence::from_xs_occurs(1, Some(1)),
            Occurrence::from_cardinality("required").expect("required")
        );
        // optional: 0.9 `minOccurs=0 maxOccurs=1`, 1.0 `cardinality="optional"`
        assert_eq!(
            Occurrence::from_xs_occurs(0, Some(1)),
            Occurrence::from_cardinality("optional").expect("optional")
        );
        // prohibited: 0.9 `maxOccurs=0`, 1.0 `cardinality="prohibited"`
        assert_eq!(
            Occurrence::from_xs_occurs(0, Some(0)),
            Occurrence::from_cardinality("prohibited").expect("prohibited")
        );
    }

    #[test]
    fn unbounded_maximum_is_required_not_optional() {
        assert_eq!(Occurrence::from_xs_occurs(1, None), Occurrence::Required);
        assert_eq!(Occurrence::from_xs_occurs(0, None), Occurrence::Optional);
    }

    /// `maxOccurs="0"` wins over `minOccurs="0"`.
    #[test]
    fn zero_maximum_is_prohibited_even_with_zero_minimum() {
        assert_eq!(
            Occurrence::from_xs_occurs(0, Some(0)),
            Occurrence::Prohibited
        );
    }

    /// The schema default is required; reading absent as optional drops
    /// requirements silently.
    #[test]
    fn default_is_required() {
        assert_eq!(Occurrence::default(), Occurrence::Required);
        assert_eq!(Occurrence::DEFAULT, Occurrence::Required);
    }

    #[test]
    fn only_prohibited_forbids_presence() {
        assert!(Occurrence::Required.permits_presence());
        assert!(Occurrence::Optional.permits_presence());
        assert!(!Occurrence::Prohibited.permits_presence());
    }

    #[test]
    fn display_matches_cardinality() {
        for occurrence in ALL {
            assert_eq!(occurrence.to_string(), occurrence.as_cardinality());
        }
    }
}
