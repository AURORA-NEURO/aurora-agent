from prism_sdk.research_contracts import EvidenceReceipt, PolicyReceipt, ReleaseReview, ResearchContractError, research_artifact_digest


def test_empty_evidence_is_explicit_unknown():
    receipt = EvidenceReceipt(
        receipt_id="evidence:q1",
        intent="retrieve",
        sources=(),
        derivation=("feature:AFA-bioir-P02-F01",),
        uncertainty=(("kind", "no admissible evidence"),),
        omissions=(("item", "query:q1"),),
        conclusion_state="unknown",
    )
    receipt.validate()


def test_unresolved_policy_cannot_allow():
    receipt = PolicyReceipt(receipt_id="policy:q1", decision="allow", reasons=("unresolved",))
    try:
        receipt.validate()
    except ResearchContractError:
        return
    raise AssertionError("unresolved policy was accepted")


def test_artifact_digest_is_stable_for_key_order():
    assert research_artifact_digest({"b": 2, "a": 1}) == research_artifact_digest({"a": 1, "b": 2})


def test_release_review_rejects_pass_without_provenance():
    review = ReleaseReview(
        capability_id="capability:demo",
        card_digest="a" * 64,
        verdict="pass",
        reasons=("all gates passed",),
        provenance_complete=False,
    )
    try:
        review.validate()
    except ResearchContractError:
        return
    raise AssertionError("a passing review without provenance was accepted")


def test_release_review_digest_is_stable():
    review = ReleaseReview(
        capability_id="capability:demo",
        card_digest="a" * 64,
        verdict="blocked",
        reasons=("replication floor unmet",),
    )
    assert review.digest() == review.digest()
