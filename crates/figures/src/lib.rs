//! Deterministic SVG figures over the workspace's serialized artifacts.
//!
//! Six pure functions render six artifact shapes. Each takes the artifact as a parsed
//! `serde_json::Value` and returns one complete, standalone SVG document as a `String`. There is
//! no I/O, no clock, and no randomness anywhere in the crate: the same input value produces the
//! same output bytes, byte for byte, and the claim tests pin whole-figure sha256 digests.
//!
//! # What is rendered, and where it is specified
//!
//! The blueprint does not specify a figure renderer; the layouts, palette, and category encodings
//! here are this crate's own design, stated as such rather than attributed. What the figures
//! render are the blueprint's artifacts:
//!
//! * [`baseline_panel`] — the 43.38 matched comparison (`Comparison::to_json`), with 43.41's
//!   admissibility vocabulary: admissible, lucky, unsound, refused.
//! * [`selection_ratio`] and [`omission_accounting`] — the plan and omission blocks of the
//!   context certificate 43.26 makes mandatory.
//! * [`sweep_grid`] — the 43.39 structural family sweep, with ties drawn as prominently as wins,
//!   because negative findings are first-class results here.
//! * [`mutation_diversity`] — PRISM Gate 3's effective-diversity measurement, caveat verbatim.
//! * [`autopilot_drive`] — the autopilot report; the report document is `bioprism-autopilot`'s
//!   design (40.36 supplies only its retry-classification vocabulary) and the figure follows the
//!   document.
//!
//! # The source digest in every footer
//!
//! Every figure ends with a monospace footer `source sha256: <hex>`. The digest is **computed
//! inside the renderer, not taken as a parameter**: it is `bioprism_ids::ContentHash::of_value`
//! over the exact `Value` being rendered — the workspace's single canonicalisation, the same
//! function that stamps `certificate_sha256` and `report_sha256`. A caller-supplied digest was
//! rejected because it could disagree with the figure above it, and a figure that mislabels its
//! own source is worse than no figure. The honest consequence: the hex identifies the canonical
//! form of the artifact, so insignificant whitespace and object-key order in the caller's file do
//! not change it, and it will match the digest any other workspace component computes for the
//! same value.
//!
//! # Rendering rules the tests hold this crate to
//!
//! * A refused row is drawn as a refused *state*, never as a zero-length bar; absent keys on
//!   refused rows are semantic and a document that contradicts that (an unjudged row carrying
//!   oracle-derived keys) is refused with [`FigureError::Inconsistent`].
//! * A field a figure renders but cannot find is [`FigureError::MissingField`] naming the dotted
//!   path — nothing silently defaults to zero.
//! * Derived flags and totals are cross-checked against the fields they are defined from
//!   (`admissible` against verdict and closure, `inflation_ratio` against its counts,
//!   `attempts_used` against the attempts array and the declared budget).
//! * Nothing is drawn outside the frame that clips it: a caption too long for one line wraps
//!   rather than losing its tail, and the sweep grid — the one figure whose column count comes
//!   from its input — widens its frame to fit the columns it drew.
//! * Every text fragment from the input is XML-escaped; figures stay well-formed for hostile
//!   strings.
//! * Colors are fixed: ink `#1a1a1a` on a transparent background, accent `#d97036`, muted
//!   `#8a8a8a`, plus hatch patterns built from the same two hues.
//!
//! # Not implemented
//!
//! * **No raster output.** SVG text only; PNG encoding belongs to whatever displays the figure.
//! * **No interactive output.** No scripts, links, tooltips, or animation — a figure is evidence,
//!   not an application.
//! * **No wall-clock axes anywhere.** The autopilot figure's axis is labelled "attempt sequence
//!   (logical, clock-free)" because the kernel owns no clock; drawing durations would fabricate
//!   measurements no artifact contains.
//! * **No styling API.** Palette, fonts, and layout are fixed at compile time so the same
//!   artifact always looks the same; a figure is a rendering of the artifact, not a canvas. The
//!   two dimensions that do follow the input — figure height, and the sweep grid's width — are
//!   computed from what was drawn, never from a caller-supplied knob.
//! * **No I/O.** Callers parse the artifact and write the SVG; this crate touches no filesystem.
//! * **No text shaping.** Labels are truncated and wrapped by character count, not measured with
//!   font metrics; layout constants leave slack instead.

mod diversity;
mod drive;
mod error;
mod extract;
mod grid;
mod omission;
mod panel;
mod selection;
mod svg;

pub use diversity::mutation_diversity;
pub use drive::autopilot_drive;
pub use error::FigureError;
pub use grid::sweep_grid;
pub use omission::omission_accounting;
pub use panel::baseline_panel;
pub use selection::selection_ratio;
pub use svg::{ACCENT, INK, MUTED};
