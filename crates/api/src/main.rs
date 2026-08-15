//! `bioprism-api` — bounded HTTP/REST and event gateway.
//!
//! Usage: `bioprism-api [--bind <host:port>] [--root <dir>] [--token <bearer-token>]`

use bioprism_api::{serve, ApiConfig, ApiRouter};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;

fn main() {
    let mut arguments = std::env::args().skip(1);
    let mut bind = "127.0.0.1:8787".to_string();
    let mut root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut token = None;
    let mut max_body_bytes = ApiConfig::default().max_body_bytes;
    let mut event_capacity = ApiConfig::default().event_capacity;

    while let Some(argument) = arguments.next() {
        let value = |name: &str, arguments: &mut std::iter::Skip<std::env::Args>| {
            arguments.next().unwrap_or_else(|| {
                eprintln!("{name} requires a value");
                std::process::exit(2);
            })
        };
        match argument.as_str() {
            "--bind" => bind = value("--bind", &mut arguments),
            "--root" => root = PathBuf::from(value("--root", &mut arguments)),
            "--token" => token = Some(value("--token", &mut arguments)),
            "--max-body-bytes" => {
                max_body_bytes = value("--max-body-bytes", &mut arguments)
                    .parse()
                    .unwrap_or_else(|_| {
                        eprintln!("--max-body-bytes must be an unsigned integer");
                        std::process::exit(2);
                    });
            }
            "--event-capacity" => {
                event_capacity = value("--event-capacity", &mut arguments)
                    .parse()
                    .unwrap_or_else(|_| {
                        eprintln!("--event-capacity must be an unsigned integer");
                        std::process::exit(2);
                    });
            }
            "-h" | "--help" => {
                println!(
                    "bioprism-api — bounded HTTP/REST and event gateway\n\n\
                     USAGE\n  bioprism-api [--bind <host:port>] [--root <dir>] [--token <bearer-token>]\n\n\
                     GET /healthz and /readyz are public. Other /v1 routes require --token when configured.\n\
                     REST tools: POST /v1/tools/<name> with a JSON object body.\n\
                     JSON-RPC: POST /v1/rpc. Events: GET /v1/events or /v1/events/stream.\n\
                     Webhooks: register, poll signed deliveries, retry, and acknowledge.\n\
                     The gateway does not terminate TLS, speak gRPC, or send arbitrary outbound requests."
                );
                return;
            }
            other => {
                eprintln!("unrecognised argument {other:?}; use --help");
                std::process::exit(2);
            }
        }
    }

    if !root.is_dir() {
        eprintln!("root is not a directory: {}", root.display());
        std::process::exit(2);
    }
    let config = ApiConfig {
        max_body_bytes,
        event_capacity,
        bearer_token: token,
        ..ApiConfig::default()
    };
    let router = match ApiRouter::new(root, config) {
        Ok(router) => Arc::new(router),
        Err(error) => {
            eprintln!("invalid API configuration: {error}");
            std::process::exit(2);
        }
    };
    let listener = TcpListener::bind(&bind).unwrap_or_else(|error| {
        eprintln!("cannot bind {bind}: {error}");
        std::process::exit(1);
    });
    eprintln!("bioprism-api listening on {bind}");
    if let Err(error) = serve(listener, router) {
        eprintln!("bioprism-api stopped: {error}");
        std::process::exit(1);
    }
}
