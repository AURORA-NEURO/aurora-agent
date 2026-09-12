from __future__ import annotations

import json
from pathlib import Path
import tempfile
import unittest
from urllib.parse import parse_qs, unquote, urlsplit

from prism_sdk.public_literature_refresh import (
    PUBMED_SPECIALTY_LANES,
    PublicLiteratureRefreshError,
    atomic_refresh_neurosurgical_public_literature,
    bundle_digest,
    refresh_neurosurgical_public_literature,
    validate_public_literature_bundle,
)


def _fake_pubmed_fetch(url: str):
    if "esearch.fcgi" in url:
        term = unquote(url.split("term=", 1)[1].split("&", 1)[0])
        specialty = next(
            key for key, value in PUBMED_SPECIALTY_LANES.items() if value == term
        )
        pmid = str(10_000 + list(PUBMED_SPECIALTY_LANES).index(specialty))
        return {"esearchresult": {"idlist": [pmid]}}
    pmid = url.split("id=", 1)[1].split("&", 1)[0]
    if "esummary.fcgi" in url:
        return {
            "result": {
                pmid: {
                    "title": "Observed PubMed cohort",
                    "fulljournalname": "Journal of Neurosurgery",
                    "pubdate": "2026 Jan 02",
                    "articleids": [{"idtype": "doi", "value": "10.1000/aurora"}],
                }
            }
        }
    return (
        "<PubmedArticleSet><PubmedArticle><MedlineCitation>"
        f'<PMID>{pmid}</PMID><Article><Abstract><AbstractText Label="BACKGROUND">'
        "Observed source abstract</AbstractText></Abstract><PublicationTypeList>"
        "<PublicationType>Journal Article</PublicationType></PublicationTypeList></Article>"
        "<MeshHeadingList><MeshHeading><DescriptorName>Neurosurgery</DescriptorName>"
        "</MeshHeading></MeshHeadingList></MedlineCitation></PubmedArticle></PubmedArticleSet>"
    ).encode()


class PublicLiteratureRefreshTests(unittest.TestCase):
    def test_refresh_builds_rust_digest_parity_bundle_without_network(self) -> None:
        bundle, report = refresh_neurosurgical_public_literature(
            fetch=_fake_pubmed_fetch,
            per_specialty_limit=1,
            retrieved_at="2026-08-30T06:02:51Z",
        )
        validate_public_literature_bundle(bundle)
        self.assertEqual(report.bundle_digest, bundle_digest(bundle))
        self.assertEqual(report.record_count, 6)
        self.assertEqual(set(report.specialty_counts), set(PUBMED_SPECIALTY_LANES))
        self.assertFalse(bundle["synthetic_data"])
        self.assertEqual(len(bundle["sources"][0]["content_sha256"]), 64)

    def test_registered_ncbi_contact_is_on_every_request_but_not_in_the_bundle_or_report(
        self,
    ) -> None:
        calls: list[str] = []

        def observed_fetch(url: str):
            calls.append(url)
            return _fake_pubmed_fetch(url)

        tool = "aurora_registered_research"
        email = "eutilities-contact@example.org"
        bundle, report = refresh_neurosurgical_public_literature(
            fetch=observed_fetch,
            per_specialty_limit=1,
            specialty_lanes=("glioma",),
            retrieved_at="2026-08-30T06:02:51Z",
            ncbi_tool=tool,
            ncbi_email=email,
        )

        self.assertEqual(len(calls), 3)
        for url in calls:
            parameters = parse_qs(urlsplit(url).query, strict_parsing=True)
            self.assertEqual(parameters["tool"], [tool])
            self.assertEqual(parameters["email"], [email])
            self.assertNotIn("api_key", parameters)
        serialized = json.dumps(
            {"bundle": bundle, "report": report.to_dict()}, sort_keys=True
        )
        self.assertNotIn(tool, serialized)
        self.assertNotIn(email, serialized)

    def test_ncbi_contact_must_be_a_bounded_pair_before_any_request(self) -> None:
        for tool, email in (
            ("aurora", None),
            (None, "developer@example.org"),
            ("aurora research", "developer@example.org"),
            ("aurora", "not-an-email"),
        ):
            calls: list[str] = []
            with self.assertRaises(PublicLiteratureRefreshError):
                refresh_neurosurgical_public_literature(
                    fetch=lambda url: calls.append(url),
                    per_specialty_limit=1,
                    specialty_lanes=("glioma",),
                    ncbi_tool=tool,
                    ncbi_email=email,
                )
            self.assertEqual(calls, [])

    def test_refresh_rejects_malformed_or_empty_lane_before_write(self) -> None:
        def empty_fetch(url: str):
            if "esearch.fcgi" in url:
                return {"esearchresult": {"idlist": []}}
            return {}

        with self.assertRaises(PublicLiteratureRefreshError):
            refresh_neurosurgical_public_literature(
                fetch=empty_fetch, per_specialty_limit=1
            )

    def test_checked_in_snapshot_matches_rust_bundle_digest(self) -> None:
        snapshot = (
            Path(__file__).parents[2]
            / "data"
            / "neurosurgery"
            / "neurosurgical_public_literature_snapshot.json"
        )
        bundle = json.loads(snapshot.read_text(encoding="utf-8"))
        validate_public_literature_bundle(bundle)
        self.assertEqual(
            bundle_digest(bundle),
            "a75c0216a584a29c2ec02dc98b3d7353a2869adeb2d3accbaf22a0503cb10d6e",
        )

    def test_synthetic_marker_is_rejected_even_when_flag_is_false(self) -> None:
        bundle, _ = refresh_neurosurgical_public_literature(
            fetch=_fake_pubmed_fetch,
            per_specialty_limit=1,
            retrieved_at="2026-08-30T06:02:51Z",
        )
        bundle["records"][0]["title"] = "Synthetic fixture title"
        with self.assertRaises(PublicLiteratureRefreshError):
            validate_public_literature_bundle(bundle)

    def test_partial_pubmed_dates_remain_missing_instead_of_becoming_january_first(
        self,
    ) -> None:
        def partial_date_fetch(url: str):
            response = _fake_pubmed_fetch(url)
            if "esummary.fcgi" in url:
                response = dict(response)
                response["result"] = dict(response["result"])
                pmid = next(iter(response["result"]))
                article = dict(response["result"][pmid])
                article["pubdate"] = "2026 Jan"
                response["result"][pmid] = article
            return response

        bundle, _ = refresh_neurosurgical_public_literature(
            fetch=partial_date_fetch,
            per_specialty_limit=1,
            retrieved_at="2026-08-30T06:02:51Z",
        )
        self.assertIsNone(bundle["records"][0]["publication_date"])

        def year_only_fetch(url: str):
            response = _fake_pubmed_fetch(url)
            if "esummary.fcgi" in url:
                response = dict(response)
                response["result"] = dict(response["result"])
                pmid = next(iter(response["result"]))
                article = dict(response["result"][pmid])
                article["pubdate"] = "2026"
                response["result"][pmid] = article
            return response

        bundle, _ = refresh_neurosurgical_public_literature(
            fetch=year_only_fetch,
            per_specialty_limit=1,
            retrieved_at="2026-08-30T06:02:51Z",
        )
        self.assertIsNone(bundle["records"][0]["publication_date"])

    def test_atomic_refresh_replaces_candidate_only_after_validation(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "literature.json"
            report = atomic_refresh_neurosurgical_public_literature(
                output,
                fetch=_fake_pubmed_fetch,
                per_specialty_limit=1,
                retrieved_at="2026-08-30T06:02:51Z",
            )
            self.assertEqual(report.output_path, str(output))
            persisted = json.loads(output.read_text(encoding="utf-8"))
            self.assertEqual(bundle_digest(persisted), report.bundle_digest)
            self.assertFalse(list(Path(directory).glob("*.candidate")))

    def test_atomic_refresh_keeps_existing_snapshot_when_network_fails(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "literature.json"
            output.write_text("last-known-good", encoding="utf-8")

            def failed_fetch(_url: str):
                raise OSError("transport unavailable")

            with self.assertRaises(PublicLiteratureRefreshError):
                atomic_refresh_neurosurgical_public_literature(
                    output, fetch=failed_fetch
                )
            self.assertEqual(output.read_text(encoding="utf-8"), "last-known-good")


if __name__ == "__main__":
    unittest.main()
