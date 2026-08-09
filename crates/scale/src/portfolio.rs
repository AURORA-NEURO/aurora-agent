//! Parent BioWorld portfolio design.
//!
//! Blueprint 35.01: plan a balanced set of worlds across biological scales, modalities and decision
//! families. This is the top of the section because it is the only place where scientific breadth
//! is decided — everything downstream multiplies whatever breadth this produces, and multiplication
//! by mutation families does not create coverage that was never authored.
//!
//! ## Blind spots are a field, not a footnote
//!
//! 35.01's required components end with "known blind spots", and [`PortfolioReport`] carries them
//! as a populated `Vec` computed from the plan itself: every cell with a target and no worlds. A
//! coverage percentage without the list of empty cells is the same species of dishonesty as an
//! instance count without an effective size — it reports what is there and stays quiet about what
//! is not, which `AGENTS.md` calls the product.
//!
//! ## Not implemented
//!
//! No oracle-feasibility model and no data-rights model. 35.01 lists both; feasibility is
//! `bioprism-oracle`'s question and rights are `bioprism-governance`'s. Authoring cost is a single
//! caller-supplied rate rather than a per-cell estimate.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One cell of the coverage matrix: a biological scale, a modality, a decision family.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Cell {
    pub scale: String,
    pub modality: String,
    pub decision_family: String,
}

impl Cell {
    pub fn new(
        scale: impl Into<String>,
        modality: impl Into<String>,
        decision_family: impl Into<String>,
    ) -> Self {
        Cell {
            scale: scale.into(),
            modality: modality.into(),
            decision_family: decision_family.into(),
        }
    }

    pub fn label(&self) -> String {
        format!("{}/{}/{}", self.scale, self.modality, self.decision_family)
    }
}

/// Targets and actuals per cell, plus the sites and populations the worlds draw on.
#[derive(Debug, Clone, Default)]
pub struct PortfolioPlan {
    targets: BTreeMap<Cell, usize>,
    authored: BTreeMap<Cell, usize>,
    sites: BTreeMap<Cell, Vec<String>>,
}

impl PortfolioPlan {
    pub fn new() -> Self {
        PortfolioPlan::default()
    }

    pub fn target(&mut self, cell: Cell, worlds: usize) -> &mut Self {
        self.targets.insert(cell, worlds);
        self
    }

    /// Records an authored parent world in `cell`, drawn from `site`.
    ///
    /// Sites are tracked per world rather than per cell because 35.01 asks for "independent site
    /// and population targets": ten worlds from one site are one site's worth of breadth, and a
    /// coverage matrix that counts worlds alone cannot see that.
    pub fn author(&mut self, cell: Cell, site: impl Into<String>) -> &mut Self {
        *self.authored.entry(cell.clone()).or_default() += 1;
        self.sites.entry(cell).or_default().push(site.into());
        self
    }

    pub fn report(&self, expert_minutes_per_world: f64) -> PortfolioReport {
        let mut cells = Vec::new();
        let mut blind_spots = Vec::new();
        let mut targeted = 0usize;
        let mut authored_total = 0usize;
        let mut distinct_sites: BTreeMap<String, usize> = BTreeMap::new();

        for (cell, target) in &self.targets {
            let authored = self.authored.get(cell).copied().unwrap_or(0);
            targeted += target;
            authored_total += authored;
            for site in self.sites.get(cell).into_iter().flatten() {
                *distinct_sites.entry(site.clone()).or_default() += 1;
            }
            if authored == 0 && *target > 0 {
                blind_spots.push(cell.clone());
            }
            cells.push(CellCoverage {
                cell: cell.clone(),
                target: *target,
                authored,
                sites: self
                    .sites
                    .get(cell)
                    .map(|sites| sites.len())
                    .unwrap_or(0),
            });
        }

        let site_concentration = if authored_total == 0 {
            0.0
        } else {
            distinct_sites.values().copied().max().unwrap_or(0) as f64 / authored_total as f64
        };

        PortfolioReport {
            cells,
            blind_spots,
            targeted_worlds: targeted,
            authored_worlds: authored_total,
            coverage: if targeted == 0 {
                0.0
            } else {
                (authored_total.min(targeted)) as f64 / targeted as f64
            },
            distinct_sites: distinct_sites.len(),
            site_concentration,
            expert_minutes: authored_total as f64 * expert_minutes_per_world,
        }
    }
}

/// One cell's state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellCoverage {
    pub cell: Cell,
    pub target: usize,
    pub authored: usize,
    pub sites: usize,
}

/// Portfolio coverage, with the empty cells named.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortfolioReport {
    pub cells: Vec<CellCoverage>,
    /// Targeted cells with no authored world. Never summarized away.
    pub blind_spots: Vec<Cell>,
    pub targeted_worlds: usize,
    pub authored_worlds: usize,
    pub coverage: f64,
    pub distinct_sites: usize,
    /// Share of authored worlds from the single most prolific site.
    pub site_concentration: f64,
    pub expert_minutes: f64,
}

impl PortfolioReport {
    /// The sentence a portfolio summary leads with.
    pub fn headline(&self) -> String {
        format!(
            "{} of {} targeted parent worlds authored ({:.0}% coverage) across {} sites, with {} \
             blind spot(s): {}",
            self.authored_worlds,
            self.targeted_worlds,
            self.coverage * 100.0,
            self.distinct_sites,
            self.blind_spots.len(),
            if self.blind_spots.is_empty() {
                "none".to_string()
            } else {
                self.blind_spots
                    .iter()
                    .map(Cell::label)
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        )
    }
}
