# Agent-fabric integration contract

`integrations/agent-fabric` is a detached, dependency-free Rust crate for bounded coordination of
logical agents. It is deliberately separate from the workspace `crates/fabric`: that crate is the
blueprint composition algebra, while this crate owns dispatch, leases, queues, retries, quotas,
idempotency, receipts, and fault simulation.

## Supported surfaces

- The scheduler is deterministic and in-process. Logical agent count is not a thread count; the
  driver owns the real concurrency bound. Each shard queue has its own capacity, and
  `max_pending_tasks` bounds all accepted work across queues, retries, and execution.
- MCP stdio uses newline-delimited JSON-RPC 2.0 and implements `initialize`, `ping`, `tools/list`,
  and `tools/call` for `fabric.submit` and `fabric.cancel`. Oversized or malformed frames fail
  before dispatch.
- The HTTP adapter accepts one HTTP/1.1 request at a time. `POST /mcp` routes to the same MCP
  server, `GET /health` is a liveness response, and responses always use `Content-Length`.
- `Fabric::checkpoint` exports versioned scheduler state and `Fabric::from_checkpoint` restores it
  without dispatching. Capture fails while any attempt still owns a lease.
- A2A conversion is an explicitly labelled wire-shape profile. It preserves task id,
  capabilities, payload hex, and the payload digest, but it is not A2A discovery, authentication,
  remote execution, streaming, or a distributed task service.
- ACP returns a typed `acp_not_implemented` refusal. It must not be advertised as ACP support.

## Invariants

1. Task payloads are digest-bound at composition and checked before execution and at settlement.
2. When a caller omits an idempotency key, the fabric derives one from the full payload and the
   complete normalized capability set using a versioned, length-delimited encoding. A caller key
   takes precedence; the automatic key is a 128-bit SHA-256 prefix, so its uniqueness is
   probabilistic rather than an absolute guarantee.
3. A task has at most one live lease. Lease epochs reject stale completions and releases.
4. Every ready queue, driver hand-off channel, retry heap, receipt ledger, and aggregate pending
   task set is bounded by an explicit configuration or retention limit. The pending-task limit
   prevents unroutable or deferred tasks from escaping the per-shard queue bounds into unbounded
   retry state; backpressure identifies whether a shard queue or the aggregate limit was reached.
5. Cancellation is cooperative and monotone; a receipt records that cancellation raced execution
   rather than rewriting the observed outcome.
6. The simulator uses virtual time and seeded fault injection so crash, silent-drop, failed,
   corrupt-result, retry, and expiry paths are replayable.

## Deliberate limitations

There is no automatic durable store or write-ahead journal. The caller must store checkpoints
atomically and protect them because they contain task payloads. A checkpoint checksum detects
accidental corruption, not forgery, and checkpoints do not capture a live attempt or resolve an
uncertain external effect. Restoring can therefore replay work that ran after the last checkpoint;
handlers with external effects need caller-enforced idempotency or reconciliation. Cross-process
distribution, TLS termination, HTTP/2, chunked transfer, async runtime, authentication,
authorization boundaries, and executable A2A/ACP support are also outside this crate. Quotas are
admission control, not security. A production deployment must put the adapter behind an
authenticated transport and add a durable, versioned coordination layer before claiming
multi-host semantics.

## Verification

From the repository root:

```text
cargo fmt --manifest-path integrations/agent-fabric/Cargo.toml -- --check
cargo test --manifest-path integrations/agent-fabric/Cargo.toml --offline
cargo clippy --manifest-path integrations/agent-fabric/Cargo.toml --all-targets --offline -- -D warnings
```

The crate intentionally has an empty `[workspace]` table and no external dependencies, so these
gates do not change or resolve the parent Aurora workspace.
