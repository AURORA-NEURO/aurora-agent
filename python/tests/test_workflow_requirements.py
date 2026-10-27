"""Caller contracts constrain real workflow inputs without inventing NLP fidelity."""

from copy import deepcopy
import json
import unittest

from prism_sdk.authoring import content_digest
from prism_sdk.workflow_integrations import WorkflowOperation, WorkflowOperationCatalog, WorkflowOperationResult
from prism_sdk.workflow_requirements import (
    WorkflowRequirementError,
    WorkflowRequirements,
    audit_workflow_requirements,
    compile_workflow_requirements,
    enforce_workflow_step_requirements,
    validate_workflow_requirements,
)


_TASK = "Build the explicit public research workflow"


def _catalog():
    catalog = WorkflowOperationCatalog()
    for operation in ("source.read", "bridge.transform", "target.review", "forbidden.read"):
        catalog.register(WorkflowOperation(operation, "Local test contract fixture", {"type": "object"}, {"type": "object"}, "fixture", implementation="local"), lambda *_args, **_kwargs: WorkflowOperationResult({}, {}))
    return catalog


def _step(step_id="source", operation="source.read", arguments=None, depends_on=()):
    return {"id": step_id, "operation": operation, "arguments": {} if arguments is None else arguments, "depends_on": list(depends_on)}


def _proposal(arguments=None):
    return {"name": "Explicit research workflow", "steps": [_step(arguments=arguments)], "outputs": ["source"]}


def _rule(path="/value", comparison="equals", value=None, operation="source.read"):
    return {"operation": operation, "path": path, "comparison": comparison, "value": value}


def _contract(**kwargs):
    return compile_workflow_requirements(_TASK, **kwargs)


def _restamp(contract):
    contract["requirements_digest"] = content_digest({key: value for key, value in contract.items() if key != "requirements_digest"})
    return contract


class WorkflowRequirementTests(unittest.TestCase):
    def test_contract_is_immutable_digest_bound_and_implies_required_operations(self):
        contract = _contract(output_operations=["target.review"], argument_rules=[_rule(operation="source.read", value=3)], data_links=[{"source_operation": "source.read", "target_operation": "target.review"}])
        raw = contract.to_dict()
        self.assertEqual(set(raw["required_operations"]), {"source.read", "target.review"})
        self.assertEqual(raw["task_digest"], content_digest({"task": _TASK}))
        self.assertEqual(contract.requirements_digest, content_digest({key: value for key, value in raw.items() if key != "requirements_digest"}))
        raw["max_steps"] = 1
        self.assertEqual(contract.to_dict()["max_steps"], 32)
        self.assertEqual(validate_workflow_requirements(contract, _catalog()).to_dict(), contract.to_dict())

    def test_task_binding_is_checked_for_explicit_plan_digest_and_validation(self):
        contract = _contract(required_operations=["source.read"])
        expected = content_digest({"task": _TASK})
        validate_workflow_requirements(contract, task_digest=expected)
        with self.assertRaisesRegex(WorkflowRequirementError, "different task"):
            validate_workflow_requirements(contract, task_digest=content_digest({"task": "changed"}))
        proposal = {**_proposal(), "task_digest": content_digest({"task": "changed"})}
        with self.assertRaisesRegex(WorkflowRequirementError, "different task"):
            audit_workflow_requirements(proposal, contract)
        # Plain normalized proposals are allowed; the builder validates their task separately.
        self.assertTrue(audit_workflow_requirements(_proposal(), contract)["contract_satisfied"])
        for invalid in (False, 0, [], "not-a-digest"):
            with self.assertRaises(WorkflowRequirementError):
                validate_workflow_requirements(contract, task_digest=invalid)

    def test_false_zero_and_null_are_exact_distinct_values_and_missing_is_not_null(self):
        for expected in (False, 0, None, True, 1, "0"):
            contract = _contract(argument_rules=[_rule(value=expected)])
            for actual in (False, 0, None, True, 1, "0"):
                matches = type(actual) is type(expected) and actual == expected
                report = audit_workflow_requirements(_proposal({"value": actual}), contract)
                self.assertIs(report["contract_satisfied"], matches)
                if matches:
                    enforce_workflow_step_requirements("source.read", {"value": actual}, contract)
                else:
                    with self.assertRaises(WorkflowRequirementError):
                        enforce_workflow_step_requirements("source.read", {"value": actual}, contract)
        self.assertFalse(audit_workflow_requirements(_proposal({}), _contract(argument_rules=[_rule(value=None)]))["contract_satisfied"])

    def test_numeric_bounds_reject_boolean_null_nonfinite_and_non_numeric_values(self):
        for comparison in ("at_most", "at_least"):
            for invalid in (False, None, "3", [], float("nan"), float("inf"), 2**100):
                with self.assertRaises(WorkflowRequirementError):
                    _contract(argument_rules=[_rule(comparison=comparison, value=invalid)])
        contract = _contract(argument_rules=[_rule(comparison="at_most", value=3)])
        self.assertTrue(audit_workflow_requirements(_proposal({"value": 0}), contract)["contract_satisfied"])
        for invalid in (False, None, "0", [], {}):
            self.assertFalse(audit_workflow_requirements(_proposal({"value": invalid}), contract)["contract_satisfied"])

    def test_contains_uses_exact_json_values_and_does_not_coerce_false_into_zero(self):
        null = _contract(argument_rules=[_rule(comparison="contains", value=None)])
        self.assertTrue(audit_workflow_requirements(_proposal({"value": [False, None]}), null)["contract_satisfied"])
        self.assertFalse(audit_workflow_requirements(_proposal({"value": [False, 0]}), null)["contract_satisfied"])
        zero = _contract(argument_rules=[_rule(comparison="contains", value=0)])
        self.assertFalse(audit_workflow_requirements(_proposal({"value": [False]}), zero)["contract_satisfied"])
        text = _contract(argument_rules=[_rule(comparison="contains", value="glioma")])
        self.assertTrue(audit_workflow_requirements(_proposal({"value": "public glioma research"}), text)["contract_satisfied"])
        self.assertFalse(audit_workflow_requirements(_proposal({"value": {"glioma": True}}), text)["contract_satisfied"])

    def test_declared_dependencies_alone_cannot_satisfy_required_value_flow(self):
        contract = _contract(data_links=[{"source_operation": "source.read", "target_operation": "target.review"}])
        proposal = {"name": "Flow", "steps": [_step(), _step("target", "target.review", {"value": "not from source"}, ["source"])], "outputs": ["target"]}
        report = audit_workflow_requirements(proposal, contract, _catalog())
        self.assertFalse(report["contract_satisfied"])
        self.assertEqual(report["blocking_issues"][0]["code"], "required_value_flow_missing")
        proposal["steps"][1]["arguments"] = {"value": {"$ref": "source#/records"}}
        self.assertTrue(audit_workflow_requirements(proposal, contract, _catalog())["contract_satisfied"])

    def test_transitive_value_references_are_admitted_but_broken_intermediate_links_fail(self):
        contract = _contract(data_links=[{"source_operation": "source.read", "target_operation": "target.review"}])
        proposal = {"name": "Flow", "steps": [_step(), _step("bridge", "bridge.transform", {"value": {"$ref": "source#"}}, ["source"]), _step("target", "target.review", {"value": {"$ref": "bridge#/result"}}, ["bridge"])], "outputs": ["target"]}
        self.assertTrue(audit_workflow_requirements(proposal, contract, _catalog())["contract_satisfied"])
        proposal["steps"][1]["arguments"] = {"value": "constant"}
        self.assertFalse(audit_workflow_requirements(proposal, contract, _catalog())["contract_satisfied"])

    def test_deferred_rule_must_be_satisfied_by_resolved_arguments_before_dispatch(self):
        contract = _contract(argument_rules=[_rule(value=3, operation="target.review")])
        proposal = {"name": "Deferred", "steps": [_step(), _step("target", "target.review", {"value": {"$ref": "source#/count"}}, ["source"])], "outputs": ["target"]}
        report = audit_workflow_requirements(proposal, contract, _catalog())
        self.assertIsNone(report["contract_satisfied"])
        self.assertEqual(len(report["deferred_arguments"]), 1)
        enforce_workflow_step_requirements("target.review", {"value": 3}, contract)
        for actual in (4, False, None, {"$ref": "source#/count"}):
            with self.assertRaises(WorkflowRequirementError):
                enforce_workflow_step_requirements("target.review", {"value": actual}, contract)

    def test_numeric_and_contains_predicates_defer_when_the_entire_value_is_a_reference(self):
        proposal = {"name": "Deferred", "steps": [_step(), _step("target", "target.review", {"value": {"$ref": "source#/value"}}, ["source"])], "outputs": ["target"]}
        for comparison, expected, resolved in (("at_most", 3, 2), ("at_least", 3, 4), ("contains", "glioma", ["glioma"])):
            contract = _contract(argument_rules=[_rule(comparison=comparison, value=expected, operation="target.review")])
            report = audit_workflow_requirements(proposal, contract, _catalog())
            self.assertIsNone(report["contract_satisfied"])
            self.assertEqual(report["blocking_issues"], [])
            enforce_workflow_step_requirements("target.review", {"value": resolved}, contract)
            with self.assertRaises(WorkflowRequirementError):
                enforce_workflow_step_requirements("target.review", {"value": {"$ref": "source#/value"}}, contract)

    def test_partial_container_equality_rejects_known_bad_siblings_before_source_calls(self):
        contract = _contract(argument_rules=[_rule(value={"selected": 3, "fixed": "glioma"}, operation="target.review")])
        proposal = {"name": "Partial", "steps": [_step(), _step("target", "target.review", {"value": {"selected": {"$ref": "source#/count"}, "fixed": "wrong"}}, ["source"])], "outputs": ["target"]}
        self.assertFalse(audit_workflow_requirements(proposal, contract, _catalog())["contract_satisfied"])
        proposal["steps"][1]["arguments"]["value"]["fixed"] = "glioma"
        self.assertIsNone(audit_workflow_requirements(proposal, contract, _catalog())["contract_satisfied"])

    def test_known_literal_can_satisfy_contains_while_unknown_member_remains_deferred_elsewhere(self):
        contract = _contract(argument_rules=[_rule(comparison="contains", value="glioma", operation="target.review")])
        proposal = {"name": "Partial", "steps": [_step(), _step("target", "target.review", {"value": ["glioma", {"$ref": "source#/tag"}]}, ["source"])], "outputs": ["target"]}
        self.assertTrue(audit_workflow_requirements(proposal, contract, _catalog())["contract_satisfied"])
        proposal["steps"][1]["arguments"]["value"][0] = "other"
        self.assertIsNone(audit_workflow_requirements(proposal, contract, _catalog())["contract_satisfied"])

    def test_json_pointer_escaping_root_arrays_and_missing_members_are_exact(self):
        contract = _contract(argument_rules=[_rule(path="/a~1b/~0key/0", value=None)])
        self.assertTrue(audit_workflow_requirements(_proposal({"a/b": {"~key": [None]}}), contract)["contract_satisfied"])
        for pointer in ("/a~1b/~0key/01", "/a~1b/~0key/-", "/a~1b/~0key/1"):
            self.assertFalse(audit_workflow_requirements(_proposal({"a/b": {"~key": [None]}}), _contract(argument_rules=[_rule(path=pointer, value=None)]))["contract_satisfied"])
        self.assertTrue(audit_workflow_requirements(_proposal({"value": None}), _contract(argument_rules=[_rule(path="", value={"value": None})]))["contract_satisfied"])
        for pointer in ("bad", "/~2", False, None, "/" + "x" * 512):
            with self.assertRaises(WorkflowRequirementError):
                _contract(argument_rules=[_rule(path=pointer)])

    def test_known_operations_are_required_in_contracts_and_all_proposal_steps(self):
        unknown = _contract(required_operations=["unknown.read"])
        with self.assertRaisesRegex(WorkflowRequirementError, "unregistered"):
            validate_workflow_requirements(unknown, _catalog())
        proposal = _proposal()
        proposal["steps"].append(_step("unknown", "unknown.read", {"value": {"$ref": "source#"}}, ["source"]))
        proposal["outputs"] = ["unknown"]
        with self.assertRaisesRegex(WorkflowRequirementError, "unregistered"):
            audit_workflow_requirements(proposal, _contract(required_operations=["source.read"]), _catalog())
        with self.assertRaises(WorkflowRequirementError):
            validate_workflow_requirements(_contract(), catalog={})

    def test_step_output_forbidden_and_per_operation_call_bounds_are_independent(self):
        contract = _contract(required_operations=["source.read"], output_operations=["target.review"], forbidden_operations=["forbidden.read"], max_calls_by_operation={"source.read": 1})
        proposal = {"name": "Bounds", "steps": [_step(), _step("second", arguments={"value": {"$ref": "source#"}}, depends_on=["source"]), _step("forbidden", "forbidden.read", {"value": {"$ref": "second#"}}, ["second"])], "outputs": ["forbidden"]}
        codes = {row["code"] for row in audit_workflow_requirements(proposal, contract, _catalog())["blocking_issues"]}
        self.assertEqual(codes, {"required_operation_missing", "required_output_missing", "forbidden_operation_used", "operation_call_budget_exceeded"})
        with self.assertRaises(WorkflowRequirementError):
            enforce_workflow_step_requirements("forbidden.read", {}, contract)
        blocked = _contract(max_calls_by_operation={"forbidden.read": 0})
        with self.assertRaises(WorkflowRequirementError):
            enforce_workflow_step_requirements("forbidden.read", {}, blocked)
        budget = _contract(required_operations=["source.read"], max_steps=1)
        proposal["steps"] = proposal["steps"][:2]
        proposal["outputs"] = ["second"]
        self.assertIn("step_budget_exceeded", [row["code"] for row in audit_workflow_requirements(proposal, budget)["blocking_issues"]])

    def test_malformed_callers_fail_as_requirement_errors_instead_of_type_errors_or_silent_defaults(self):
        invalid_options = [
            {"required_operations": "source.read"}, {"required_operations": ["source.read", "source.read"]},
            {"required_operations": [["unhashable"]]}, {"output_operations": [None]},
            {"argument_rules": [False]}, {"argument_rules": [{"operation": []}]},
            {"argument_rules": [_rule(comparison=[])]}, {"data_links": [{"source_operation": [], "target_operation": "target.review"}]},
            {"max_calls_by_operation": False}, {"max_calls_by_operation": 0}, {"max_calls_by_operation": []},
            {"max_calls_by_operation": [("source.read", 1)]}, {"max_calls_by_operation": {"source.read": False}},
            {"max_steps": False}, {"max_steps": 0}, {"max_steps": None}, {"max_steps": 33},
            {"required_operations": ["source.read"], "max_calls_by_operation": {"source.read": 0}},
            {"required_operations": ["source.read"], "forbidden_operations": ["source.read"]},
            {"required_operations": ["source.read", "target.review"], "max_steps": 1},
        ]
        for options in invalid_options:
            with self.subTest(options=options), self.assertRaises(WorkflowRequirementError):
                _contract(**options)
        for task in (None, False, "", "\ud800", "\x00", "x" * 32001):
            with self.assertRaises(WorkflowRequirementError):
                compile_workflow_requirements(task)
        for arguments in (None, False, 0, [], "value"):
            with self.assertRaises(WorkflowRequirementError):
                enforce_workflow_step_requirements("source.read", arguments, _contract())

    def test_malformed_reference_dag_and_output_inputs_cannot_fake_a_satisfied_contract(self):
        contract = _contract(required_operations=["source.read"])
        for reference in ({"$ref": False}, {"$ref": "source#", "extra": "ignored"}, {"$ref": "source#/a~2b"}, {"$ref": "missing#"}, {"$ref": "source#"}):
            with self.assertRaises(WorkflowRequirementError):
                audit_workflow_requirements(_proposal({"value": reference}), contract)
        for malformed in (False, None, [], "proposal"):
            with self.assertRaises(WorkflowRequirementError):
                audit_workflow_requirements(malformed, contract)
        for outputs in ([{}], [None], [], ["source", "source"], ["missing"]):
            with self.assertRaises(WorkflowRequirementError):
                audit_workflow_requirements({**_proposal(), "outputs": outputs}, contract)
        cyclic = {"name": "Cycle", "steps": [_step(depends_on=["target"]), _step("target", "target.review", depends_on=["source"])], "outputs": ["target"]}
        with self.assertRaisesRegex(WorkflowRequirementError, "cyclic"):
            audit_workflow_requirements(cyclic, contract)

    def test_digest_encoding_duplicate_keys_and_non_json_predicates_are_rejected(self):
        contract = _contract(required_operations=["source.read"])
        altered = contract.to_dict()
        altered["max_steps"] = 1
        with self.assertRaisesRegex(WorkflowRequirementError, "digest"):
            validate_workflow_requirements(altered)
        for encoded in ("not JSON", "[]", "null", '{"schema":"first","schema":"second"}', " " * 128001):
            with self.assertRaises(WorkflowRequirementError):
                validate_workflow_requirements(WorkflowRequirements(encoded))
        for expected in ({0: "coercion forbidden"}, {"authorization": "private"}, float("nan"), 2**100):
            with self.assertRaises(WorkflowRequirementError):
                _contract(argument_rules=[_rule(value=expected)])
        # An exact null equality remains valid after a caller-owned serialize/parse round trip.
        null = _contract(argument_rules=[_rule(value=None)])
        self.assertEqual(validate_workflow_requirements(json.loads(json.dumps(null.to_dict()))).to_dict(), null.to_dict())

    def test_every_instance_of_a_required_target_must_receive_the_required_source_value(self):
        contract = _contract(data_links=[{"source_operation": "source.read", "target_operation": "target.review"}])
        proposal = {"name": "Instances", "steps": [_step(), _step("good", "target.review", {"value": {"$ref": "source#"}}, ["source"]), _step("bad", "target.review", {"value": "constant"}, ["source"])], "outputs": ["good", "bad"]}
        report = audit_workflow_requirements(proposal, contract)
        self.assertFalse(report["contract_satisfied"])
        self.assertEqual([row["step_id"] for row in report["blocking_issues"] if row["code"] == "required_value_flow_missing"], ["bad"])


if __name__ == "__main__":
    unittest.main()
