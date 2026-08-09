#!/usr/bin/env python3
from __future__ import annotations
from pathlib import Path
from collections import defaultdict, deque
import json
from fiber_compile import compile_fiber

HERE=Path(__file__).resolve().parent
world=json.loads((HERE/'examples/radiogenomic_world.json').read_text())
query=json.loads((HERE/'examples/leakage_query.json').read_text())

facts={f['id']:f for f in world['facts']}
var_fact={f['provides']:f['id'] for f in world['facts']}
factors={f['id']:f for f in world['factors']}

# Undirected incidence projection, a fair connected-structure stress baseline.
adj=defaultdict(set)
for factor in world['factors']:
    fn='factor:'+factor['id']
    for var in factor['inputs']+factor['outputs']:
        vn='var:'+var;adj[fn].add(vn);adj[vn].add(fn)
    for var in factor['inputs']:
        if var in var_fact:
            factn='fact:'+var_fact[var];vn='var:'+var
            adj[factn].add(vn);adj[vn].add(factn)

def reachable(starts,max_depth=None):
    q=deque((s,0) for s in starts);seen=set(starts)
    while q:
        x,d=q.popleft()
        if max_depth is not None and d>=max_depth: continue
        for y in adj[x]:
            if y not in seen: seen.add(y);q.append((y,d+1))
    return seen

starts=['var:'+x for x in query['targets']]
full=len(world['facts'])
graph_nodes=reachable(starts,max_depth=7)
graph_facts={x[5:] for x in graph_nodes if x.startswith('fact:')}
hyper_nodes=reachable(starts,None)
hyper_facts={x[5:] for x in hyper_nodes if x.startswith('fact:')}
result=compile_fiber(world,query).certificate
fiber=len(result['selected_facts'])
rows=[('Full-context facts',full),('Graph 7-hop facts',len(graph_facts)),('Connected hypergraph/incidence facts',len(hyper_facts)),('FIBER compiled facts',fiber)]
print('| Baseline | Facts exposed |')
print('|---|---:|')
for name,value in rows: print(f'| {name} | {value} |')
print()
print(f'FIBER/full ratio: {fiber/full:.4f}')
print(f'Oracle status: {result["oracle"]["status"]}')
print('Witness types:',', '.join(x['type'] for x in result['oracle']['witnesses']))
print('\nImportant: this toy is constructed to expose hub expansion. It demonstrates compiler mechanics, not universal superiority.')
