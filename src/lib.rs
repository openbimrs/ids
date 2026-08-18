//! `ids` — buildingSMART Information Delivery Specification.
//!
//! IDS is the standard, machine-readable way to state *"this model must contain
//! these things, with these properties"* and audit a model against it. It is
//! the highest-leverage openBIM standard for real projects, because it turns
//! contractual information requirements into an automated check.
//!
//! # Why this lives in `openbim/`, not `ifc/`
//!
//! IDS is a **consumer** of the IFC layer, not part of it. It reads a model and
//! reports findings; nothing in `packages/ifc/` may depend on it. Keeping the
//! direction of dependency one-way is what stops the IFC core from accreting
//! every standard that happens to use it.
//!
//! # Note on validation
//!
//! An IDS audit that quietly treats "property missing" as "check passed" is
//! worse than no audit. Facet results must distinguish *applicable and passed*,
//! *applicable and failed*, and *not applicable* — a lesson taken from the
//! sibling `../vendor/solibri` engine, whose rule layer makes the same
//! distinction explicit.
//!
//! # Status
//!
//! Reserved. The `references/ifclite` clone carries a buildingSMART IDS test
//! corpus (`packages/ids/src/__corpus__/`) usable as an oracle. See
//! `docs/ROADMAP.md` Stage 5.
