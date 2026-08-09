//! Runs every registered vertical slice and prints the result.
//!
//! This is the artifact a newcomer runs first: `cargo run -p bioprism-examples --example
//! run_slices`. It prints one line per slice with its digest, every expectation that stopped
//! holding, and then the coverage table — including the claims no slice exercises and what blocks
//! each of them.
//!
//! Pass `--json` for the full [`bioprism_examples::RegistryReport`] as canonical JSON, which is
//! the form a CI job should archive: it carries the digest that makes two runs comparable.

use bioprism_examples::{ExampleError, SliceRegistry};

fn main() -> Result<(), ExampleError> {
    let json = std::env::args().any(|arg| arg == "--json");
    let registry = SliceRegistry::standard();
    let report = registry.run_all()?;

    if json {
        let body = serde_json::to_value(&report).expect("registry report is serialisable");
        println!(
            "{}",
            bioprism_ids::to_canonical_string(&body).expect("report is finite JSON")
        );
    } else {
        print!("{}", report.render());
    }

    if !report.holds() {
        std::process::exit(1);
    }
    Ok(())
}
