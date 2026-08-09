//! Compile-time scaling of the FIBER kernel.
//!
//! Run with `cargo run --release --offline -p bioprism-worldgen --example scaling`.
//!
//! The question this answers is whether the compiler kernel is anywhere near being the bottleneck
//! at realistic world sizes, which decides whether the core needs a faster language or a faster
//! algorithm — or neither.

use bioprism_fiber::{compile, Query};
use bioprism_world::World;
use bioprism_worldgen::{generate, WorldSpec};
use std::time::Instant;

fn main() {
    println!(
        "{:>10}  {:>10}  {:>10}  {:>12}  {:>12}  {:>12}  {:>10}",
        "distractors", "facts", "factors", "parse_ms", "compile_ms", "hash_ms", "selected"
    );

    for distractors in [1_000usize, 10_000, 100_000, 500_000, 1_000_000] {
        let spec = WorldSpec::discriminating(distractors);
        let generated = generate(&spec);
        let query_json = generated.query;
        let world_json = generated.world;

        let started = Instant::now();
        let world = World::from_json(world_json).expect("valid world");
        let parse_ms = started.elapsed().as_secs_f64() * 1000.0;

        let query = Query::from_json(query_json).expect("valid query");

        let started = Instant::now();
        let out = compile(&world, &query).expect("compiles");
        let compile_ms = started.elapsed().as_secs_f64() * 1000.0;

        let started = Instant::now();
        let digest = out
            .certificate
            .digest(bioprism_section::CertificateProfile::Reference)
            .expect("digest");
        let hash_ms = started.elapsed().as_secs_f64() * 1000.0;
        std::hint::black_box(&digest);

        println!(
            "{:>10}  {:>10}  {:>10}  {:>12.1}  {:>12.1}  {:>12.1}  {:>10}",
            distractors,
            world.facts.len(),
            world.factors.len(),
            parse_ms,
            compile_ms,
            hash_ms,
            out.certificate.selected_facts.len()
        );
    }
}
