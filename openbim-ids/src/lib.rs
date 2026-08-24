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
//! # Why this is not in `packages/`
//!
//! IDS is a *consumer* of the IFC layer, not part of it. Nothing in
//! `packages/` may depend on it. That one-way rule is what stops the IFC
//! core from accreting every standard that happens to use it.
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

    /// Whether this version is an approved standard rather than a draft.
    #[must_use]
    pub fn is_approved(self) -> bool {
        matches!(self, IdsVersion::Ids1_0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
