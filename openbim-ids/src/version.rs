//! Version detection that reports the evidence it used.
//!
//! # The hazard
//!
//! Every published IDS revision declares the same `targetNamespace`, so the
//! namespace cannot discriminate. The revisions differ in attribute *names*
//! and in where occurrence lives, not in element names — so a reader that
//! guesses wrong does not fail. It produces a *different specification*,
//! silently, and an audit run against it reports confident wrong answers.
//!
//! This module therefore never guesses. It collects the shape signals a
//! document exhibits, infers a revision from them, and compares that against
//! whatever the document declared. Disagreement becomes
//! [`Detected::Conflict`], which [`Detected::resolved`] refuses to collapse.
//!
//! # Scope
//!
//! [`VersionSignals`] is the evidence *model*, deliberately independent of any
//! XML reader: it takes observations and decides. Populating it from a document
//! belongs to the parser (`IDS-PARSE`), which keeps the detection rules
//! testable now and keeps this crate free of a parser dependency it does not
//! yet need.

use crate::IdsVersion;
use openbim_core::Detected;

/// A shape signal observed in a document, and the revision it indicates.
///
/// Each variant is an *observation*, not a conclusion — a document can exhibit
/// signals pointing at different revisions, which is precisely the case that
/// must be reported rather than resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Signal {
    /// A requirement facet carries `minOccurs` / `maxOccurs`.
    ///
    /// Pre-1.0: 1.0 moved occurrence onto `<applicability>` and gave facets
    /// `@cardinality` instead. This is the single most reliable tell.
    OccursOnRequirementFacet,
    /// A requirement facet carries `@cardinality`.
    ///
    /// 1.0 only — the attribute does not exist before it.
    CardinalityOnRequirementFacet,
    /// A property facet uses `<name>`.
    ///
    /// Pre-1.0: renamed to `<baseName>` in 1.0.
    PropertyName,
    /// A property facet uses `<baseName>`.
    ///
    /// 1.0 only.
    PropertyBaseName,
    /// A property facet carries `@measure`.
    ///
    /// Pre-1.0: renamed to `@dataType` in 1.0.
    MeasureAttribute,
    /// A property facet carries `@dataType`.
    ///
    /// 1.0 only.
    DataTypeAttribute,
    /// An `ifcVersion` lists `IFC4X3`.
    ///
    /// Pre-1.0: 1.0 spells it `IFC4X3_ADD2`.
    Ifc4x3Unsuffixed,
    /// An `ifcVersion` lists `IFC4X3_ADD2`.
    ///
    /// 1.0 only.
    Ifc4x3Add2,
    /// A classification facet omits `<system>`.
    ///
    /// Pre-1.0: `<system>` became required in 1.0. Weaker than the others —
    /// its *presence* proves nothing, since 0.9 permits it too.
    ClassificationWithoutSystem,
}

impl Signal {
    /// The revision this signal indicates, or `None` if it only excludes 1.0.
    ///
    /// Pre-1.0 signals cannot pick between 0.9, 0.9.6 and 0.9.7 — those
    /// revisions are not distinguished by the shapes listed here, so claiming
    /// a specific draft would be an invention. They resolve to
    /// [`IdsVersion::Draft0_9`] as the least specific pre-1.0 revision.
    #[must_use]
    pub fn indicates(self) -> IdsVersion {
        match self {
            Signal::CardinalityOnRequirementFacet
            | Signal::PropertyBaseName
            | Signal::DataTypeAttribute
            | Signal::Ifc4x3Add2 => IdsVersion::Ids1_0,
            Signal::OccursOnRequirementFacet
            | Signal::PropertyName
            | Signal::MeasureAttribute
            | Signal::Ifc4x3Unsuffixed
            | Signal::ClassificationWithoutSystem => IdsVersion::Draft0_9,
        }
    }

    /// Whether this signal is exclusive to IDS 1.0.
    #[must_use]
    pub fn is_1_0_only(self) -> bool {
        self.indicates() == IdsVersion::Ids1_0
    }
}

/// The shape evidence a document exhibits.
///
/// Built by the reader as it walks a document, then interrogated once at the
/// end — a single facet is not enough to judge a file, and a mixed document
/// must be reported as mixed rather than decided by whichever facet was seen
/// last.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VersionSignals {
    observed: std::collections::BTreeSet<Signal>,
}

impl VersionSignals {
    /// Evidence with nothing observed yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a signal. Repeat observations are idempotent.
    pub fn observe(&mut self, signal: Signal) -> &mut Self {
        self.observed.insert(signal);
        self
    }

    /// Whether a given signal was observed.
    #[must_use]
    pub fn saw(&self, signal: Signal) -> bool {
        self.observed.contains(&signal)
    }

    /// Whether any signal was observed at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.observed.is_empty()
    }

    /// Every observed signal, in a stable order.
    pub fn iter(&self) -> impl Iterator<Item = Signal> + '_ {
        self.observed.iter().copied()
    }

    /// The observed signals that are exclusive to 1.0.
    #[must_use]
    pub fn signals_for_1_0(&self) -> Vec<Signal> {
        self.iter().filter(|s| s.is_1_0_only()).collect()
    }

    /// The observed signals that indicate a pre-1.0 draft.
    #[must_use]
    pub fn signals_for_draft(&self) -> Vec<Signal> {
        self.iter().filter(|s| !s.is_1_0_only()).collect()
    }

    /// Whether the document mixes shapes from incompatible revisions.
    ///
    /// A document doing this is malformed under *both* revisions. It is
    /// reported, never repaired: choosing which half to believe changes which
    /// requirements exist.
    #[must_use]
    pub fn is_internally_inconsistent(&self) -> bool {
        !self.signals_for_1_0().is_empty() && !self.signals_for_draft().is_empty()
    }

    /// The revision the shape indicates, if the evidence is unambiguous.
    ///
    /// Returns `None` when nothing was observed, and also when the evidence is
    /// self-contradictory — both are cases where a caller must not receive a
    /// confident answer.
    #[must_use]
    pub fn infer(&self) -> Option<IdsVersion> {
        if self.is_empty() || self.is_internally_inconsistent() {
            return None;
        }
        Some(if self.signals_for_1_0().is_empty() {
            IdsVersion::Draft0_9
        } else {
            IdsVersion::Ids1_0
        })
    }
}

/// Resolves a document's version from what it declared and what it looks like.
///
/// The four outcomes:
///
/// - declared, and the shape agrees (or is silent) → [`Detected::Declared`]
/// - nothing declared, shape unambiguous → [`Detected::Inferred`]
/// - declared, shape says otherwise → [`Detected::Conflict`]
/// - nothing declared and no usable shape evidence → `None`
///
/// The conflict case is the one that matters: a document declaring 1.0 while
/// carrying `minOccurs` on a requirement facet is a pre-1.0 draft with a
/// rewritten header, and reading it as 1.0 loses every occurrence constraint
/// without erroring.
///
/// ```
/// use openbim_ids::{detect_version, IdsVersion, Signal, VersionSignals};
/// use openbim_core::Detected;
///
/// let mut signals = VersionSignals::new();
/// signals.observe(Signal::OccursOnRequirementFacet);
///
/// // A 1.0 claim over a pre-1.0 shape must not resolve.
/// let detected = detect_version(Some(IdsVersion::Ids1_0), &signals).expect("evidence");
/// assert!(detected.is_conflict());
/// assert_eq!(detected.resolved(), None);
/// ```
#[must_use]
pub fn detect_version(
    declared: Option<IdsVersion>,
    signals: &VersionSignals,
) -> Option<Detected<IdsVersion>> {
    let observed = signals.infer();
    match (declared, observed) {
        // Both present: agreement confirms the claim, disagreement is reported.
        (Some(declared), Some(observed)) => Some(if versions_agree(declared, observed) {
            Detected::Declared(declared)
        } else {
            Detected::Conflict { declared, observed }
        }),
        // A claim with no usable shape evidence is taken at face value; there
        // is nothing to contradict it. An internally inconsistent document
        // yields no inference, so it lands here and keeps the declared value.
        (Some(declared), None) => Some(Detected::Declared(declared)),
        // No claim, but the shape is unambiguous.
        (None, Some(observed)) => Some(Detected::Inferred(observed)),
        (None, None) => None,
    }
}

/// Whether a declared revision is consistent with an inferred one.
///
/// Shape inference cannot separate the pre-1.0 drafts from each other, so any
/// declared draft is consistent with an inferred `Draft0_9`. Treating a
/// declared 0.9.7 as conflicting with an inferred "some draft" would be a false
/// positive, and false conflicts train callers to ignore real ones.
fn versions_agree(declared: IdsVersion, observed: IdsVersion) -> bool {
    match observed {
        IdsVersion::Ids1_0 => declared == IdsVersion::Ids1_0,
        _ => declared != IdsVersion::Ids1_0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signals(list: &[Signal]) -> VersionSignals {
        let mut s = VersionSignals::new();
        for signal in list {
            s.observe(*signal);
        }
        s
    }

    #[test]
    fn every_signal_indicates_a_revision() {
        let all = [
            Signal::OccursOnRequirementFacet,
            Signal::CardinalityOnRequirementFacet,
            Signal::PropertyName,
            Signal::PropertyBaseName,
            Signal::MeasureAttribute,
            Signal::DataTypeAttribute,
            Signal::Ifc4x3Unsuffixed,
            Signal::Ifc4x3Add2,
            Signal::ClassificationWithoutSystem,
        ];
        for signal in all {
            // Exhaustive match: a new variant without an `indicates` arm fails
            // to compile, so it cannot silently default to a revision.
            assert!(matches!(
                signal.indicates(),
                IdsVersion::Ids1_0 | IdsVersion::Draft0_9
            ));
        }
        assert_eq!(all.iter().filter(|s| s.is_1_0_only()).count(), 4);
    }

    /// The headline case from the issue.
    #[test]
    fn occurs_on_a_requirement_facet_is_a_pre_1_0_tell() {
        let s = signals(&[Signal::OccursOnRequirementFacet]);
        assert_eq!(s.infer(), Some(IdsVersion::Draft0_9));
    }

    #[test]
    fn cardinality_on_a_requirement_facet_is_1_0() {
        let s = signals(&[Signal::CardinalityOnRequirementFacet]);
        assert_eq!(s.infer(), Some(IdsVersion::Ids1_0));
    }

    /// A 1.0 document carrying `minOccurs` must be reported, not parsed.
    #[test]
    fn declared_1_0_over_a_draft_shape_is_a_conflict() {
        let s = signals(&[Signal::OccursOnRequirementFacet]);
        let detected = detect_version(Some(IdsVersion::Ids1_0), &s).expect("evidence");
        assert_eq!(
            detected,
            Detected::Conflict {
                declared: IdsVersion::Ids1_0,
                observed: IdsVersion::Draft0_9,
            }
        );
        assert!(detected.is_conflict());
        assert_eq!(detected.resolved(), None, "a conflict must not resolve");
    }

    #[test]
    fn declared_draft_over_a_1_0_shape_is_a_conflict() {
        let s = signals(&[Signal::CardinalityOnRequirementFacet]);
        let detected = detect_version(Some(IdsVersion::Draft0_9_7), &s).expect("evidence");
        assert!(detected.is_conflict());
    }

    #[test]
    fn agreement_is_declared_not_inferred() {
        let s = signals(&[
            Signal::CardinalityOnRequirementFacet,
            Signal::PropertyBaseName,
        ]);
        assert_eq!(
            detect_version(Some(IdsVersion::Ids1_0), &s),
            Some(Detected::Declared(IdsVersion::Ids1_0))
        );
    }

    #[test]
    fn shape_alone_infers() {
        let s = signals(&[Signal::PropertyBaseName, Signal::DataTypeAttribute]);
        assert_eq!(
            detect_version(None, &s),
            Some(Detected::Inferred(IdsVersion::Ids1_0))
        );
    }

    #[test]
    fn no_declaration_and_no_evidence_yields_nothing() {
        assert_eq!(detect_version(None, &VersionSignals::new()), None);
    }

    #[test]
    fn a_declaration_with_no_evidence_stands() {
        assert_eq!(
            detect_version(Some(IdsVersion::Ids1_0), &VersionSignals::new()),
            Some(Detected::Declared(IdsVersion::Ids1_0))
        );
    }

    /// Inference cannot separate the drafts, so declaring one is not a conflict.
    #[test]
    fn a_declared_draft_does_not_conflict_with_an_inferred_draft() {
        let s = signals(&[Signal::PropertyName]);
        for declared in [
            IdsVersion::Draft0_9,
            IdsVersion::Draft0_9_6,
            IdsVersion::Draft0_9_7,
        ] {
            let detected = detect_version(Some(declared), &s).expect("evidence");
            assert_eq!(
                detected,
                Detected::Declared(declared),
                "{declared} must not be reported as conflicting with a draft shape"
            );
        }
    }

    #[test]
    fn mixed_shapes_are_inconsistent_and_do_not_infer() {
        let s = signals(&[
            Signal::OccursOnRequirementFacet,
            Signal::CardinalityOnRequirementFacet,
        ]);
        assert!(s.is_internally_inconsistent());
        assert_eq!(
            s.infer(),
            None,
            "a self-contradictory document must not infer"
        );
    }

    /// Each rename from the issue, in both directions.
    #[test]
    fn each_rename_is_detected() {
        let cases = [
            (Signal::PropertyName, Signal::PropertyBaseName),
            (Signal::MeasureAttribute, Signal::DataTypeAttribute),
            (Signal::Ifc4x3Unsuffixed, Signal::Ifc4x3Add2),
        ];
        for (old, new) in cases {
            assert_eq!(
                signals(&[old]).infer(),
                Some(IdsVersion::Draft0_9),
                "{old:?} must indicate a draft"
            );
            assert_eq!(
                signals(&[new]).infer(),
                Some(IdsVersion::Ids1_0),
                "{new:?} must indicate 1.0"
            );
        }
    }

    /// `<system>` became required in 1.0, so only its absence is evidence.
    #[test]
    fn missing_classification_system_indicates_a_draft() {
        assert_eq!(
            signals(&[Signal::ClassificationWithoutSystem]).infer(),
            Some(IdsVersion::Draft0_9)
        );
    }

    #[test]
    fn observing_is_idempotent_and_queryable() {
        let mut s = VersionSignals::new();
        assert!(s.is_empty());
        s.observe(Signal::PropertyBaseName)
            .observe(Signal::PropertyBaseName);
        assert!(!s.is_empty());
        assert!(s.saw(Signal::PropertyBaseName));
        assert!(!s.saw(Signal::PropertyName));
        assert_eq!(s.iter().count(), 1);
    }
}
