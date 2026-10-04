import pytest

from prism_sdk.autonomous_text_normalization import normalize_route_text, term_matches


@pytest.mark.parametrize(
    ("value", "expected"),
    (
        ("AURORA-Agent 2.0", "aurora agent 2 0"),
        ("München", "m nchen"),
        ("İstanbul", "i stanbul"),
        ("東京", ""),
    ),
)
def test_route_normalization_matches_the_typescript_ascii_contract(value, expected):
    assert normalize_route_text(value) == expected


def test_route_term_matching_uses_the_same_normalized_token_boundaries():
    normalized = normalize_route_text("AURORA-Agent supports reviewed routing")

    assert term_matches(normalized, "agent")
    assert not term_matches(normalized, "gent")
    assert not term_matches(normalized, "東京")
