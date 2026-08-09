//! The non-visual answer contract — blueprint 42.27.
//!
//! 42.27 asks for "table, outline, keyboard, screen-reader, text, and API equivalents for every
//! core graph task", and 42.01's acceptance gates require that "a nonvisual path has feature
//! parity for core operations". Written as prose that is a review checklist, and review
//! checklists are how accessibility is lost: the picture ships, the table is a follow-up, the
//! follow-up describes the picture.
//!
//! This module turns the requirement into a bound. The [`Witness`] trait is what a lens is
//! allowed to produce, and it has no method that returns anything drawable. Its output is a row
//! of typed [`Cell`]s under named columns, plus a sentence. A finding that can only be seen — a
//! highlighted subgraph, a colour, a position — cannot implement it, so a lens whose answer is a
//! picture does not satisfy `Lens`'s `type Witness: Witness` bound and does not compile.
//!
//! That is the whole trick, and it is worth stating what it does *not* claim. This is not an
//! accessibility conformance layer. There is no keyboard model, no focus order, no ARIA, no
//! screen-reader integration and no locale handling here, because there is no renderer in this
//! workspace to attach them to. What is enforced is the precondition all of those depend on:
//! that the answer exists as data before anything draws it.
//!
//! # Why `Cell` has no blank
//!
//! A table is only an equivalent of a picture if it carries the same information, and the
//! standard way a table loses information is the empty cell. [`Cell`] therefore has no `Empty`,
//! no `Null`, and no `Option` payload; an absent quantity is [`Cell::Absent`], which carries a
//! [`Missingness`] and can always say why. This is 42.13 enforced one level up: the missingness
//! distinction survives into the rendition, not just into the model.

use crate::missingness::{Measured, Missingness, Observation};
use serde::{Deserialize, Serialize};

/// One field of a witness row.
///
/// The absent arm carries its reason. There is deliberately no variant for "nothing here": a
/// renderer handed a `Cell` can always speak it, and a reader who cannot see the table gets the
/// same content as one who can.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cell", rename_all = "snake_case")]
pub enum Cell {
    Text {
        value: String,
    },
    Count {
        value: u64,
    },
    Identifier {
        value: String,
    },
    Quantity {
        value: f64,
        unit: String,
    },
    /// A quantity with no value, and the named reason. See [`crate::missingness`].
    Absent {
        missingness: Missingness,
    },
}

impl Cell {
    pub fn text(value: impl Into<String>) -> Self {
        Cell::Text {
            value: value.into(),
        }
    }

    pub fn id(value: impl Into<String>) -> Self {
        Cell::Identifier {
            value: value.into(),
        }
    }

    pub fn count(value: usize) -> Self {
        Cell::Count {
            value: value as u64,
        }
    }

    pub fn measured(m: &Measured) -> Self {
        Cell::Quantity {
            value: m.value(),
            unit: m.unit().to_string(),
        }
    }

    /// The one conversion that matters: an [`Observation`] becomes a cell without ever passing
    /// through a nullable intermediate.
    pub fn observation(observation: &Observation) -> Self {
        match observation {
            Observation::Present(m) => Cell::measured(m),
            Observation::Absent(missing) => Cell::Absent {
                missingness: missing.clone(),
            },
        }
    }

    /// What a reader hears. Never "blank" and never the empty string.
    pub fn spoken(&self) -> String {
        match self {
            Cell::Text { value } | Cell::Identifier { value } => value.clone(),
            Cell::Count { value } => value.to_string(),
            Cell::Quantity { value, unit } => format!("{value} {unit}"),
            Cell::Absent { missingness } => missingness.sentence(),
        }
    }

    /// Whether this cell stands for a quantity nobody established.
    pub fn is_hole(&self) -> bool {
        match self {
            Cell::Absent { missingness } => !missingness.is_measured_result(),
            _ => false,
        }
    }
}

/// What a lens is permitted to produce as a finding.
///
/// A witness is "a concrete checkable object — `alias ALT-77 spans train and test` — never a
/// score" (`AGENTS.md`, and 43.41 for the leakage oracle specifically). This trait adds the
/// 42.27 obligation on top: the object must be *readable* without vision, as data.
///
/// Object safety is deliberate. The catalogue erases lens types, and an erased lens must still
/// be answerable, so none of these methods may be generic or return `Self`.
pub trait Witness {
    /// A stable machine label for this finding shape, used for grouping and for the release gate.
    fn kind(&self) -> &'static str;

    /// Column headers for [`Witness::cells`]. Constant per witness variant, so a reader can be
    /// told the shape of a row before hearing it.
    fn columns(&self) -> &'static [&'static str];

    /// The row. Must have exactly `columns().len()` entries; a lens that violates this is
    /// rejected by [`crate::run`] with [`crate::LensError::MalformedWitness`] rather than
    /// producing a ragged table.
    fn cells(&self) -> Vec<Cell>;

    /// One sentence stating the finding, for a reader who wants the claim before the table.
    fn sentence(&self) -> String;
}

/// A witness after type erasure: the form that reaches a report, an export, or a gate.
///
/// This is what a renderer would draw and what an agent would read, and they are the same bytes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WitnessRow {
    pub kind: String,
    pub columns: Vec<String>,
    pub cells: Vec<Cell>,
    pub sentence: String,
}

impl WitnessRow {
    pub(crate) fn erase<W: Witness + ?Sized>(witness: &W) -> Result<Self, (usize, usize)> {
        let columns = witness.columns();
        let cells = witness.cells();
        if columns.len() != cells.len() {
            return Err((columns.len(), cells.len()));
        }
        Ok(WitnessRow {
            kind: witness.kind().to_string(),
            columns: columns.iter().map(|c| (*c).to_string()).collect(),
            cells,
            sentence: witness.sentence(),
        })
    }

    /// The row as spoken text: `column: value` pairs, in column order.
    pub fn spoken(&self) -> Vec<String> {
        self.columns
            .iter()
            .zip(&self.cells)
            .map(|(column, cell)| format!("{column}: {}", cell.spoken()))
            .collect()
    }

    /// Whether any field of this row stands for something nobody measured.
    pub fn contains_hole(&self) -> bool {
        self.cells.iter().any(Cell::is_hole)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::missingness::{Missingness, UnattemptedReason};

    struct GoodWitness;

    impl Witness for GoodWitness {
        fn kind(&self) -> &'static str {
            "good"
        }
        fn columns(&self) -> &'static [&'static str] {
            &["subject", "split"]
        }
        fn cells(&self) -> Vec<Cell> {
            vec![Cell::id("S001"), Cell::text("train")]
        }
        fn sentence(&self) -> String {
            "subject S001 is in train".into()
        }
    }

    struct RaggedWitness;

    impl Witness for RaggedWitness {
        fn kind(&self) -> &'static str {
            "ragged"
        }
        fn columns(&self) -> &'static [&'static str] {
            &["subject", "split"]
        }
        fn cells(&self) -> Vec<Cell> {
            vec![Cell::id("S001")]
        }
        fn sentence(&self) -> String {
            "incomplete".into()
        }
    }

    #[test]
    fn a_witness_row_is_readable_as_column_value_pairs_without_a_rendering() {
        let row = WitnessRow::erase(&GoodWitness).unwrap();
        assert_eq!(row.spoken(), vec!["subject: S001", "split: train"]);
    }

    #[test]
    fn a_witness_whose_cells_do_not_match_its_columns_cannot_be_erased() {
        assert_eq!(WitnessRow::erase(&RaggedWitness), Err((2, 1)));
    }

    #[test]
    fn no_cell_speaks_as_the_empty_string() {
        let cells = vec![
            Cell::text("x"),
            Cell::id("S001"),
            Cell::count(0),
            Cell::Quantity {
                value: 0.0,
                unit: "ng/mL".into(),
            },
            Cell::Absent {
                missingness: Missingness::NeverMeasured {
                    reason: UnattemptedReason::NotOrdered,
                },
            },
        ];
        for cell in &cells {
            assert!(!cell.spoken().is_empty(), "{cell:?} spoke as empty");
        }
    }

    #[test]
    fn an_unmeasured_cell_and_a_measured_absent_cell_do_not_speak_alike() {
        let hole = Cell::Absent {
            missingness: Missingness::NeverMeasured {
                reason: UnattemptedReason::NotOrdered,
            },
        };
        let absence = Cell::Absent {
            missingness: Missingness::BiologicalAbsence {
                assay: "IHC".into(),
            },
        };
        assert_ne!(hole.spoken(), absence.spoken());
        assert!(hole.is_hole());
        assert!(!absence.is_hole());
    }

    #[test]
    fn a_zero_quantity_is_not_a_hole() {
        let zero = Cell::Quantity {
            value: 0.0,
            unit: "ng/mL".into(),
        };
        assert!(!zero.is_hole());
        assert_eq!(zero.spoken(), "0 ng/mL");
    }
}
