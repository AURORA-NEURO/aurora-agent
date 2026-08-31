//! The `world sweep` command over the structural family grid (43.39).
//!
//! One bounded grid — the default axes with the distractor axis pinned to 50 — is enough to pin
//! the command's contract: the JSON carries one row per strategy per cell with refusals kept
//! distinct from unsound verdicts, admissibility is the only ranking axis, and the 43.41 stop
//! rule maps FIBER's admissibility to the exit code. The full 36-cell measurement lives in
//! `crates/baseline/tests/sweep_grid.rs` and `docs/FINDINGS.md` §6; this file tests the CLI
//! surface, not the finding.

use serde_json::Value;
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_bioprism");

fn run(arguments: &[&str]) -> Output {
    Command::new(BIN)
        .args(arguments)
        .output()
        .expect("cli binary runs")
}

#[test]
fn a_bounded_sweep_reports_admissibility_per_cell_and_fiber_admissible_everywhere_exits_zero() {
    let output = run(&["--json", "world", "sweep", "--distractors", "50"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "FIBER is admissible in every cell of this grid, so the stop rule must not fire: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: Value = serde_json::from_str(&String::from_utf8(output.stdout).unwrap())
        .expect("stdout is a single JSON document");

    assert_eq!(parsed["ok"], Value::Bool(true));
    assert_eq!(
        parsed["cells_total"],
        Value::from(12),
        "2 attachments x 3 relay depths x 2 tag styles x 1 distractor count"
    );
    assert_eq!(parsed["admissible_cells"]["fiber"], Value::from(12));
    assert_eq!(parsed["admissible_cells"]["full-context"], Value::from(12));
    assert_eq!(
        parsed["admissible_cells"]["graph-4-hop"],
        Value::from(0),
        "the shallow graph walk misses the relayed protected chain in every cell"
    );

    let cells = parsed["cells"].as_array().expect("cells list");
    assert_eq!(cells.len(), 12);
    for cell in cells {
        for row in cell["rows"].as_array().expect("rows list") {
            assert_eq!(
                row["judged"],
                Value::Bool(true),
                "no generated world in this grid produces an oracle refusal: {row}"
            );
            assert!(
                row["sound"].is_boolean(),
                "a judged row carries its verdict: {row}"
            );
        }
    }
}
