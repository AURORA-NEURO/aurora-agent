//! `bioprism-api` — bounded HTTP/REST and event gateway.
//!
//! Usage: `bioprism-api [--bind <host:port>] [--root <dir>] [--token <bearer-token>] [--mission-state <file>] [--mission-queue-state <file>] [--event-state <file>] [--evidence-state <file>] [--reconciliation-state <file>]`

use bioprism_api::{serve, ApiConfig, ApiRouter};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;

fn main() {
    let mut arguments = std::env::args().skip(1);
    let mut bind = "127.0.0.1:8787".to_string();
    let mut root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut token = None;
    let mut mission_state_path = None;
    let mut mission_queue_state_path = None;
    let mut mission_queue_max_jobs = ApiConfig::default().mission_queue_max_jobs;
    let mut mission_queue_max_active_leases =
        ApiConfig::default().mission_queue_max_active_leases;
    let mut event_state_path = None;
    let mut evidence_state_path = None;
    let mut reconciliation_state_path = None;
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
            "--mission-state" => {
                mission_state_path = Some(PathBuf::from(value("--mission-state", &mut arguments)))
            }
            "--mission-queue-state" => {
                mission_queue_state_path =
                    Some(PathBuf::from(value("--mission-queue-state", &mut arguments)))
            }
            "--mission-queue-max-jobs" => {
                mission_queue_max_jobs = value("--mission-queue-max-jobs", &mut arguments)
                    .parse()
                    .unwrap_or_else(|_| {
                        eprintln!("--mission-queue-max-jobs must be an unsigned integer");
                        std::process::exit(2);
                    });
            }
            "--mission-queue-max-active-leases" => {
                mission_queue_max_active_leases =
                    value("--mission-queue-max-active-leases", &mut arguments)
                        .parse()
                        .unwrap_or_else(|_| {
                            eprintln!(
                                "--mission-queue-max-active-leases must be an unsigned integer"
                            );
                            std::process::exit(2);
                        });
            }
            "--event-state" => {
                event_state_path = Some(PathBuf::from(value("--event-state", &mut arguments)))
            }
            "--evidence-state" => {
                evidence_state_path = Some(PathBuf::from(value("--evidence-state", &mut arguments)))
            }
            "--reconciliation-state" => {
                reconciliation_state_path =
                    Some(PathBuf::from(value("--reconciliation-state", &mut arguments)))
            }
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
                     USAGE\n  bioprism-api [--bind <host:port>] [--root <dir>] [--token <bearer-token>] [--mission-state <file>] [--mission-queue-state <file>] [--mission-queue-max-jobs <n>] [--mission-queue-max-active-leases <n>] [--event-state <file>] [--evidence-state <file>] [--reconciliation-state <file>]\n\n\
                     GET /healthz and /readyz are public. Other /v1 routes require --token when configured.\n\
                     REST tools: POST /v1/tools/<name> with a JSON object body.\n\
                     JSON-RPC: POST /v1/rpc. Events: GET /v1/events or /v1/events/stream.\n\
                     Missions: POST /v1/missions; --mission-state enables bounded restart-aware snapshots; --mission-queue-state enables the typed factory execution ledger; queue max flags provide explicit backpressure.\n\
                     Events: --event-state enables bounded event, subscription, and signed outbox checkpoints; secrets remain process-local.\n\
                     Evidence: --evidence-state enables bounded restart-safe imports of independently verified mission bundles.\n\
                     Reconciliation: --reconciliation-state enables bounded restart-safe imports of digest-valid workflow reconciliation reports.\n\
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
        mission_state_path,
        mission_queue_state_path,
        mission_queue_max_jobs,
        mission_queue_max_active_leases,
        event_state_path,
        evidence_state_path,
        reconciliation_state_path,
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
