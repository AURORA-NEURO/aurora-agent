//! Model Context Protocol server for the FIBER context compiler.
//!
//! Implements blueprint 11.10–11.11 and the MCP half of 43.35 (schemas, APIs and the Cognitive ABI).
//! This is the adoption wedge: an agent in any framework can compile a decision context, descend
//! only as far into the evidence as it needs, and verify a certificate — without linking the
//! engine or learning a new SDK.

// The protocol catalogue is intentionally explicit and now includes the six-lane neurosurgical
// workbench. Keep macro expansion headroom proportional to that versioned schema surface.
#![recursion_limit = "512"]
//!
//! # Not implemented, deliberately
//!
//! * **One transport.** [`serve`] speaks newline-delimited JSON-RPC over a reader and a writer,
//!   which in practice is stdio. There is no HTTP, SSE or WebSocket binding here.
//! * **Six request methods and one notification's worth of protocol.** `initialize`, `ping`,
//!   `tools/list`, `tools/call`, `resources/list` and `resources/read` are answered;
//!   `notifications/initialized` advances the lifecycle and every other notification is dropped
//!   silently. Prompts, sampling, roots, completion and progress are absent, not stubbed.
//! * **No cancellation.** `notifications/cancelled` is one of the notifications dropped, so an
//!   in-flight tool call always runs to completion. The mission executor's own cancellation flag
//!   is internal and is not reachable from the protocol.
//! * **No sessions, no authentication, no per-caller isolation.** A server is one root and one
//!   lifecycle, served to whoever holds the pipe. Confinement to that root
//!   ([`Server::resolve`]) is the only access control in the crate.
//! * **No concurrency.** The event loop reads, answers, and only then reads again. Tool
//!   dispatch runs on its own thread for stack headroom, never for parallelism.
//! * **No state of its own.** Nothing persists across a process except what a tool was
//!   explicitly asked to write to an explicitly named path inside the root.

mod brain_control;
mod research_campaign;
pub mod rpc;
pub mod server;

pub use rpc::{Request, Response};
pub use server::{
    resource_definitions, tool_definitions, workspace_capabilities, Lifecycle, Server,
    ADAPTIVE_QUERY_SCHEMA_URI, CAPABILITIES_URI, CERTIFICATE_SCHEMA_URI,
    MISSION_TRACE_SCHEMA_VERSION, PROTOCOL_VERSION, QUERY_SCHEMA_URI, SERVER_NAME,
    WORLD_SCHEMA_URI,
};

use std::io::{BufRead, Write};

/// Runs the stdio event loop until the input stream closes.
pub fn serve<R: BufRead, W: Write>(
    server: &mut Server,
    input: R,
    output: &mut W,
) -> std::io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let response = match Request::parse(&line) {
            Ok(request) => server.handle(&request),
            Err(failure) => Some(*failure),
        };

        if let Some(response) = response {
            writeln!(output, "{}", response.to_json())?;
            output.flush()?;
        }
    }
    Ok(())
}
