---
title: "AURORA FIBER Reference Runtime"
status: "Executable Reference Prototype"
owner: "AURORA BioPRISM Working Group"
last_updated: "2026-08-07"
product: "AURORA FIBER"
module_id: "REF.FIBER.01"
graph_cluster: "FIBER"
token_profile: "L2"
---

# AURORA FIBER Reference Runtime

This standard-library-only prototype demonstrates the smallest executable FIBER loop:

```text
typed world + typed query
→ protected closure
→ backward factor slicing
→ temporal accessibility cut
→ deterministic split-integrity oracle
→ Decision Section
→ Context Certificate
→ graph/hypergraph/full-context comparison
```

Run:

```bash
python fiber_compile.py   --world examples/radiogenomic_world.json   --query examples/leakage_query.json   --output /tmp/context_certificate.json   --section-output /tmp/decision_section.json

python compare_baselines.py
python -m unittest discover -s tests -v
```

The synthetic world contains 750 exploratory factors sharing a `cohort_id` hub. They are connected in an undirected incidence/hypergraph projection but have no backward dependency path to the split-integrity target. FIBER includes the cohort fact because the identity oracle needs it, yet it does not traverse forward from that fact into unrelated exploratory factors.

## What the prototype proves

- deterministic protected closure and dependency slicing can be implemented without an LLM;
- a context certificate can enumerate selected and omitted material;
- a connected-structure baseline can expand through a high-degree hub;
- exact leakage witnesses can be retained while irrelevant facts are excluded;
- output is content hashed and testable.

## What it does not prove

- universal superiority to graph or hypergraph systems;
- formal decision sufficiency for arbitrary worlds;
- production-ready sheaf gluing or cohomology;
- FAQ-width, tensor, or decision-diagram optimization;
- clinical validity;
- security of a distributed/federated runtime.

Those are staged modules and research questions, not silently implied capabilities.
