//! Eager versus indexed compile cost.
//!
//! Run with `cargo run --release --offline -p bioprism-store --example eager_vs_lazy`.
//!
//! The eager path parses the whole world on every query. The indexed path pays that once at build
//! time and then answers point queries. The question is whether query cost becomes independent of
//! corpus size, which is what blueprint 43.34 is for.

use bioprism_fiber::{compile, Query};
use bioprism_store::{build, LazyWorld};
use bioprism_world::World;
use bioprism_worldgen::{generate, WorldSpec};
use std::time::Instant;

fn main() {
    println!(
        "{:>10}  {:>12}  {:>12}  {:>12}  {:>12}  {:>10}",
        "facts", "eager_ms", "build_ms", "open_ms", "lazy_ms", "speedup"
    );

    for distractors in [1_000usize, 10_000, 100_000, 500_000, 1_000_000] {
        let spec = WorldSpec::discriminating(distractors);
        let generated = generate(&spec);
        let world_json = generated.world;
        let query = Query::from_json(generated.query).expect("valid query");

        let started = Instant::now();
        let eager = World::from_json(world_json.clone()).expect("valid world");
        let eager_out = compile(&eager, &query).expect("compiles");
        let eager_ms = started.elapsed().as_secs_f64() * 1000.0;
        let facts = eager.facts.len();
        drop(eager);

        let directory =
            std::env::temp_dir().join(format!("bioprism-bench-{distractors}"));
        let _ = std::fs::remove_dir_all(&directory);

        let started = Instant::now();
        build(&world_json, &directory).expect("store builds");
        let build_ms = started.elapsed().as_secs_f64() * 1000.0;
        drop(world_json);

        let started = Instant::now();
        let lazy = LazyWorld::open(&directory).expect("store opens");
        let open_ms = started.elapsed().as_secs_f64() * 1000.0;

        let started = Instant::now();
        let lazy_out = compile(&lazy, &query).expect("compiles");
        let lazy_ms = started.elapsed().as_secs_f64() * 1000.0;

        assert_eq!(
            eager_out
                .certificate
                .digest(bioprism_section::CertificateProfile::Reference)
                .unwrap(),
            lazy_out
                .certificate
                .digest(bioprism_section::CertificateProfile::Reference)
                .unwrap(),
            "backends diverged at {facts} facts"
        );

        println!(
            "{:>10}  {:>12.1}  {:>12.1}  {:>12.1}  {:>12.2}  {:>9.0}x",
            facts,
            eager_ms,
            build_ms,
            open_ms,
            lazy_ms,
            eager_ms / lazy_ms.max(0.001)
        );

        let _ = std::fs::remove_dir_all(&directory);
    }

    println!(
        "\neager_ms  = parse the whole world, then compile (what every query paid before)\n\
         build_ms  = one-time index construction per world release\n\
         open_ms   = load the offset tables\n\
         lazy_ms   = compile against the index"
    );
}
