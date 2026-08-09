//! Capability coverage — and, more usefully, the gaps.
//!
//! 15.00 asks for a coverage matrix whose rows are capability nodes and whose columns are
//! domains, tracking oracle strength and maintainer per cell. The matrix is easy; the reason to
//! build it is the complement. A portfolio of forty-six packs looks comprehensive until you ask
//! which capability families rest on a single pack, and which rest only on packs that nothing can
//! re-run to check.
//!
//! Four questions are answered here, in increasing order of usefulness:
//!
//! 1. **Uncovered** — no pack claims the family at all. The obvious gap, and usually the empty
//!    list, because a portfolio is normally written by enumerating the families.
//! 2. **Singly covered** — exactly one pack claims it. A single point of failure: if that pack
//!    saturates or is found contaminated, the family goes from measured to unmeasured with no
//!    change in the headline pack count.
//! 3. **Weakly covered** — every pack claiming it tops out at a nondeterministic oracle. The
//!    family is covered on the matrix and not in fact, because no disagreement about an instance
//!    can be settled by re-running anything.
//! 4. **Effectively uncovered** — every pack claiming it is unreportable under
//!    [`crate::health`]. This is why [`coverage`] takes a pack list rather than reading the
//!    portfolio directly: pass it the healthy subset and the gap list tells you what the *current
//!    release* cannot measure, which is a different and more actionable set than what the
//!    portfolio was designed to measure.
//!
//! Not modelled: maintainer and refresh state, which 15.00 also wants per cell. That is registry
//! and repository metadata, not a property of the pack definition, and inventing a field for it
//! here would produce a column that is always empty.

use crate::portfolio;
use crate::taxonomy::{CapabilityFamily, Domain, OracleTier};
use serde::{Deserialize, Serialize};

/// One capability family and the packs that claim it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoverageRow {
    pub family: CapabilityFamily,
    /// `B4`, `A07`. See [`CapabilityFamily::code_is_from_blueprint`] for which vocabulary owns it.
    pub code: String,
    pub packs: Vec<String>,
    /// The strongest oracle available across all packs claiming this family.
    pub strongest_oracle: Option<OracleTier>,
    /// Whether any pack claiming this family can be settled by re-running.
    pub grounded: bool,
}

impl CoverageRow {
    pub fn pack_count(&self) -> usize {
        self.packs.len()
    }
}

/// One cell of the 15.00 coverage matrix.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatrixCell {
    pub family: CapabilityFamily,
    pub domain: Domain,
    pub packs: Vec<String>,
}

/// Coverage and its complement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoverageReport {
    pub rows: Vec<CoverageRow>,
    /// Families no pack in the supplied set claims.
    pub uncovered: Vec<CapabilityFamily>,
    /// Families exactly one pack claims.
    pub singly_covered: Vec<CapabilityFamily>,
    /// Families claimed only by packs with no execution-grounded oracle.
    pub weakly_covered: Vec<CapabilityFamily>,
}

impl CoverageReport {
    pub fn row(&self, family: CapabilityFamily) -> Option<&CoverageRow> {
        self.rows.iter().find(|r| r.family == family)
    }

    pub fn is_covered(&self, family: CapabilityFamily) -> bool {
        self.row(family).is_some_and(|r| !r.packs.is_empty())
    }

    /// The paragraph a portfolio review should open with.
    ///
    /// Leads with the gaps rather than the count, because the count is the number that is always
    /// available and never diagnostic.
    pub fn gap_summary(&self) -> String {
        let codes = |families: &[CapabilityFamily]| -> String {
            if families.is_empty() {
                "none".to_string()
            } else {
                families
                    .iter()
                    .map(|f| f.code())
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        };
        format!(
            "{} of {} capability families have at least one pack. Uncovered: {}. Covered by a \
             single pack (a pack retirement removes the family): {}. Covered only by packs whose \
             best oracle is nondeterministic: {}.",
            self.rows.iter().filter(|r| !r.packs.is_empty()).count(),
            self.rows.len(),
            codes(&self.uncovered),
            codes(&self.singly_covered),
            codes(&self.weakly_covered),
        )
    }
}

/// Coverage of every capability family by the supplied packs.
///
/// Takes a slice rather than reading the portfolio so a caller can ask the question about a
/// subset — the healthy packs, the packs in release wave 1, the packs with a maintainer.
pub fn coverage(packs: &[&portfolio::PackDefinition]) -> CoverageReport {
    let mut rows = Vec::new();
    for family in CapabilityFamily::all() {
        let claiming: Vec<&portfolio::PackDefinition> =
            packs.iter().copied().filter(|p| p.covers(family)).collect();
        let strongest = claiming
            .iter()
            .filter_map(|p| p.strongest_oracle())
            .max_by_key(|t| t.strength());
        rows.push(CoverageRow {
            family,
            code: family.code().to_string(),
            packs: claiming.iter().map(|p| p.id.to_string()).collect(),
            strongest_oracle: strongest,
            grounded: claiming.iter().any(|p| p.has_grounded_oracle()),
        });
    }

    let uncovered = rows
        .iter()
        .filter(|r| r.packs.is_empty())
        .map(|r| r.family)
        .collect();
    let singly_covered = rows
        .iter()
        .filter(|r| r.packs.len() == 1)
        .map(|r| r.family)
        .collect();
    let weakly_covered = rows
        .iter()
        .filter(|r| !r.packs.is_empty() && !r.grounded)
        .map(|r| r.family)
        .collect();

    CoverageReport {
        rows,
        uncovered,
        singly_covered,
        weakly_covered,
    }
}

/// Coverage of the whole portfolio.
pub fn portfolio_coverage() -> CoverageReport {
    let packs: Vec<&portfolio::PackDefinition> = portfolio::all().iter().collect();
    coverage(&packs)
}

/// The (capability family x domain) matrix of 15.00, with empty cells omitted.
///
/// Empty cells are omitted rather than emitted as zeros because the product of fourteen agent
/// families, thirteen biological families and twelve domains is mostly meaningless — there is no
/// sense in which "B2 assay understanding x browser" is a gap.
pub fn matrix(packs: &[&portfolio::PackDefinition]) -> Vec<MatrixCell> {
    let mut cells = Vec::new();
    for family in CapabilityFamily::all() {
        for domain in Domain::ALL {
            let ids: Vec<String> = packs
                .iter()
                .filter(|p| p.covers(family) && p.domains.contains(domain))
                .map(|p| p.id.to_string())
                .collect();
            if !ids.is_empty() {
                cells.push(MatrixCell {
                    family,
                    domain: *domain,
                    packs: ids,
                });
            }
        }
    }
    cells
}
