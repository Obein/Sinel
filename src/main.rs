//! Main entrypoint for Signal TLS Proxy on Fly.io.

mod proxy;
mod sni;
mod web;
mod whitelist;

use log::info;
use std::env;

/// Loads key-value pairs from an optional `.env` file in the working directory
/// into the current process environment without overriding already-set variables.
fn load_env_file() {
    if let Ok(content) = std::fs::read_to_string(".env") {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if let Some((key, val)) = trimmed.split_once('=') {
                let key = key.trim();
                let mut val = val.trim();
                if (val.starts_with('"') && val.ends_with('"')) || (val.starts_with('\'') && val.ends_with('\'')) {
                    val = &val[1..val.len() - 1];
                }
                if env::var(key).is_err() {
                    // SAFETY: Called synchronously at the very beginning of main() before any threads or runtimes are started.
                    unsafe {
                        env::set_var(key, val);
                    }
                }
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Attempt to load .env file if present (useful for local development and Docker)
    load_env_file();

    // Initialize env_logger (default to sinel=info,warn if RUST_LOG is unset)
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("sinel=info,warn")
    ).init();

    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let bind_addr = format!("{}:{}", host, port);

    info!("Starting Signal TLS Proxy for Fly.io...");
    info!("Listen address: {}", bind_addr);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        if let Err(e) = proxy::run_server(&bind_addr).await {
            log::error!("Server exited with fatal error: {e}");
        }
    });

    Ok(())
}
