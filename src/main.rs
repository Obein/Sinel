//! Main entrypoint for Signal TLS Proxy on Fly.io.

mod proxy;
mod sni;
mod web;
mod whitelist;

use log::info;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
