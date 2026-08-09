//! What the eighty-four add up to, and the shape of the argument they make.
//!
//! These tests are the crate's findings written as assertions. Each one states a claim about the
//! residue that would be wrong if somebody quietly reclassified a module — which is the specific
//! way a register like this goes bad, because every reclassification into a never-moves bucket
//! makes the remaining work look smaller.

use std::collections::BTreeSet;

use bioprism_residue::{
    residue, Classification, Distribution, ModuleKey, Report, Standing, UncoveredStanding,
};

#[test]
fn the_register_explains_the_whole_backlog_and_nothing_else() {
    let register = residue().expect("well formed");
    assert_eq!(register.len(), 84);
    assert_eq!(register.sections().len(), 13);
}

#[test]
fn every_module_carries_at_least_one_verdict_and_every_verdict_carries_a_source() {
    let register = residue().expect("well formed");
    for entry in register.entries() {
        assert!(!entry.verdicts().is_empty(), "{}", entry.title());
        for verdict in entry.verdicts() {
            assert!(!verdict.source().reasoning().is_empty());
            assert!(!verdict.source().locus().is_empty());
            assert!(!verdict.source().anchor().needle.is_empty());
        }
    }
}

#[test]
fn twenty_six_of_the_eighty_four_still_carry_work_on_at_least_one_reading() {
    let register = residue().expect("well formed");
    let distribution = Distribution::of(&register);
    assert_eq!(distribution.work_remaining, 26);
    assert_eq!(distribution.modules.total(), 84);
    // Twenty-five by primary verdict. The twenty-sixth is a module whose section's own crate says
    // the content was handed to a sibling and whose sibling says half of it never landed, and a
    // reader planning work needs to see the second reading.
    assert_eq!(distribution.modules.genuinely_uncovered, 25);
}

#[test]
fn the_verdict_distribution_over_modules_is_the_one_reported() {
    let register = residue().expect("well formed");
    let counts = Distribution::of(&register).modules;
    assert_eq!(counts.process, 37);
    assert_eq!(counts.foreign_artifact, 10);
    assert_eq!(counts.discharged_elsewhere, 12);
    assert_eq!(counts.genuinely_uncovered, 25);
    assert_eq!(
        counts.block_level_split, 0,
        "no module's primary verdict is a split"
    );
    assert_eq!(counts.total(), 84);
}

#[test]
fn counting_recorded_judgements_rather_than_modules_gives_a_different_and_larger_answer() {
    // Twelve modules carry a second verdict, and the difference is the whole reason the report
    // prints both columns: "how many modules are process" and "how many process verdicts were
    // recorded" are different questions.
    let register = residue().expect("well formed");
    let distribution = Distribution::of(&register);
    assert!(distribution.verdicts.total() > distribution.modules.total());
    assert_eq!(distribution.verdicts.block_level_split, 1);
}

#[test]
fn most_of_the_register_is_transcription_rather_than_this_crates_own_reading() {
    let register = residue().expect("well formed");
    let distribution = Distribution::of(&register);
    assert!(
        distribution.transcribed > distribution.inferred_here * 2,
        "transcribed {} vs inferred {}",
        distribution.transcribed,
        distribution.inferred_here
    );
}

#[test]
fn eleven_modules_are_contested_and_none_of_them_is_adjudicated_here() {
    let register = residue().expect("well formed");
    let contested = register.contested();
    assert_eq!(contested.len(), 11);
    for entry in contested {
        let positions = entry.contest().expect("contested");
        let verdicts: BTreeSet<&str> = positions.iter().map(|(_, kind)| *kind).collect();
        assert!(verdicts.len() >= 2, "{}", entry.title());
        let crates: BTreeSet<&String> = positions.iter().map(|(name, _)| name).collect();
        assert!(crates.len() >= 2, "{}", entry.title());
    }
}

#[test]
fn ten_of_the_eleven_contests_are_one_argument_about_one_section() {
    // `crates/atlasx` reports that the capability-metrics remainder defines nothing;
    // `bioprism-metrics` reports that it already implements the arithmetic governing all of it.
    // One section, ten modules, two readings, neither of them adjudicated.
    let register = residue().expect("well formed");
    let contested_sections: Vec<u8> = register
        .contested()
        .into_iter()
        .map(|entry| entry.key().section())
        .collect();
    assert_eq!(
        contested_sections
            .iter()
            .filter(|section| **section == 33)
            .count(),
        10
    );
    assert_eq!(register.section(33).len(), 10);
}

#[test]
fn the_one_remaining_contest_is_about_who_actually_landed_the_scheduling_work() {
    let register = residue().expect("well formed");
    let entry = register
        .get(ModuleKey::new(35, 13).expect("in range"))
        .expect("registered");
    assert!(entry.is_contested());
    let positions = entry.contest().expect("contested");
    assert!(positions
        .iter()
        .any(|(name, kind)| name == "bioprism-scale" && *kind == "discharged elsewhere"));
    assert!(positions
        .iter()
        .any(|(name, kind)| name == "bioprism-factory" && *kind == "genuinely uncovered"));
}

#[test]
fn a_compound_entry_is_one_crate_holding_two_readings_and_is_not_a_contest() {
    let register = residue().expect("well formed");
    let compound = register.compound();
    assert!(!compound.is_empty());
    for entry in &compound {
        for verdict in entry.verdicts() {
            assert_eq!(
                entry.primary().recorded_by().as_str(),
                verdict.recorded_by().as_str(),
                "a compound entry's verdicts all come from one crate: {}",
                entry.title()
            );
        }
        assert!(!entry.is_contested(), "{}", entry.title());
    }
}

#[test]
fn the_block_level_split_is_recorded_where_the_crate_that_named_it_found_it() {
    let register = residue().expect("well formed");
    let entry = register
        .get(ModuleKey::new(26, 19).expect("in range"))
        .expect("registered");
    assert!(entry.is_compound());
    let split = entry
        .verdicts()
        .iter()
        .find(|verdict| {
            matches!(
                verdict.classification(),
                Classification::BlockLevelSplit { .. }
            )
        })
        .expect("the split is recorded");
    assert_eq!(split.recorded_by().as_str(), "bioprism-bioevalx");
}

#[test]
fn nineteen_modules_have_no_recorded_judgement_at_all() {
    let register = residue().expect("well formed");
    let unread: Vec<_> = register
        .entries()
        .iter()
        .filter(|entry| {
            entry.verdicts().iter().any(|verdict| {
                matches!(
                    verdict.classification(),
                    Classification::GenuinelyUncovered {
                        standing: UncoveredStanding::NobodyHasRead { .. }
                    }
                )
            })
        })
        .collect();
    assert_eq!(unread.len(), 19);
    // The other seven uncovered modules are uncovered for a *stated* reason — no sandbox, no
    // storage backend, no executions to mine — which is a different fact from nobody having looked.
    let distribution = Distribution::of(&register);
    assert_eq!(distribution.verdicts.genuinely_uncovered - unread.len(), 7);
}

#[test]
fn every_unread_module_names_the_crates_that_were_searched() {
    let register = residue().expect("well formed");
    for entry in register.entries() {
        for verdict in entry.verdicts() {
            if let Classification::GenuinelyUncovered {
                standing: UncoveredStanding::NobodyHasRead { surveyed },
            } = verdict.classification()
            {
                assert!(
                    surveyed.crates().len() >= 2,
                    "a survey of fewer than two crates is barely a search: {}",
                    entry.title()
                );
            }
        }
    }
}

#[test]
fn the_unread_modules_are_exactly_the_sections_whose_crates_are_being_written_right_now() {
    // §23, §12 and §36 have `crates/interweave`, `crates/dataops` and `crates/bioethics` in flight.
    // Those entries are expected to be deleted, not rewritten, which is what makes the register
    // maintainable while four siblings are moving underneath it.
    let register = residue().expect("well formed");
    let sections: BTreeSet<u8> = register
        .entries()
        .iter()
        .filter(|entry| {
            entry.verdicts().iter().any(|verdict| {
                matches!(
                    verdict.classification(),
                    Classification::GenuinelyUncovered {
                        standing: UncoveredStanding::NobodyHasRead { .. }
                    }
                )
            })
        })
        .map(|entry| entry.key().section())
        .collect();
    assert_eq!(sections, BTreeSet::from([12, 23, 36]));
}

#[test]
fn every_module_read_across_from_a_neighbouring_section_is_marked_as_this_registers_reading() {
    let register = residue().expect("well formed");
    let inferred_only = register.only_inferred();
    for entry in &inferred_only {
        assert!(
            entry
                .verdicts()
                .iter()
                .all(|verdict| verdict.standing() == Standing::InferredHere),
            "{}",
            entry.title()
        );
    }
    let million_scale = inferred_only
        .iter()
        .filter(|entry| entry.key().section() == 35)
        .count();
    assert_eq!(million_scale, 6);
}

#[test]
fn a_foreign_artifact_verdict_never_says_the_work_is_done_here() {
    let register = residue().expect("well formed");
    let foreign: Vec<_> = register
        .entries()
        .iter()
        .filter(|entry| {
            matches!(
                entry.primary().classification(),
                Classification::ForeignArtifact { .. }
            )
        })
        .collect();
    assert_eq!(foreign.len(), 10);
    for entry in foreign {
        assert!(!entry.primary().classification().is_work_remaining());
    }
}

#[test]
fn every_discharge_names_at_least_one_crate_that_holds_the_substance() {
    let register = residue().expect("well formed");
    let mut discharges = 0;
    for entry in register.entries() {
        for verdict in entry.verdicts() {
            if let Classification::DischargedElsewhere { by } = verdict.classification() {
                assert!(!by.crates().is_empty(), "{}", entry.title());
                discharges += 1;
            }
        }
    }
    assert!(discharges >= 12);
}

#[test]
fn a_crate_naming_itself_as_the_discharger_is_the_finding_a_token_scan_cannot_see() {
    // The sharpest thing in the register: a crate that implemented a module's content under a
    // *different* section's id, so `tools/coverage.sh` reports the module uncovered while the
    // capability exists. Fourteen verdicts, in three sections, and every one of them is a module a
    // contributor might otherwise pick up and build a second time.
    let register = residue().expect("well formed");
    let mut sections = BTreeSet::new();
    let mut count = 0;
    for entry in register.entries() {
        for verdict in entry.verdicts() {
            if let Classification::DischargedElsewhere { by } = verdict.classification() {
                if by
                    .crates()
                    .iter()
                    .any(|name| name.as_str() == verdict.recorded_by().as_str())
                {
                    sections.insert(entry.key().section());
                    count += 1;
                }
            }
        }
    }
    assert_eq!(count, 14);
    assert_eq!(sections, BTreeSet::from([33, 34, 35]));
}

#[test]
fn the_residue_concentrates_in_four_sections_and_none_of_them_holds_a_majority() {
    let register = residue().expect("well formed");
    let report = Report::of(&register);
    let largest = report
        .by_section
        .values()
        .max()
        .copied()
        .unwrap_or_default();
    assert_eq!(largest, 13);
    assert!(largest * 2 < register.len());
}

#[test]
fn a_module_leaving_the_backlog_is_a_deletion_and_touches_nothing_else() {
    let mut register = residue().expect("well formed");
    let key = ModuleKey::new(11, 10).expect("in range");
    assert!(register.get(key).is_some());
    assert!(register.without(key));
    assert_eq!(register.len(), 83);
    assert!(register.get(key).is_none());
    assert!(!register.without(key), "removing it twice is a no-op");
    // Nothing else moved: the remaining entries hold no cross-references to each other.
    assert!(register.find("Mcp Server").is_none());
    assert!(register.find("Python Sdk").is_some());
}

#[test]
fn a_module_can_be_found_by_the_title_the_backlog_gives_it_without_anyone_writing_an_id() {
    let register = residue().expect("well formed");
    assert!(register.find("registry overview").is_some());
    assert!(register.find("  Opentelemetry Adapter  ").is_some());
    assert!(register.find("a module that does not exist").is_none());
}

#[test]
fn the_rendered_report_is_a_function_of_the_register_alone() {
    let register = residue().expect("well formed");
    let once = Report::of(&register).render();
    let twice = Report::of(&register).render();
    assert_eq!(once, twice);
    assert!(once.contains("genuinely uncovered"));
    assert!(once.contains("no crate has taken any position"));
}

#[test]
fn a_register_survives_a_round_trip_through_its_wire_form_with_every_gate_re_run() {
    let register = residue().expect("well formed");
    let json = serde_json::to_string(&register).expect("serializes");
    let restored: bioprism_residue::Register = serde_json::from_str(&json).expect("deserializes");
    assert_eq!(restored, register);
}

#[test]
fn a_stored_verdict_that_lost_its_discharger_fails_to_parse_rather_than_arriving_weaker() {
    let register = residue().expect("well formed");
    let json = serde_json::to_string(&register).expect("serializes");
    let hollowed = json.replace(
        "\"verdict\":\"discharged_elsewhere\",\"by\":[\"bioprism-mcp\"]",
        "\"verdict\":\"discharged_elsewhere\",\"by\":[]",
    );
    assert_ne!(hollowed, json, "the fixture matched something");
    assert!(serde_json::from_str::<bioprism_residue::Register>(&hollowed).is_err());
}

#[test]
fn a_stored_verdict_that_lost_its_reasoning_fails_to_parse() {
    let register = residue().expect("well formed");
    let json = serde_json::to_string(&register).expect("serializes");
    let start = json.find("\"reasoning\":\"").expect("a reasoning field");
    let rest = &json[start + "\"reasoning\":\"".len()..];
    let end = rest.find("\",\"").expect("its end");
    let hollowed = format!("{}\"reasoning\":\"{}", &json[..start], &rest[end..]);
    assert!(serde_json::from_str::<bioprism_residue::Register>(&hollowed).is_err());
}
