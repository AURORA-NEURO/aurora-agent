# Getting aurora-agent onto the connector surfaces

State as of 2026-08. The bundle `mcpb/dist/aurora-agent.mcpb` (14.8 MB, built by
`python mcpb/build_mcpb.py`) is a working Claude Desktop extension: the packed
server completes the MCP handshake and reproduces the reference verdict and
parity digest (`c0da17ff…`) against its bundled fixtures. Three listing
surfaces exist; they have different gates.

## Already true, with no listing at all

A `.mcpb` file installs into Claude Desktop with a double-click (Settings →
Extensions also accepts it). Distributing `aurora-agent.mcpb` directly — a
release asset, a website link — requires no review or approval from anyone.

## 1. Official MCP registry (registry.modelcontextprotocol.io) — instant, minimal exposure

Metadata-only registry, automated validation, no human review. The listed
artifact must be publicly downloadable; **the source can stay private** — only
the compiled `.mcpb` becomes public.

Steps (owner actions marked ►):

1. ► Create a public release location (a minimal public releases-only repo
   works, e.g. `MurariAmbati/aurora-agent-releases`), upload
   `aurora-agent.mcpb` as a GitHub release asset. The download URL must
   contain "mcp" — the `.mcpb` extension satisfies this.
2. Fill the URL into `mcpb/server.json` (`identifier`); the `fileSha256` is
   already the real hash of the built bundle — recompute after any repack
   (`python -c "import hashlib;print(hashlib.sha256(open('mcpb/dist/aurora-agent.mcpb','rb').read()).hexdigest())"`).
3. ► `mcp-publisher login github` (device-code OAuth) then
   `mcp-publisher publish --file mcpb/server.json`. GitHub auth pins the
   namespace `io.github.MurariAmbati/*`; publishing under `com.<domain>/*`
   instead needs DNS/HTTP domain auth.
4. Verify: `curl "https://registry.modelcontextprotocol.io/v0.1/servers?search=aurora-agent"`.

Caveats: the registry is preview-status, and Claude's own "Browse connectors"
UI does **not** pull from it — this buys ecosystem discoverability (GitHub MCP
registry, VS Code, aggregators), not placement in Claude apps.

## 2. Claude Desktop Extensions directory — the right surface for this server, with conditions

The one Anthropic surface designed for local binaries. Submission is a Google
Form (open to anyone, no paid plan): https://clau.de/desktop-extention-submission

Non-negotiable gates from the published review criteria:

- **Open source**: the Software Directory Terms' MCPB open-source clause is
  explicitly "required and not waivable". Listing here means opening the
  source (the whole repo, or a carve-out that genuinely builds the server).
- **Privacy policy in three places**: a "Privacy Policy" section in README, a
  `privacy_policies` array of HTTPS URLs in `manifest.json`, and the hosted
  policy itself. (The server touches no external services — say exactly that.)
- **Windows AND macOS testing**: the current bundle is `win32`-only. A macOS
  listing needs a darwin build of `bioprism-mcp` wired via
  `server.mcp_config.platform_overrides`. A win32-only submission is
  possible but a weaker candidate.
- Review is manual with no published timeline; assume weeks.

## 3. Connectors Directory remote portal — not eligible as-is

`https://claude.ai/admin-settings/directory/submissions/new` accepts **remote**
servers only (public HTTPS, streamable HTTP/SSE, OAuth 2.0 for authed
services, per-tool annotations, privacy policy, reviewer test accounts) and
requires a **Team/Enterprise** Claude org. A local stdio binary is
categorically ineligible. The existing `bioprism-api` HTTP gateway is NOT this
either (bearer token, loopback, no OAuth, no TLS — by design). Eligibility
would mean standing up a hardened public deployment: reverse proxy + TLS +
OAuth in front of the gateway, plus the org plan. Possible engineering, but a
product decision, not a packaging step.

## Recommendation

Do 1 now (instant, source stays private), decide on 2 when ready to open the
source, treat 3 as a separate hosted-product decision. Whatever the listing
state, the `.mcpb` already works for anyone you hand it to.
