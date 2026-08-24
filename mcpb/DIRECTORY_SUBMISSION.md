# Desktop Extensions directory — submission answer sheet

Form: https://clau.de/desktop-extention-submission (Google sign-in required).
Every prerequisite is now in place; paste from here.

| Field (typical) | Answer |
|---|---|
| Extension name | AURORA Agent |
| Short description | FIBER decision-context compiler with honest omission accounting: 259 evidence-bearing MCP tools. |
| Download URL (.mcpb) | https://github.com/MurariAmbati/aurora-agent-releases/releases/download/v0.1.1/aurora-agent.mcpb |
| SHA-256 | see the v0.1.1 release notes (recompute: `openssl dgst -sha256 aurora-agent.mcpb`) |
| Source repository (open source) | https://github.com/AURORA-NEURO/aurora-agent |
| License | Apache-2.0 (LICENSE at repo root) |
| Privacy policy URL | https://github.com/AURORA-NEURO/aurora-agent/blob/main/PRIVACY.md |
| Documentation | https://github.com/AURORA-NEURO/aurora-agent/blob/main/README.md |
| Support / issues | https://github.com/AURORA-NEURO/aurora-agent/issues |
| Platforms tested | Windows (win32). No macOS build yet — declared in `compatibility.platforms`; state this plainly in the form. |
| Author / contact | AURORA-NEURO / the submitter's email |
| MCP registry listing | io.github.MurariAmbati/aurora-agent (registry.modelcontextprotocol.io) |

Longer description (if asked): use `long_description` from `mcpb/manifest.json`
verbatim — it already carries the research-boundary disclaimer.

Prerequisite status:
- Open source: DONE — source is public at AURORA-NEURO/aurora-agent (Apache-2.0).
- Privacy policy in three places: DONE — README section, `privacy_policies`
  in manifest.json (v0.1.1 bundle), hosted PRIVACY.md.
- Windows testing: DONE (packed server handshake + fiber_compile verified).
- macOS testing: NOT done — no darwin build. Do not claim it. A future
  universal bundle needs a darwin `bioprism-mcp` via
  `server.mcp_config.platform_overrides`.

After submitting: Anthropic reviews manually (no published timeline; assume
weeks). Approved extensions appear in Claude Desktop → Settings → Extensions →
directory browsing. Escalation contact per docs: mcp-review@anthropic.com.
