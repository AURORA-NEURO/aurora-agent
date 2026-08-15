//! What the residue adds up to, and the shape of the argument it makes.
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
    assert_eq!(register.len(), 53);
    assert_eq!(register.sections().len(), 9);
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
fn exactly_one_of_the_fifty_three_still_carries_work_on_any_reading() {
    // This fell from twenty-six to one when four crates landed, and the fall is the register doing
    // its job rather than an embarrassment to it: twenty-five of the twenty-six left the backlog
    // outright. The survivor is the sandbox, which no crate can build and every crate says so. If a
    // later pass drives this to zero, check that the module really left rather than that somebody
    // reclassified it into a bucket that never moves.
    let register = residue().expect("well formed");
    let distribution = Distribution::of(&register);
    assert_eq!(distribution.work_remaining, 1);
    assert_eq!(distribution.modules.total(), 53);
    // Zero by *primary* verdict: no module's first-listed reading is that work remains. The one
    // survivor is a second reading beside a discharge, which is exactly the case a register holding
    // one verdict per module would have lost.
    assert_eq!(distribution.modules.genuinely_uncovered, 0);
    assert_eq!(distribution.verdicts.genuinely_uncovered, 1);
}

#[test]
fn the_verdict_distribution_over_modules_is_the_one_reported() {
    let register = residue().expect("well formed");
    let counts = Distribution::of(&register).modules;
    assert_eq!(counts.process, 37);
    assert_eq!(counts.foreign_artifact, 7);
    assert_eq!(counts.discharged_elsewhere, 9);
    assert_eq!(counts.genuinely_uncovered, 0);
    assert_eq!(
        counts.block_level_split, 0,
        "no module's primary verdict is a split"
    );
    assert_eq!(counts.total(), 53);
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
fn every_verdict_but_one_is_now_a_crates_own_sentence_rather_than_this_registers_reading() {
    // Thirty of a hundred and eight verdicts used to be read across from a neighbouring section or
    // drawn from a not-implemented list, because four sections had no crate that had read them.
    // Four crates then did. This is a fact about the backlog and not about the design: the inferred
    // standing is still the right answer whenever a crate decides a module without naming it, and
    // `verdict_gate.rs` still exercises it.
    let register = residue().expect("well formed");
    let distribution = Distribution::of(&register);
    assert_eq!(distribution.inferred_here, 1);
    assert_eq!(
        distribution.transcribed + distribution.inferred_here,
        distribution.verdicts.total()
    );
}

#[test]
fn ten_modules_are_contested_and_none_of_them_is_adjudicated_here() {
    let register = residue().expect("well formed");
    let contested = register.contested();
    assert_eq!(contested.len(), 10);
    for entry in contested {
        let positions = entry.contest().expect("contested");
        let verdicts: BTreeSet<&str> = positions.iter().map(|(_, kind)| *kind).collect();
        assert!(verdicts.len() >= 2, "{}", entry.title());
        let crates: BTreeSet<&String> = positions.iter().map(|(name, _)| name).collect();
        assert!(crates.len() >= 2, "{}", entry.title());
    }
}

#[test]
fn every_contest_left_in_the_register_is_one_argument_about_one_section() {
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
    // The eleventh contest — one crate saying the scheduling work had been handed on, the crate it
    // was handed to saying half of it never landed — left with its module.
    assert!(contested_sections.iter().all(|section| *section == 33));
}

#[test]
fn the_two_modules_a_newly_landed_crate_declined_are_transcribed_from_it() {
    // The regeneration case worth a test of its own: these two used to be this register's reading
    // of another crate's not-implemented list. `crates/bioethics` then read the modules and took a
    // position, so the source moved from inferred to transcribed without the entries moving.
    let register = residue().expect("well formed");
    for index in [7u8, 19] {
        let entry = register
            .get(ModuleKey::new(36, index).expect("in range"))
            .expect("registered");
        assert_eq!(entry.primary().recorded_by().as_str(), "bioprism-bioethics");
        assert_eq!(entry.primary().standing(), Standing::Transcribed);
        assert!(matches!(
            entry.primary().classification(),
            Classification::DischargedElsewhere { .. }
        ));
    }
}

#[test]
fn the_sandbox_carries_a_second_reading_and_the_red_team_programme_does_not() {
    // Both were declined on one ground, and the asymmetry is real rather than editorial. What is
    // left of the red-team module after the discharge is a clause the blueprint never defines, so
    // nobody could build it from the specification. What is left of the sandbox is a control that
    // would work if somebody built it and nobody has, so recording only the discharge would leave a
    // reader thinking a sandbox exists.
    let register = residue().expect("well formed");
    let sandbox = register
        .get(ModuleKey::new(36, 7).expect("in range"))
        .expect("registered");
    assert!(sandbox.is_compound());
    assert!(sandbox.has_work_remaining());
    assert!(!sandbox.is_contested(), "both readings are one crate's");
    assert_eq!(
        sandbox.verdicts()[1].standing(),
        Standing::InferredHere,
        "the crate classified it as positioned elsewhere and did not call it work remaining"
    );

    let red_team = register
        .get(ModuleKey::new(36, 19).expect("in range"))
        .expect("registered");
    assert_eq!(red_team.verdicts().len(), 1);
    assert!(!red_team.has_work_remaining());
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
fn no_module_is_left_without_a_recorded_judgement() {
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
    // Nineteen, once. Every one was in a section whose crate had not been written yet, and all
    // nineteen left the backlog when it was. Nothing was reclassified to get here.
    assert!(unread.is_empty());
    // The one uncovered verdict left is uncovered for a *stated* reason, which is a different fact
    // from nobody having looked and is why the two are separate variants.
    let distribution = Distribution::of(&register);
    assert_eq!(distribution.verdicts.genuinely_uncovered, 1);
}

#[test]
fn every_uncovered_verdict_states_either_a_survey_or_a_blocker() {
    // Neither variant is assertable by omission: a survey names the crates that were read, a
    // blocker names what stops the work. This is the honest default of the whole register and so
    // the one verdict an author could otherwise reach for without doing any work.
    let register = residue().expect("well formed");
    let mut seen = 0;
    for entry in register.entries() {
        for verdict in entry.verdicts() {
            if let Classification::GenuinelyUncovered { standing } = verdict.classification() {
                seen += 1;
                match standing {
                    UncoveredStanding::NobodyHasRead { surveyed } => assert!(
                        surveyed.crates().len() >= 2,
                        "a survey of fewer than two crates is barely a search: {}",
                        entry.title()
                    ),
                    UncoveredStanding::RealWorkNotDone { blocked_by } => assert!(
                        blocked_by.chars().count() >= 40,
                        "a blocker has to be a statement: {}",
                        entry.title()
                    ),
                }
            }
        }
    }
    assert_eq!(seen, 1);
}

#[test]
fn the_sections_that_had_nobody_have_left_the_register_rather_than_been_reclassified() {
    // §12, §23 and §35 held twenty-two of the register's twenty-six work-remaining modules and now
    // hold none, because `crates/dataops`, `crates/interweave` and `crates/megafactory` cited them.
    // The distinction this asserts is the one that matters: they are *absent*, not relabelled.
    let register = residue().expect("well formed");
    let sections: BTreeSet<u8> = register
        .entries()
        .iter()
        .map(|entry| entry.key().section())
        .collect();
    for departed in [12u8, 23, 35] {
        assert!(
            !sections.contains(&departed),
            "section {departed} still has entries"
        );
        assert!(register.section(departed).is_empty());
    }
}

#[test]
fn no_module_is_explained_only_by_this_registers_own_reading() {
    // Six modules were, until the crate owning their section was written. The one inferred verdict
    // left sits *beside* a transcribed one on the same module, so every entry in the register is
    // now anchored to a sentence a classifying crate wrote about that module.
    let register = residue().expect("well formed");
    assert!(register.only_inferred().is_empty());
    let inferred: Vec<_> = register
        .entries()
        .iter()
        .filter(|entry| {
            entry
                .verdicts()
                .iter()
                .any(|verdict| verdict.standing() == Standing::InferredHere)
        })
        .collect();
    assert_eq!(inferred.len(), 1);
    assert_eq!(
        inferred[0].primary().standing(),
        Standing::Transcribed,
        "the inferred reading is the second one, never the headline"
    );
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
    assert_eq!(foreign.len(), 7);
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
    // contributor might otherwise pick up and build a second time. It fell from fourteen to eleven
    // when the three §35 modules left, which is the only way this number should ever fall.
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
    assert_eq!(count, 11);
    assert_eq!(sections, BTreeSet::from([33, 34]));
}

#[test]
fn no_single_section_holds_a_majority_of_the_residue() {
    // The residue is spread rather than concentrated, and stayed spread through the regeneration:
    // three of the four sections that emptied were among the largest, and the largest remaining is
    // still under a quarter of the total. A register dominated by one section would be a report
    // about that section.
    let register = residue().expect("well formed");
    let report = Report::of(&register);
    let largest = report
        .by_section
        .values()
        .max()
        .copied()
        .unwrap_or_default();
    assert_eq!(largest, 10);
    assert!(largest * 4 < register.len());
}

#[test]
fn a_module_leaving_the_backlog_is_a_deletion_and_touches_nothing_else() {
    let mut register = residue().expect("well formed");
    let key = ModuleKey::new(11, 4).expect("in range");
    assert!(register.get(key).is_some());
    assert!(register.without(key));
    assert_eq!(register.len(), 52);
    assert!(register.get(key).is_none());
    assert!(!register.without(key), "removing it twice is a no-op");
    // Nothing else moved: the remaining entries hold no cross-references to each other.
    assert!(register.find("Python Sdk").is_none());
    assert!(register.find("Typescript Sdk").is_some());
}

#[test]
fn a_module_can_be_found_by_the_title_the_backlog_gives_it_without_anyone_writing_an_id() {
    let register = residue().expect("well formed");
    assert!(register.find("registry overview").is_some());
    assert!(register.find("  Opentelemetry Adapter  ").is_none());
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
        "\"verdict\":\"discharged_elsewhere\",\"by\":[\"bioprism-packs\",\"bioprism-factory\"]",
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
