//! `openbim-ids` — buildingSMART Information Delivery Specification.
//!
//! # What this is
//!
//! The standard, machine-readable way to state *"this model must contain these
//! things, with these properties"* and audit a model against it. It is the
//! highest-leverage openBIM standard for real projects, because it turns
//! contractual information requirements into an automated check.
//!
//! # 🚨 One namespace, six schema versions
//!
//! Every published IDS version from 0.2 to 1.0 declares the **same**
//! `targetNamespace`. The namespace identifies the format, never the version.
//! Because the differences are in attribute *names* and cardinality rather
//! than element names, a reader that guesses wrong does not fail — it silently
//! produces a *different* specification.
//!
//! Version detection must therefore report how it knows, and must surface
//! disagreement between a file's claim and its shape instead of picking one.
//! `openbim_core::Detected` exists for exactly this.
//!
//! Only 1.0 is an approved buildingSMART standard. Older versions are worth
//! *reading* because files using them exist; new documents should be 1.0.
//!
//! # Reporting discipline
//!
//! An audit that quietly treats "property missing" as "check passed" is worse
//! than no audit. Results distinguish applicable-and-passed,
//! applicable-and-failed, and not-applicable — see `openbim_core::Outcome`.
//!
//! # Repository boundary
//!
//! IDS is a *consumer* of the IFC layer, not part of it. The IFC layer must
//! never depend on this crate. Shared vocabulary belongs in `openbim-core`, and
//! the `openbimrs/openbim` integration repository pins compatible revisions of
//! the repositories without reversing that dependency direction.
//!
//! # Status
//!
//! **Reserved — no implementation.** Published to establish the name.
//!
//! An oracle already exists on disk: the buildingSMART IDS test corpus carries
//! `pass-`/`fail-` cases, so the acceptance bar for the implementation is that
//! every `pass-` case passes and every `fail-` case fails, with not-applicable
//! distinguished from passed.

#![forbid(unsafe_code)]

pub mod occurrence;
pub mod version;

pub use occurrence::Occurrence;
pub use version::{detect_version, Signal, VersionSignals};

/// The XML namespace shared by **all** IDS versions.
///
/// Deliberately a single constant: there is no per-version namespace to key
/// on, which is the whole difficulty of reading IDS.
pub const NAMESPACE: &str = "http://standards.buildingsmart.org/IDS";

/// A published IDS schema version.
///
/// Ordered oldest to newest; `Ids1_0` is the only approved standard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IdsVersion {
    /// 0.9 and earlier pre-release drafts.
    Draft0_9,
    /// 0.9.6.
    Draft0_9_6,
    /// 0.9.7.
    Draft0_9_7,
    /// 1.0 — the approved buildingSMART standard.
    Ids1_0,
}

impl IdsVersion {
    /// The version new documents should be written as.
    ///
    /// Writing anything older is a deliberate compatibility choice, never a
    /// default.
    pub const CURRENT: IdsVersion = IdsVersion::Ids1_0;

    /// Every published version, oldest first.
    ///
    /// Exposed so callers — and the tests here — can iterate versions without
    /// hand-listing variants and silently omitting a newly added one.
    ///
    /// ```
    /// use openbim_ids::IdsVersion;
    /// assert!(IdsVersion::ALL.contains(&IdsVersion::CURRENT));
    /// ```
    pub const ALL: [IdsVersion; 4] = [
        IdsVersion::Draft0_9,
        IdsVersion::Draft0_9_6,
        IdsVersion::Draft0_9_7,
        IdsVersion::Ids1_0,
    ];

    /// Whether this version is an approved standard rather than a draft.
    #[must_use]
    pub fn is_approved(self) -> bool {
        matches!(self, IdsVersion::Ids1_0)
    }

    /// The canonical release string, as it appears in the schema location path.
    ///
    /// IDS schemas are published under
    /// `http://standards.buildingsmart.org/IDS/<version>/ids.xsd`, so these are
    /// the forms that actually occur in an `xsi:schemaLocation` hint.
    ///
    /// ```
    /// use openbim_ids::IdsVersion;
    /// assert_eq!(IdsVersion::Ids1_0.as_str(), "1.0");
    /// ```
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            IdsVersion::Draft0_9 => "0.9",
            IdsVersion::Draft0_9_6 => "0.9.6",
            IdsVersion::Draft0_9_7 => "0.9.7",
            IdsVersion::Ids1_0 => "1.0",
        }
    }
}

impl core::fmt::Display for IdsVersion {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The error returned when a string does not name a published IDS version.
///
/// Carries the rejected input so a diagnostic can quote it without the call
/// site having to keep the original string alive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseIdsVersionError {
    input: String,
}

impl ParseIdsVersionError {
    /// The string that failed to parse.
    #[must_use]
    pub fn input(&self) -> &str {
        &self.input
    }
}

impl core::fmt::Display for ParseIdsVersionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "`{}` is not a published IDS version (expected one of 0.9, 0.9.6, 0.9.7, 1.0)",
            self.input
        )
    }
}

impl std::error::Error for ParseIdsVersionError {}

impl core::str::FromStr for IdsVersion {
    type Err = ParseIdsVersionError;

    /// Parses a published IDS version string.
    ///
    /// Accepts the canonical release forms produced by [`IdsVersion::as_str`],
    /// and additionally the three-component spellings that the schema files
    /// themselves carry in their own `@version` attribute — `ids.xsd` for the
    /// approved standard declares `version="1.0.0"`, not `"1.0"`. Rejecting
    /// that spelling would mean a version read straight out of a schema failed
    /// to parse.
    ///
    /// ```
    /// use openbim_ids::IdsVersion;
    /// assert_eq!("1.0".parse(), Ok(IdsVersion::Ids1_0));
    /// assert_eq!("1.0.0".parse(), Ok(IdsVersion::Ids1_0));
    /// assert!("0.8".parse::<IdsVersion>().is_err());
    /// ```
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "0.9" | "0.9.0" => Ok(IdsVersion::Draft0_9),
            "0.9.6" => Ok(IdsVersion::Draft0_9_6),
            "0.9.7" => Ok(IdsVersion::Draft0_9_7),
            "1.0" | "1.0.0" => Ok(IdsVersion::Ids1_0),
            other => Err(ParseIdsVersionError {
                input: other.to_owned(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [IdsVersion; 4] = IdsVersion::ALL;

    /// `ALL` must list every variant.
    ///
    /// The exhaustive `match` is the enforcement: adding a variant without
    /// extending `ALL` fails to compile here, so the iterating tests below can
    /// never silently skip a version.
    #[test]
    fn all_lists_every_variant() {
        for version in ALL {
            match version {
                IdsVersion::Draft0_9
                | IdsVersion::Draft0_9_6
                | IdsVersion::Draft0_9_7
                | IdsVersion::Ids1_0 => {}
            }
        }
        assert_eq!(ALL.len(), 4);
        let unique: std::collections::BTreeSet<_> = ALL.iter().collect();
        assert_eq!(unique.len(), ALL.len(), "ALL contains a duplicate");
    }

    #[test]
    fn all_is_ordered_oldest_first() {
        assert!(ALL.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn only_1_0_is_approved() {
        assert!(IdsVersion::Ids1_0.is_approved());
        assert!(!IdsVersion::Draft0_9.is_approved());
        assert!(!IdsVersion::Draft0_9_6.is_approved());
        assert!(!IdsVersion::Draft0_9_7.is_approved());
    }

    #[test]
    fn current_is_the_newest_version() {
        assert_eq!(IdsVersion::CURRENT, IdsVersion::Ids1_0);
        assert!(IdsVersion::Ids1_0 > IdsVersion::Draft0_9_7);
        assert!(IdsVersion::Draft0_9_7 > IdsVersion::Draft0_9);
    }

    /// Every version must survive a text round trip, for all variants.
    ///
    /// Enumerated exhaustively rather than spot-checked: the failure this
    /// guards against is a new variant getting an `as_str` arm that `from_str`
    /// does not accept, which only shows up on the variant nobody tested.
    #[test]
    fn every_version_round_trips_through_text() {
        for version in ALL {
            assert_eq!(
                version.as_str().parse::<IdsVersion>(),
                Ok(version),
                "{version} did not round trip"
            );
        }
    }

    #[test]
    fn display_matches_as_str() {
        for version in ALL {
            assert_eq!(version.to_string(), version.as_str());
        }
    }

    #[test]
    fn version_strings_are_distinct() {
        let mut seen = std::collections::BTreeSet::new();
        for version in ALL {
            assert!(seen.insert(version.as_str()), "duplicate {version}");
        }
    }

    /// The schema files declare three-component versions; accept those too.
    #[test]
    fn schema_file_version_spelling_parses() {
        assert_eq!("1.0.0".parse(), Ok(IdsVersion::Ids1_0));
        assert_eq!("0.9.0".parse(), Ok(IdsVersion::Draft0_9));
    }

    #[test]
    fn surrounding_whitespace_is_tolerated() {
        assert_eq!("  1.0\n".parse(), Ok(IdsVersion::Ids1_0));
    }

    #[test]
    fn unknown_versions_are_rejected_and_quote_the_input() {
        let err = "0.8".parse::<IdsVersion>().expect_err("must reject 0.8");
        assert_eq!(err.input(), "0.8");
        assert!(err.to_string().contains("0.8"), "{err}");

        for bad in ["", "1", "1.0.1", "v1.0", "IDS1.0"] {
            assert!(
                bad.parse::<IdsVersion>().is_err(),
                "`{bad}` must not parse as a published version"
            );
        }
    }
}
