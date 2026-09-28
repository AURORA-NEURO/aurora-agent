# Privacy Policy — AURORA Agent (bioprism)

Effective: 2026-09-28. Applies to the `bioprism` binaries (`bioprism`,
`bioprism-mcp`, `bioprism-api`) and the packaged Claude Desktop extension
(`aurora-agent.mcpb`).

## Summary

AURORA Agent is local-first software. It has no telemetry, analytics, crash
reporting, account, or background network activity. Local workflows can run
offline. The source retrieval connector can make a network request only when an
operator explicitly allows an exact HTTP or HTTPS origin at startup and a caller submits
a retained plan that separately opts into networking and allows that host.
Outbound source retrieval is denied by default. HTTP and HTTPS require exact
operator-approved origins; HTTPS validates the server certificate against
platform trust roots and checks the hostname. Redirects are refused.

## Details

- **Data processed**: the server processes the tool arguments sent by its MCP
  client and returns results to that client. Local file reads and writes are
  confined to the configured data root (`--root`); absolute paths, `..`
  traversal, and symlink escapes are refused. The API gateway can persist
  registries only when an operator configures the corresponding state paths.
- **Network**: no background requests, telemetry, or conversation-history
transfer occurs. `domain_evidence_source_execute` can send an HTTP or HTTPS GET only
  after both gates pass: the operator starts `bioprism-mcp` or `bioprism-api`
  with one or more `--allow-http-origin <host[:port]>` or
  `--allow-https-origin <host[:port]>` options, and the retained
  plan sets `retrieval_policy.network` to `enabled` and includes the requested
  host in `retrieval_policy.allowed_hosts`. The default operator allow-list is
  empty. The request contains the locator's path and query; the remote host and
  network intermediaries can observe the request, the server's network address,
  and ordinary connection metadata. Do not place secrets or personal data in a
  locator. HTTP requests are unencrypted; HTTPS uses verified TLS. The path and
  query are sent to the explicitly approved source origin.
  Credentials are not accepted by this connector. Returned content is bounded
  by the plan and is passed back to the caller; it may also be retained by the
  server's evidence registries, and the API gateway may persist it if configured.
- **Conversation data**: the server sees only the tool arguments the MCP client
  sends it. It does not read, store, or transmit conversation history unless a
  caller explicitly places data in a tool argument or source locator.
- **Third parties**: the project does not send data to service operators. When
  an operator enables and a caller uses remote HTTP retrieval, the selected
  source host receives the request described above and applies its own privacy
  policy.

## Boundary

Research and developer infrastructure. It does not diagnose an individual,
recommend treatment, triage care, enroll participants, or claim
medical-device functionality.

## Contact

Issues: https://github.com/AURORA-NEURO/aurora-agent/issues
