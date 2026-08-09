#!/usr/bin/env python3
"""AURORA FIBER v0.1 reference slicer.

This is deliberately small. It demonstrates protected closure, backward factor
slicing, the policy screen, temporal accessibility, deterministic oracle
evaluation, omission receipts, and certificate hashing. It is not the complete
mathematical runtime.

Parity note. The Rust engine in crates/fiber must emit these bytes exactly, so
every string this file builds into a Decision Section or a Certificate is wire
format: the policy obligation sentence, the refinement action, and the sorted
order of every list. Change one and the certificate digest moves.
"""
from __future__ import annotations
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any
import argparse, hashlib, json


def canonical(obj: Any) -> bytes:
    return json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode("utf-8")


def sha(obj: Any) -> str:
    return hashlib.sha256(canonical(obj)).hexdigest()


def parse_time(value: str) -> datetime:
    return datetime.fromisoformat(value.replace("Z", "+00:00")).astimezone(timezone.utc)


@dataclass(frozen=True)
class CompileResult:
    decision_section: dict[str, Any]
    certificate: dict[str, Any]


ACCEPTED_QUERY_SCHEMA_VERSIONS = ("fiber-query/0.1", "fiber-query/0.2")

# Every key fiber-query/0.2 declares, as dotted paths, sorted. Must equal
# bioprism_fiber::QUERY_FIELD_PATHS: if the two languages disagree about which keys exist they
# disagree about which queries compile, and "the Rust engine emits these bytes exactly" becomes a
# claim about a smaller set of documents than anyone thinks.
QUERY_FIELD_PATHS = (
    "budgets",
    "budgets.max_facts",
    "budgets.max_tokens",
    "decision_time",
    "distortion_tolerance",
    "goal",
    "policy",
    "protected_tags",
    "query_id",
    "role",
    "schema_version",
    "targets",
)


def validate_query(query: dict[str, Any]) -> None:
    """Version, then undeclared keys, then nothing else.

    Through 0.1 this function checked the version and let every other key through. The query
    document is hashed whole into source_hashes.query_sha256 and thence into the certificate's own
    digest, so an undeclared key was ignored semantically and honoured cryptographically: it moved
    a compiled answer's identity without being able to move the answer. 0.2 refuses it.

    A 0.1 label is still read. The published schema has declared additionalProperties: false since
    0.1, so a document carrying an undeclared key was never a valid 0.1 query -- the readers simply
    were not enforcing it -- and re-labelling the frozen reference pair would move the very digest
    three implementations agree on.
    """
    if query.get("schema_version") not in ACCEPTED_QUERY_SCHEMA_VERSIONS:
        raise ValueError("unsupported query schema")
    undeclared = [k for k in query if k not in QUERY_FIELD_PATHS]
    budgets = query.get("budgets")
    if isinstance(budgets, dict):
        undeclared += [f"budgets.{k}" for k in budgets if f"budgets.{k}" not in QUERY_FIELD_PATHS]
    if undeclared:
        raise ValueError(
            f"query carries undeclared field(s) {sorted(undeclared)}; "
            f"fiber-query/0.2 declares exactly {list(QUERY_FIELD_PATHS)}"
        )


def validate_world(world: dict[str, Any]) -> None:
    if world.get("schema_version") != "fiber-world/0.1":
        raise ValueError("unsupported world schema")
    fact_ids=[x["id"] for x in world["facts"]]
    factor_ids=[x["id"] for x in world["factors"]]
    if len(fact_ids) != len(set(fact_ids)):
        raise ValueError("duplicate fact id")
    if len(factor_ids) != len(set(factor_ids)):
        raise ValueError("duplicate factor id")
    provided={x["provides"] for x in world["facts"]}
    outputs={v for f in world["factors"] for v in f["outputs"]}
    for factor in world["factors"]:
        missing=[v for v in factor["inputs"] if v not in provided and v not in outputs]
        if missing:
            raise ValueError(f"factor {factor['id']} has unknown inputs {missing}")


def accessible_variables(world: dict[str, Any], decision_time: str) -> set[str]:
    cut=parse_time(decision_time)
    produced_by_event={v for e in world.get("events",[]) if parse_time(e["availability_time"]) <= cut for v in e.get("produces",[])}
    event_managed={v for e in world.get("events",[]) for v in e.get("produces",[])}
    all_vars={f["provides"] for f in world["facts"]}
    return (all_vars-event_managed) | produced_by_event


def backward_slice(world: dict[str, Any], targets: list[str]) -> tuple[set[str],set[str]]:
    producers: dict[str,list[dict[str,Any]]]={}
    for factor in world["factors"]:
        for out in factor["outputs"]:
            producers.setdefault(out,[]).append(factor)
    needed=set(targets); selected_factors:set[str]=set(); stack=list(targets)
    while stack:
        variable=stack.pop()
        for factor in producers.get(variable,[]):
            if factor["id"] in selected_factors:
                continue
            selected_factors.add(factor["id"])
            for inp in factor["inputs"]:
                if inp not in needed:
                    needed.add(inp);stack.append(inp)
    return needed,selected_factors


def protected_closure(world: dict[str, Any], tags: set[str]) -> set[str]:
    return {f["id"] for f in world["facts"] if tags.intersection(f.get("tags",[]))}


DATA_POLICY_VARIABLE="data_policy"
POLICY_SCOPE_DIMENSION="policy"
POLICY_REFINEMENT_ACTION="declare_the_required_policy_clauses_or_obtain_a_grant"


def _clause_set(raw: Any, subject: str) -> set[str]:
    if isinstance(raw,str) and raw:
        return {raw}
    if isinstance(raw,list):
        clauses=set()
        for item in raw:
            if not isinstance(item,str) or not item:
                raise ValueError(f"{subject} is not a list of non-empty clause names")
            clauses.add(item)
        return clauses
    raise ValueError(f"{subject} is not a list of non-empty clause names")


def governing_policy(world: dict[str, Any]) -> set[str] | None:
    """Clauses the corpus declares it was released under, or None when it declares none.

    The last fact providing the variable wins, matching the dict comprehension
    the rest of this file uses to index facts by variable.
    """
    declaring=[f for f in world["facts"] if f["provides"]==DATA_POLICY_VARIABLE]
    if not declaring:
        return None
    fact=declaring[-1]
    return _clause_set(fact["value"],f"fact {fact['id']} provides {DATA_POLICY_VARIABLE} but its value")


def policy_requirement(fact: dict[str, Any]) -> set[str]:
    """Clauses a reader must hold before this fact may be released.

    An array binding is read as a conjunction: a fact released under two
    restrictions requires both. Where the scope syntax is ambiguous the stricter
    reading is taken, per 43.33's invariant that requirements only tighten.
    """
    raw=fact.get("scope",{}).get(POLICY_SCOPE_DIMENSION)
    if raw is None:
        return set()
    uninterpretable=ValueError(f"fact {fact['id']} binds the {POLICY_SCOPE_DIMENSION!r} scope dimension to {json.dumps(raw)}, which names no policy clause")
    try:
        clauses=_clause_set(raw,"")
    except ValueError:
        raise uninterpretable from None
    if not clauses:
        raise uninterpretable
    return clauses


def resolve_policy(world: dict[str, Any], query: dict[str, Any]) -> tuple[set[str] | None, set[str]]:
    """The admission gate: runs before closure and slice, and needs no evidence."""
    governing=governing_policy(world)
    in_force=set(query.get("policy",[]))
    if governing is not None:
        ungranted=sorted(in_force-governing)
        if ungranted:
            raise ValueError(f"policy conflict: the query declares clause(s) {ungranted} that the world's data policy does not grant; the corpus was released under {sorted(governing)}")
    return governing,in_force


def policy_screen(candidates: dict[str,dict[str,Any]], in_force: set[str], protected: set[str]) -> dict[str,list[str]]:
    """Withheld candidates, and the clauses each was withheld for.

    Runs after the closure, not before it, so that a protected fact the caller
    cannot unlock is visible as a collision rather than pre-filtered out of the
    closure it was supposed to be inside.
    """
    withheld={}
    for fid in sorted(candidates):
        missing=sorted(policy_requirement(candidates[fid])-in_force)
        if missing:
            withheld[fid]=missing
    blocked=sorted(x for x in withheld if x in protected)
    if blocked:
        clauses=sorted({c for x in blocked for c in withheld[x]})
        raise ValueError(f"policy withholds protected fact(s) {blocked} for want of clause(s) {clauses}; the mandatory closure of 43.13 cannot be delivered, so no section is compiled")
    return withheld


def policy_obligation_detail(fact_id: str, missing: list[str]) -> str:
    return f"{fact_id} requires undeclared policy clauses: {', '.join(missing)}"


def evaluate_oracle(values: dict[str,Any]) -> dict[str,Any]:
    witnesses=[]
    aliases=values.get("subject_aliases",{})
    split=values.get("split_assignment",{})
    reverse={}
    for subject,names in aliases.items():
        for name in names:
            reverse.setdefault(name,[]).append(subject)
    for alias,subjects in sorted(reverse.items()):
        groups={split.get(s) for s in subjects}
        if len(subjects)>1 and len(groups)>1:
            witnesses.append({'type':'identity_leakage','alias':alias,'subjects':subjects,'splits':sorted(groups)})
    site=values.get("site_assignment",{})
    if site and split:
        by_split={}
        for subject,sp in split.items():
            by_split.setdefault(sp,set()).add(site.get(subject))
        clean={k:sorted(x for x in v if x is not None) for k,v in by_split.items()}
        if len(clean)>1 and all(len(v)==1 for v in clean.values()) and len({tuple(v) for v in clean.values()})>1:
            witnesses.append({'type':'site_leakage','site_by_split':clean})
    cut=values.get("training_decision_time")
    label_times=values.get("label_source_time",{})
    if cut:
        bad={s:t for s,t in label_times.items() if t>cut}
        if bad:
            witnesses.append({'type':'temporal_leakage','decision_time':cut,'future_label_sources':bad})
    if values.get("preprocess_fit_scope") == "all_subjects_before_split":
        witnesses.append({'type':'preprocessing_leakage','detail':'preprocessing fit used all subjects before split'})
    status='invalid' if witnesses else 'valid'
    return {'status':status,'witnesses':witnesses,'oracle_kind':'deterministic_split_integrity_v1'}


def compile_fiber(world: dict[str,Any], query: dict[str,Any]) -> CompileResult:
    validate_world(world)
    validate_query(query)
    governing,in_force=resolve_policy(world,query)
    facts_by_var={f["provides"]:f for f in world["facts"]}
    facts_by_id={f["id"]:f for f in world["facts"]}
    needed_vars,selected_factor_ids=backward_slice(world,query["targets"])
    protected_ids=protected_closure(world,set(query.get("protected_tags",[])))
    selected_fact_ids={facts_by_var[v]["id"] for v in needed_vars if v in facts_by_var} | protected_ids
    withheld_by_policy=policy_screen({x:facts_by_id[x] for x in selected_fact_ids},in_force,protected_ids)
    selected_fact_ids-=set(withheld_by_policy)
    accessible=accessible_variables(world,query["decision_time"])
    inaccessible=[fid for fid in selected_fact_ids if facts_by_id[fid]["provides"] not in accessible]
    selected_fact_ids={fid for fid in selected_fact_ids if facts_by_id[fid]["provides"] in accessible}
    if len(selected_fact_ids)>query["budgets"]["max_facts"]:
        raise RuntimeError(f"protected/sliced facts exceed max_facts: {len(selected_fact_ids)}")
    selected_facts=[facts_by_id[x] for x in sorted(selected_fact_ids)]
    values={f["provides"]:f["value"] for f in selected_facts}
    oracle=evaluate_oracle(values)
    factor_by_id={f["id"]:f for f in world["factors"]}
    max_arity=max((len(factor_by_id[x]["inputs"]) for x in selected_factor_ids),default=0)
    omitted_facts=sorted(set(facts_by_id)-selected_fact_ids)
    omitted_exploratory=sum('exploratory' in facts_by_id[x].get('tags',[]) for x in omitted_facts)
    section={
      'schema_version':'fiber-decision-section/0.1',
      'world_id':world['world_id'],'query_id':query['query_id'],'decision_time':query['decision_time'],
      'goal':'Determine whether the proposed radiogenomic split supports a valid external-generalization analysis.',
      'selected_evidence':[{'id':f['id'],'provides':f['provides'],'value':f['value'],'scope':f['scope'],'tags':f.get('tags',[]),'provenance':f.get('provenance',[])} for f in selected_facts],
      'selected_factors':[factor_by_id[x] for x in sorted(selected_factor_ids)],
      'oracle':oracle,
      'unresolved_obligations':[{'type':'policy_blocked','detail':policy_obligation_detail(x,withheld_by_policy[x])} for x in sorted(withheld_by_policy)]
                               +[{'type':'inaccessible_at_cut','fact_id':x} for x in sorted(inaccessible)],
      'refinement_frontier':([{'action':POLICY_REFINEMENT_ACTION,'facts':sorted(withheld_by_policy)}] if withheld_by_policy else [])
                           +([{'action':'advance_time_cut_or_use_retrospective_mode','facts':sorted(inaccessible)}] if inaccessible else [])
    }
    cert_base={
      'schema_version':'fiber-context-certificate/0.1','world_id':world['world_id'],'query_id':query['query_id'],
      'selected_facts':sorted(selected_fact_ids),'selected_factors':sorted(selected_factor_ids),
      'protected_closure':sorted(protected_ids),
      'omissions':{'total_facts':len(omitted_facts),'exploratory_facts':omitted_exploratory,'classification':'no_backward_dependency_path_or_temporally_inaccessible','inaccessible_selected_before_cut':sorted(inaccessible)},
      'plan':{'backend':'backward_factor_slice_reference','compiled_factor_count':len(selected_factor_ids),'compiled_fact_count':len(selected_fact_ids),'total_factor_count':len(world['factors']),'total_fact_count':len(world['facts']),'max_selected_factor_arity':max_arity,'fallback':None},
      'oracle':oracle,
      'source_hashes':{'world_sha256':sha(world),'query_sha256':sha(query),'decision_section_sha256':sha(section)},
      'limitations':['Reference slicer uses dependency reachability and protected tags; it does not yet implement sheaf cohomology, FAQ-width optimization, abstract interpretation, or formal influence bounds.']
    }
    cert=dict(cert_base);cert['certificate_sha256']=sha(cert_base)
    return CompileResult(section,cert)


def main() -> None:
    ap=argparse.ArgumentParser()
    ap.add_argument('--world',required=True);ap.add_argument('--query',required=True)
    ap.add_argument('--output',required=True);ap.add_argument('--section-output')
    args=ap.parse_args()
    world=json.loads(Path(args.world).read_text());query=json.loads(Path(args.query).read_text())
    result=compile_fiber(world,query)
    Path(args.output).write_text(json.dumps(result.certificate,indent=2,sort_keys=True)+'\n')
    if args.section_output:
        Path(args.section_output).write_text(json.dumps(result.decision_section,indent=2,sort_keys=True)+'\n')
    print(json.dumps({'status':result.certificate['oracle']['status'],'selected_facts':len(result.certificate['selected_facts']),'selected_factors':len(result.certificate['selected_factors']),'omitted_facts':result.certificate['omissions']['total_facts'],'certificate_sha256':result.certificate['certificate_sha256']},indent=2))

if __name__=='__main__': main()
