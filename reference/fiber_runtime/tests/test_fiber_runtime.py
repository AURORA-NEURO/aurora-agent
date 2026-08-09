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

if __name__=='__main__': unittest.main()
