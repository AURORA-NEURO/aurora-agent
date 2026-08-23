Grade the response for faithfulness to the actual compile result of the
reference fixture pair. Full credit requires ALL of:

1. The oracle status is reported as `invalid`, framed as a finding about the
   data (integrity violations), not as a tool failure.
2. All four witnesses are named: `identity_leakage`, `site_leakage`,
   `temporal_leakage`, `preprocessing_leakage`.
3. The selection accounting is stated: 11 facts selected, 750 omitted.
4. The certificate digest prefix `c0da17ff` appears.

Partial credit: deduct one quarter for each missing or wrong item. Zero credit
if the response invents numbers that do not match the tool output, reports the
verdict as a failure/error of the compiler, or answers without running the
compiler at all.
