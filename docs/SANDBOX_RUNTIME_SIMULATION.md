# Sandbox runtime simulation

`sandbox_runtime_simulate` is the process-side companion to [`sandbox_admission_audit`](SANDBOX_ADMISSION_AUDIT.md). It consumes one admission declaration, selects one declared profile, and evaluates a bounded ordered request trace. The route is useful for testing the decision contract that an external launcher would need to enforce, while remaining strictly a simulation: it never starts a process, executes code, mounts a path, opens a socket, reads a secret, changes a credential, or applies a kernel policy.

## Wire contract

The request is a `bioprism-sandbox-runtime/0.1` manifest:

- `admission` is the complete `SandboxManifest` that must pass the admission audit by default;
- `profile` selects one declared execution profile;
- `requests` is an ordered list capped at 4,096 rows, each naming an id, capability kind, exact target, and positive CPU, memory, wall-time, process, and output charges;
- `policies.stop_on_refusal` defaults to `true`, preserving a fail-closed suffix;
- `policies.require_admission` defaults to `true`, so an invalid admission cannot become runtime readiness; and
- `policies.max_requests` cannot exceed 4,096.

The audit returns both the admission digest and a separate trace digest. Each step contains:

- the exact capability id, when one was found;
- independent `capability_valid`, `target_valid`, and `resource_valid` booleans;
- `simulated`, `refused`, or `not_run` decision state;
- whether the request was charged; and
- cumulative usage after the step plus a stable refusal code when applicable.

## Decision order

For each request, the simulator performs these checks in order:

1. request identity and target are bounded;
2. one capability belonging to the selected profile has the exact requested kind and target and is explicitly allowed;
3. filesystem targets remain private paths and network targets remain finite, exact destinations;
4. the request's memory and process peaks fit the profile ceilings; and
5. cumulative CPU, wall-time, and output charges fit the profile ceilings.

A successful row is charged and becomes `simulated`. A failed row becomes `refused` and is never charged. With `stop_on_refusal`, every later bounded row is emitted as `not_run` with `stopped_on_refusal`; with that policy disabled, later rows are evaluated independently, but the overall trace remains invalid if any refusal occurred. A trace is runtime-ready only when the admission is valid, every request was simulated, all requests were within the bound, and no blocking issue remains.

## Guarantees and limitations

The contract guarantees deterministic admission-before-simulation, exact capability/target matching, explicit resource accounting, refusal preservation, and content-addressed replay identity. It does not guarantee that a process would be safe on a host. Enforcement remains an external responsibility for the launcher/container/runtime integration: namespaces, syscall filters, cgroups, credential and secret isolation, filesystem resolution, network policy, quarantine storage, output review, and operator response are not exercised here. A valid trace is therefore a decision artifact, not runtime enforcement evidence.

The Rust module is `bioprism-devplat::sandbox_runtime`; the MCP route and typed Python/TypeScript facades preserve the same schema. See [PYTHON_SDK.md](PYTHON_SDK.md) and [TYPESCRIPT_SDK.md](TYPESCRIPT_SDK.md).
