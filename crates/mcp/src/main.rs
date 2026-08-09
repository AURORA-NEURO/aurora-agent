//! `bioprism-mcp` — stdio MCP server.
//!
//! Usage: `bioprism-mcp [--root <dir>]`
//!
//! The root defaults to the working directory and confines every path the server will read or
//! write. stdout carries JSON-RPC only; audit records and diagnostics go to stderr.

use bioprism_mcp::{serve, Server};
use std::io::{self, BufReader};
use std::path::PathBuf;

fn main() {
    let mut arguments = std::env::args().skip(1);
    let mut root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--root" => match arguments.next() {
                Some(value) => root = PathBuf::from(value),
                None => {
                    eprintln!("--root requires a directory");
                    std::process::exit(2);
                }
            },
            "-h" | "--help" => {
                println!(
                    "bioprism-mcp — Model Context Protocol server for the FIBER context compiler\n\n\
                     USAGE\n  bioprism-mcp [--root <dir>]\n\n\
                     Speaks JSON-RPC 2.0 over newline-delimited stdio. Every path an agent supplies \n\
                     is resolved inside --root; absolute paths and traversal outside it are refused.\n\n\
                     Tools: fiber_compile, fiber_refine, fiber_explain, fiber_verify, world_index"
                );
                return;
            }
            other => {
                eprintln!("unrecognised argument {other:?}");
                std::process::exit(2);
            }
        }
    }

    if !root.is_dir() {
        eprintln!("root is not a directory: {}", root.display());
        std::process::exit(2);
    }

    let mut server = Server::new(root);
    let stdin = BufReader::new(io::stdin());
    let mut stdout = io::stdout();

    if let Err(error) = serve(&mut server, stdin, &mut stdout) {
        eprintln!("server error: {error}");
        std::process::exit(1);
    }
}
