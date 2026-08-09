from pathlib import Path
import json, sys, unittest
HERE=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(HERE))
from fiber_compile import compile_fiber

class FiberRuntimeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.world=json.loads((HERE/'examples/radiogenomic_world.json').read_text())
        cls.query=json.loads((HERE/'examples/leakage_query.json').read_text())

    def test_deterministic(self):
        a=compile_fiber(self.world,self.query).certificate
        b=compile_fiber(self.world,self.query).certificate
        self.assertEqual(a,b)

    def test_protected_closure_retained(self):
        cert=compile_fiber(self.world,self.query).certificate
        selected=set(cert['selected_facts'])
        required={'fact.subject_aliases','fact.split','fact.site','fact.label_source','fact.decision_cut','fact.preprocess_fit','fact.specimen_dates','fact.policy','fact.negative_duplicates'}
        self.assertTrue(required.issubset(selected),required-selected)

    def test_exploratory_hub_not_followed_forward(self):
        cert=compile_fiber(self.world,self.query).certificate
        self.assertFalse(any(x.startswith('fact.explore.') for x in cert['selected_facts']))
        self.assertEqual(cert['omissions']['exploratory_facts'],750)

    def test_oracle_detects_defects(self):
        cert=compile_fiber(self.world,self.query).certificate
        self.assertEqual(cert['oracle']['status'],'invalid')
        kinds={x['type'] for x in cert['oracle']['witnesses']}
        self.assertTrue({'identity_leakage','site_leakage','preprocessing_leakage'}.issubset(kinds))

    def test_strong_reduction_on_designed_world(self):
        cert=compile_fiber(self.world,self.query).certificate
        self.assertLess(len(cert['selected_facts']),len(self.world['facts'])/20)

    def test_reference_certificate_digest_is_unmoved_by_the_policy_pass(self):
        cert=compile_fiber(self.world,self.query).certificate
        self.assertEqual(cert['certificate_sha256'],'c0da17ffc80465258345c8a538171bfd868100cd883e9a20780a0dc5477e7ea4')


class PolicyPassTests(unittest.TestCase):
    """The policy screen, against the same bytes crates/fiber/tests/policy_pass.rs compiles.

    Both suites assert the two digests below. They are the cross-language check
    for the pass; without them the policy screen would be the one part of the
    compiler with no parity guard.
    """
    CERTIFICATE='7c26ed5dee031c10b5433cb266835e8341d5f50497a8cc56ea6fc029ee90d097'
    SECTION='309ea233223cb6242d495efddb98861d3bc58ff0d024f58029b8e315bce490ae'

    @classmethod
    def setUpClass(cls):
        cls.world=json.loads((HERE/'examples/policy_restricted_world.json').read_text())
        cls.query=json.loads((HERE/'examples/policy_restricted_query.json').read_text())
        cls.closure_query=json.loads((HERE/'examples/policy_protected_closure_query.json').read_text())

    def _with_clauses(self,*clauses):
        query=dict(self.query);query['policy']=list(clauses);return query

    def test_digests_match_the_rust_engine(self):
        cert=compile_fiber(self.world,self.query).certificate
        self.assertEqual(cert['certificate_sha256'],self.CERTIFICATE)
        self.assertEqual(cert['source_hashes']['decision_section_sha256'],self.SECTION)

    def test_non_protected_evidence_is_withheld_and_named(self):
        result=compile_fiber(self.world,self.query)
        self.assertNotIn('fact.subject_aliases',result.certificate['selected_facts'])
        self.assertEqual(result.decision_section['unresolved_obligations'],
                         [{'type':'policy_blocked','detail':'fact.subject_aliases requires undeclared policy clauses: no-identifiable-export'}])
        self.assertEqual(result.decision_section['refinement_frontier'],
                         [{'action':'declare_the_required_policy_clauses_or_obtain_a_grant','facts':['fact.subject_aliases']}])

    def test_a_verdict_reached_without_the_withheld_alias_table_is_wrong(self):
        withheld=compile_fiber(self.world,self.query).certificate
        released=compile_fiber(self.world,self._with_clauses('research-only','no-identifiable-export')).certificate
        self.assertEqual(withheld['oracle']['status'],'valid')
        self.assertEqual(released['oracle']['status'],'invalid')
        self.assertEqual([x['type'] for x in released['oracle']['witnesses']],['identity_leakage'])

    def test_a_policy_withheld_protected_fact_refuses(self):
        with self.assertRaises(ValueError) as raised:
            compile_fiber(self.world,self.closure_query)
        self.assertIn('mandatory closure of 43.13',str(raised.exception))

    def test_an_ungranted_clause_is_a_policy_conflict(self):
        with self.assertRaises(ValueError) as raised:
            compile_fiber(self.world,self._with_clauses('research-only','commercial-use'))
        self.assertIn('policy conflict',str(raised.exception))

    def test_the_structure_survives_the_evidence_being_withheld(self):
        cert=compile_fiber(self.world,self.query).certificate
        self.assertEqual(cert['selected_factors'],['factor.claim_support','factor.identity_check'])

if __name__=='__main__': unittest.main()
