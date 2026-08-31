# Go control-plane parity slice

`ports/go-control-plane` is a deliberately bounded Go 1.21+ port of the
deterministic primitives used by Aurora's Rust agent fabric and Python scale
layer. It is intended as a future optimization and embedding surface, not as
a claim that the entire Aurora runtime has been converted to Go.

## Included

- typed nonzero agent, task, shard, and lease-epoch identifiers;
- SplitMix64/rendezvous placement and the Python SHA-256 placement rule, kept
  as separate APIs because the upstream layers intentionally differ;
- epoch leases with stale-handle refusal and expiry reporting;
- bounded FIFO queues with explicit backpressure and high-water telemetry;
- settlement-closed idempotency windows;
- deterministic adapter descriptors, including 1,000+ descriptor-only platform
  entries without claiming live connectivity;
- JSON telemetry with stable field order;
- streaming synthetic manifest metadata and a five-million-line-equivalent
  benchmark model that creates no files and retains only bounded chunk state;
- parity vectors generated from executed Rust/Python reference code.

## Deliberate boundary

This slice does not implement network transports, MCP/HTTP/A2A/ACP clients,
authentication, OAuth, persistence, distributed leases, a scheduler, OS
process spawning, or provider-specific Hermes/Codex/Claude behavior. Those
surfaces remain in their authoritative integrations until a separately tested
port is justified. Logical agents and descriptors are not live workers or
platform connections.

## Verification gate

The host currently has no `go` executable, so `gofmt`, `go test`, `go vet`, and
the benchmark have not been run here. This is recorded as an unavailable gate,
not a passing claim. Once Go 1.21+ is installed, run from this directory:

```text
gofmt -w *.go
go test ./...
go vet ./...
go test -run '^$' -bench BenchmarkSyntheticManifestMetadata5MLines -benchmem ./...
```

The Rust agent-fabric and Python scale layers remain the semantic authorities;
`vectors/parity.json` makes the cross-language intent reviewable and testable
when the Go toolchain is available.
