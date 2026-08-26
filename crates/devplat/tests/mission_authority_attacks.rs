//! Adversarial coverage of the mission contract's authority boundary.
//!
//! `plan_mission` is the *only* place a mission's tool allow-list, recursion guard, side-effect
//! posture, and output budgets are enforced: the MCP executor calls it once and then dispatches
//! every step without re-checking any of them. That makes `MissionRequest::validate` a single
//! choke point, and a single choke point deserves to be attacked from every direction rather than
//! demonstrated on a happy path.
//!
//! Each test below is one attack. The claim in the name is what the attack must fail to achieve.

use bioprism_devplat::{
    apply_binding, plan_mission, MissionBinding, MissionError, MissionPolicy, MissionRequest,
    MissionStep,
};
use serde_json::{json, Value};

fn step(id: &str, tool: &str, deps: &[&str]) -> MissionStep {
    MissionStep {
        id: id.into(),
        domain: "metrics".into(),
        capability: "analytics".into(),
        objective: format!("run {id}"),
        tool: tool.into(),
        arguments: json!({}),
        depends_on: deps.iter().map(|value| (*value).into()).collect(),
        bindings: Vec::new(),
        required: true,
    }
}

fn mission(steps: Vec<MissionStep>) -> MissionRequest {
    MissionRequest {
        mission_id: "attack-mission".into(),
        goal: "probe the authority boundary".into(),
        steps,
        policy: MissionPolicy::default(),
        claim_requests: Vec::new(),
        evaluator_review: None,
        workflow_binding: None,
        route_review: None,
    }
}

/// A mission that is authorised to execute exactly the tools it names, which is the posture every
/// authority attack below has to break out of.
fn executing(steps: Vec<MissionStep>, allowed: &[&str]) -> MissionRequest {
    let mut request = mission(steps);
    request.policy.execute = true;
    request.policy.allowed_tools = allowed.iter().map(|tool| (*tool).to_string()).collect();
    request
}

fn refusal(request: &MissionRequest) -> MissionError {
    plan_mission(request).expect_err("this mission must be refused")
}

mod authority {
    use super::*;

    #[test]
    fn a_tool_differing_from_an_allow_listed_tool_only_by_case_is_not_allow_listed() {
        for disguise in [
            "Metrics_Analytics_Audit",
            "METRICS_ANALYTICS_AUDIT",
            "metrics_Analytics_audit",
        ] {
            let request = executing(
                vec![step("one", disguise, &[])],
                &["metrics_analytics_audit"],
            );
            assert!(
                matches!(refusal(&request), MissionError::ToolNotAllowed { tool, .. } if tool == disguise),
                "`{disguise}` must not inherit the allow-listed lowercase tool's authority"
            );
        }
    }

    #[test]
    fn a_tool_differing_from_an_allow_listed_tool_only_by_whitespace_is_refused_as_unsafe() {
        for disguise in [
            "metrics_analytics_audit ",
            " metrics_analytics_audit",
            "metrics_analytics audit",
            "metrics_analytics\taudit",
        ] {
            let request = executing(
                vec![step("one", disguise, &[])],
                &["metrics_analytics_audit"],
            );
            assert!(
                matches!(refusal(&request), MissionError::UnsafeTool { .. }),
                "`{disguise:?}` must be refused as an unsafe identifier, not trimmed into authority"
            );
        }
    }

    #[test]
    fn an_allow_list_entry_carrying_whitespace_cannot_launder_a_padded_tool_name() {
        let request = executing(
            vec![step("one", "metrics_analytics_audit ", &[])],
            &["metrics_analytics_audit "],
        );
        assert!(
            matches!(refusal(&request), MissionError::UnsafeTool { .. }),
            "an allow-list cannot legitimise a padded identifier by carrying the same padding"
        );
    }

    #[test]
    fn a_duplicate_step_id_is_refused_even_when_the_shadowing_step_is_allow_listed() {
        let request = executing(
            vec![
                step("one", "metrics_analytics_audit", &[]),
                step("one", "metrics_analytics_audit", &[]),
            ],
            &["metrics_analytics_audit"],
        );
        assert!(matches!(
            refusal(&request),
            MissionError::Duplicate {
                kind: "mission step",
                ..
            }
        ));
    }

    #[test]
    fn a_duplicate_step_id_hiding_an_unallowed_tool_is_refused_whichever_copy_comes_first() {
        for order in [0usize, 1] {
            let mut steps = vec![
                step("one", "metrics_analytics_audit", &[]),
                step("one", "release_audit", &[]),
            ];
            if order == 1 {
                steps.reverse();
            }
            let request = executing(steps, &["metrics_analytics_audit"]);
            let error = refusal(&request);
            assert!(
                matches!(
                    error,
                    MissionError::Duplicate { .. } | MissionError::ToolNotAllowed { .. }
                ),
                "a shadowed step id must never let `release_audit` through: got {error:?}"
            );
        }
    }

    #[test]
    fn a_binding_target_pointer_writes_only_inside_its_own_steps_arguments() {
        let binding = MissionBinding {
            from_step: "source".into(),
            source_pointer: String::new(),
            target_pointer: "/tool".into(),
        };
        let mut arguments = json!({ "tool": null });
        apply_binding(&mut arguments, &binding, &json!("release_audit")).unwrap();

        let mut request = executing(
            vec![
                step("source", "metrics_analytics_audit", &[]),
                step("sink", "metrics_analytics_audit", &["source"]),
            ],
            &["metrics_analytics_audit"],
        );
        request.steps[1].arguments = arguments.clone();
        request.steps[1].bindings = vec![binding];
        let plan = plan_mission(&request).expect("a binding into an argument slot is legitimate");

        assert_eq!(
            arguments["tool"], "release_audit",
            "the binding wrote into the arguments object, as JSON pointers are resolved there"
        );
        assert!(
            plan.steps
                .iter()
                .all(|planned| planned.tool == "metrics_analytics_audit"),
            "an argument named `tool` is data; it never becomes the dispatched tool"
        );
    }

    #[test]
    fn an_escaped_json_pointer_token_addresses_a_literal_key_and_never_a_parent() {
        let binding = MissionBinding {
            from_step: "source".into(),
            source_pointer: "/~0escaped".into(),
            target_pointer: "/~1slash".into(),
        };
        let mut arguments = json!({ "/slash": null, "slash": "untouched" });
        apply_binding(
            &mut arguments,
            &binding,
            &json!({ "~escaped": 7, "escaped": 99 }),
        )
        .unwrap();
        assert_eq!(arguments["/slash"], json!(7));
        assert_eq!(arguments["slash"], json!("untouched"));
    }

    #[test]
    fn a_binding_target_pointer_that_resolves_nowhere_is_refused_before_execution() {
        let mut request = executing(
            vec![
                step("source", "metrics_analytics_audit", &[]),
                step("sink", "metrics_analytics_audit", &["source"]),
            ],
            &["metrics_analytics_audit"],
        );
        request.steps[1].arguments = json!({ "inputs": {} });
        request.steps[1].bindings = vec![MissionBinding {
            from_step: "source".into(),
            source_pointer: String::new(),
            target_pointer: "/inputs/absent".into(),
        }];
        assert!(matches!(
            refusal(&request),
            MissionError::MissingPointer { .. }
        ));
    }

    #[test]
    fn a_binding_may_not_read_from_a_step_that_is_not_a_direct_dependency() {
        let mut request = executing(
            vec![
                step("source", "metrics_analytics_audit", &[]),
                step("middle", "metrics_analytics_audit", &["source"]),
                step("sink", "metrics_analytics_audit", &["middle"]),
            ],
            &["metrics_analytics_audit"],
        );
        request.steps[2].arguments = json!({ "slot": null });
        request.steps[2].bindings = vec![MissionBinding {
            from_step: "source".into(),
            source_pointer: String::new(),
            target_pointer: "/slot".into(),
        }];
        assert!(matches!(
            refusal(&request),
            MissionError::BindingWithoutDependency { .. }
        ));
    }

    #[test]
    fn a_plan_only_mission_never_reports_itself_as_authorized_to_execute() {
        let mut request = mission(vec![step("one", "release_audit", &[])]);
        request.policy.allowed_tools = vec!["metrics_analytics_audit".into()];
        let plan = plan_mission(&request).expect("planning does not require an allow-list");
        assert_eq!(
            plan.execution, "planned",
            "an unauthorised tool may be planned, so the plan must not claim execution authority"
        );

        request.policy.execute = true;
        assert!(matches!(
            refusal(&request),
            MissionError::ToolNotAllowed { .. }
        ));
    }
}

mod recursion {
    use super::*;

    #[test]
    fn a_step_naming_agent_mission_is_refused_as_recursive() {
        let request = executing(
            vec![step("one", "agent_mission", &[])],
            &["metrics_analytics_audit"],
        );
        assert_eq!(
            refusal(&request),
            MissionError::RecursiveTool,
            "the step's own guard must fire; leaving this to the allow-list would let a mission \
             that happens to allow-list the mission tool decide the recursion question"
        );
    }

    #[test]
    fn an_allow_list_naming_agent_mission_is_refused_even_when_no_step_uses_it() {
        let request = executing(
            vec![step("one", "metrics_analytics_audit", &[])],
            &["metrics_analytics_audit", "agent_mission"],
        );
        assert_eq!(refusal(&request), MissionError::RecursiveTool);
    }

    #[test]
    fn a_plan_only_mission_still_refuses_a_recursive_step() {
        let request = mission(vec![step("one", "agent_mission", &[])]);
        assert_eq!(
            refusal(&request),
            MissionError::RecursiveTool,
            "the recursion guard must not be conditional on the execute flag"
        );
    }

    #[test]
    fn a_case_variant_of_agent_mission_is_refused_by_the_recursion_guard_not_merely_unmatched() {
        for disguise in ["Agent_Mission", "AGENT_MISSION", "agent_Mission"] {
            let request = executing(vec![step("one", disguise, &[])], &[disguise]);
            assert_eq!(
                refusal(&request),
                MissionError::RecursiveTool,
                "`{disguise}` differs from the mission tool only by ASCII case and must be \
                 refused as recursive rather than left to an exact-match dispatch table"
            );
        }
    }

    #[test]
    fn a_case_variant_of_agent_mission_in_the_allow_list_is_refused() {
        let request = executing(
            vec![step("one", "metrics_analytics_audit", &[])],
            &["metrics_analytics_audit", "Agent_Mission"],
        );
        assert_eq!(refusal(&request), MissionError::RecursiveTool);
    }

    /// The executor builds each nested call as `{"name": step.tool, "arguments": effective}`,
    /// where `effective` is the step's arguments with every binding applied. A binding's only
    /// handle on that pair is an RFC 6901 pointer resolved against `effective`, so the name slot
    /// is out of its reach by construction — there is no runtime check to defeat, and the two
    /// halves of this test say so in the two ways that can be observed.
    ///
    /// The first half makes the attempt real rather than described: it binds the literal
    /// `agent_mission` in through the production [`apply_binding`] and asserts the payload
    /// actually arrived, so the case cannot pass by failing to attack. The second half moves the
    /// same string one slot over, into the field the name is read from, and that is where the
    /// recursion guard answers.
    #[test]
    fn a_binding_lands_agent_mission_in_the_arguments_while_the_name_slot_stays_the_planned_tool() {
        let binding = MissionBinding {
            from_step: "source".into(),
            source_pointer: "/chosen_tool".into(),
            target_pointer: "/tool".into(),
        };
        let mut request = executing(
            vec![
                step("source", "metrics_analytics_audit", &[]),
                step("sink", "metrics_analytics_audit", &["source"]),
            ],
            &["metrics_analytics_audit"],
        );
        request.steps[1].arguments = json!({ "tool": "metrics_analytics_audit" });
        request.steps[1].bindings = vec![binding.clone()];

        let plan = plan_mission(&request).expect("binding into an argument slot is legitimate");
        let sink = plan
            .steps
            .iter()
            .find(|planned| planned.id == "sink")
            .expect("the sink step survives planning");

        let mut effective = request.steps[1].arguments.clone();
        apply_binding(
            &mut effective,
            &binding,
            &json!({ "chosen_tool": "agent_mission" }),
        )
        .expect("the prerequisite payload resolves at the source pointer");
        assert_eq!(
            effective["tool"], "agent_mission",
            "the attack has to land before the refusal means anything: the mission tool's name is \
             now sitting in this step's arguments"
        );

        let dispatched = json!({ "name": sink.tool, "arguments": effective });
        assert_eq!(
            dispatched["name"], "metrics_analytics_audit",
            "the nested call takes its name from the step field the planner validated, so the \
             smuggled string stays an argument value"
        );
        assert!(
            !sink.tool.eq_ignore_ascii_case("agent_mission"),
            "no casing of the mission tool reached the name slot"
        );
    }

    /// The reason the smuggling attempt above is worth trying at all: a step that carries bindings
    /// has arguments that are only finalised at execution time, and a planner that deferred the
    /// whole step's checks to match would leave the recursion guard running after dispatch. It
    /// does not — the guard is a planning-time refusal that bindings do not postpone.
    #[test]
    fn a_step_carrying_bindings_is_refused_at_planning_time_when_its_own_tool_is_the_mission_tool()
    {
        for disguise in ["agent_mission", "Agent_Mission", "AGENT_MISSION"] {
            let mut request = executing(
                vec![
                    step("source", "metrics_analytics_audit", &[]),
                    step("sink", disguise, &["source"]),
                ],
                &["metrics_analytics_audit"],
            );
            request.steps[1].arguments = json!({ "tool": null });
            request.steps[1].bindings = vec![MissionBinding {
                from_step: "source".into(),
                source_pointer: String::new(),
                target_pointer: "/tool".into(),
            }];
            assert_eq!(
                refusal(&request),
                MissionError::RecursiveTool,
                "`{disguise}` must be refused while the mission is still a plan, not left for a \
                 check that runs once its arguments are finalised"
            );
        }
    }
}

mod side_effects {
    use super::*;

    fn confirming(arguments: Value) -> MissionRequest {
        let mut request = executing(
            vec![step("one", "metrics_analytics_audit", &[])],
            &["metrics_analytics_audit"],
        );
        request.steps[0].arguments = arguments;
        request
    }

    #[test]
    fn a_confirmation_nested_deep_inside_an_object_is_refused() {
        let request = confirming(json!({
            "a": { "b": { "c": { "d": { "e": { "confirm": true } } } } }
        }));
        assert!(matches!(
            refusal(&request),
            MissionError::SideEffectsDisallowed { .. }
        ));
    }

    #[test]
    fn a_confirmation_inside_an_array_element_is_refused() {
        let request = confirming(json!({ "batch": [{ "op": "write", "confirm": true }] }));
        assert!(matches!(
            refusal(&request),
            MissionError::SideEffectsDisallowed { .. }
        ));
    }

    #[test]
    fn a_confirmation_inside_nested_arrays_is_refused() {
        let request = confirming(json!({ "batch": [[[{ "confirm": true }]]] }));
        assert!(matches!(
            refusal(&request),
            MissionError::SideEffectsDisallowed { .. }
        ));
    }

    #[test]
    fn a_confirmation_beside_a_long_run_of_innocent_keys_is_still_found() {
        let mut arguments = serde_json::Map::new();
        for index in 0..256 {
            arguments.insert(format!("key_{index}"), json!(index));
        }
        arguments.insert("confirm".into(), json!(true));
        let request = confirming(Value::Object(arguments));
        assert!(matches!(
            refusal(&request),
            MissionError::SideEffectsDisallowed { .. }
        ));
    }

    #[test]
    fn a_confirmation_in_the_last_of_several_steps_is_found() {
        let mut request = executing(
            vec![
                step("one", "metrics_analytics_audit", &[]),
                step("two", "metrics_analytics_audit", &[]),
                step("three", "metrics_analytics_audit", &[]),
            ],
            &["metrics_analytics_audit"],
        );
        request.steps[2].arguments = json!({ "nested": { "confirm": true } });
        assert!(
            matches!(refusal(&request), MissionError::SideEffectsDisallowed { step } if step == "three")
        );
    }

    #[test]
    fn a_false_confirmation_is_not_a_side_effect_and_a_true_one_needs_authority() {
        let allowed = confirming(json!({ "nested": { "confirm": false } }));
        assert!(plan_mission(&allowed).is_ok());

        let mut refused = confirming(json!({ "nested": { "confirm": true } }));
        assert!(matches!(
            refusal(&refused),
            MissionError::SideEffectsDisallowed { .. }
        ));
        refused.policy.allow_side_effects = true;
        assert_eq!(plan_mission(&refused).unwrap().execution, "authorized");
    }
}

mod budgets {
    use super::*;

    fn budgeted(step_bytes: usize, total_bytes: usize) -> MissionRequest {
        let mut request = executing(
            vec![step("one", "metrics_analytics_audit", &[])],
            &["metrics_analytics_audit"],
        );
        request.policy.max_step_output_bytes = step_bytes;
        request.policy.max_total_output_bytes = total_bytes;
        request
    }

    #[test]
    fn a_per_step_budget_above_the_total_budget_is_refused_as_an_ordering_error() {
        assert_eq!(
            refusal(&budgeted(2_000_001, 2_000_000)),
            MissionError::OutputBudgetOrder
        );
    }

    #[test]
    fn a_zero_or_over_ceiling_output_budget_is_refused_rather_than_clamped() {
        assert!(matches!(
            refusal(&budgeted(0, 1_000)),
            MissionError::InvalidLimit {
                field: "policy.max_step_output_bytes",
                value: 0
            }
        ));
        assert!(matches!(
            refusal(&budgeted(1_000, 0)),
            MissionError::InvalidLimit {
                field: "policy.max_total_output_bytes",
                value: 0
            }
        ));
        assert!(matches!(
            refusal(&budgeted(1_000, 20_000_001)),
            MissionError::InvalidLimit {
                field: "policy.max_total_output_bytes",
                ..
            }
        ));
        assert!(matches!(
            refusal(&budgeted(20_000_001, 20_000_001)),
            MissionError::InvalidLimit {
                field: "policy.max_step_output_bytes",
                ..
            }
        ));
    }

    #[test]
    fn a_parallel_wave_reserves_its_worst_case_before_a_single_step_is_dispatched() {
        let mut request = executing(
            vec![
                step("one", "metrics_analytics_audit", &[]),
                step("two", "metrics_analytics_audit", &[]),
                step("three", "metrics_analytics_audit", &[]),
            ],
            &["metrics_analytics_audit"],
        );
        request.policy.execution_mode = "parallel_waves".into();
        request.policy.max_step_output_bytes = 1_000_000;
        request.policy.max_total_output_bytes = 2_999_999;
        assert!(matches!(
            refusal(&request),
            MissionError::ParallelWaveBudget {
                required: 3_000_000,
                available: 2_999_999
            }
        ));

        request.policy.max_total_output_bytes = 3_000_000;
        assert_eq!(plan_mission(&request).unwrap().waves[0].len(), 3);
    }

    #[test]
    fn the_worst_case_reservation_counts_the_widest_wave_not_the_step_total() {
        let mut request = executing(
            vec![
                step("one", "metrics_analytics_audit", &[]),
                step("two", "metrics_analytics_audit", &["one"]),
                step("three", "metrics_analytics_audit", &["two"]),
            ],
            &["metrics_analytics_audit"],
        );
        request.policy.execution_mode = "parallel_waves".into();
        request.policy.max_step_output_bytes = 1_000_000;
        request.policy.max_total_output_bytes = 1_000_000;
        let plan = plan_mission(&request)
            .expect("a serialized chain never has more than one step in flight");
        assert_eq!(plan.waves.len(), 3);
        assert!(plan.waves.iter().all(|wave| wave.len() == 1));
    }

    #[test]
    fn a_step_count_above_the_policy_maximum_is_refused_rather_than_trimmed() {
        let mut request = executing(
            vec![
                step("one", "metrics_analytics_audit", &[]),
                step("two", "metrics_analytics_audit", &[]),
            ],
            &["metrics_analytics_audit"],
        );
        request.policy.max_steps = 1;
        let error = refusal(&request);
        assert!(
            matches!(
                error,
                MissionError::TooMany {
                    kind: "mission steps",
                    count: 2,
                    maximum: 1
                }
            ),
            "{error:?}"
        );
    }

    #[test]
    fn a_policy_max_steps_above_the_hard_ceiling_is_refused_not_silently_capped() {
        let mut request = executing(
            vec![step("one", "metrics_analytics_audit", &[])],
            &["metrics_analytics_audit"],
        );
        request.policy.max_steps = 129;
        assert!(matches!(
            refusal(&request),
            MissionError::InvalidLimit {
                field: "policy.max_steps",
                value: 129
            }
        ));
    }
}
