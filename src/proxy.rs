//! Asynchronous TCP proxy engine for Signal TLS traffic.
//!
//! Handles incoming decrypted TCP connections from Fly.io, extracts the inner SNI,
//! applies whitelist filtering, and streams bidirectional data to Signal's official servers.

use crate::sni::{get_record_expected_length, parse_sni, SniError};
use crate::whitelist::is_allowed_domain;
use log::{debug, error, info, warn};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Maximum allowed size for a single TLS ClientHello record (16 KB)
const MAX_RECORD_SIZE: usize = 16384;

/// Timeout for client to send initial TLS ClientHello
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// Timeout for establishing upstream connection to Signal servers
const UPSTREAM_TIMEOUT: Duration = Duration::from_secs(10);

/// Starts the proxy listener and runs the event loop.
pub async fn run_server(bind_addr: &str) -> std::io::Result<()> {
    let listener = TcpListener::bind(bind_addr).await?;
    info!("Signal TLS Proxy running on {}", bind_addr);

    loop {
        match listener.accept().await {
            Ok((stream, peer_addr)) => {
                tokio::spawn(async move {
                    if let Err(err) = handle_connection(stream, peer_addr).await {
                        debug!("[{}] Connection terminated with error: {}", peer_addr, err);
                    }
                });
            }
            Err(err) => {
                warn!("Failed to accept incoming TCP connection: {}", err);
            }
        }
    }
}

/// Handles a single incoming TCP stream.
async fn handle_connection(mut client: TcpStream, peer_addr: SocketAddr) -> std::io::Result<()> {
    // Optimize TCP latency
    let _ = client.set_nodelay(true);

    let mut buffer = Vec::with_capacity(2048);
    let mut chunk = [0u8; 1024];

    // Read the complete TLS record containing ClientHello
    let sni = loop {
        let read_future = client.read(chunk.as_mut_slice());
        let n = match tokio::time::timeout(HANDSHAKE_TIMEOUT, read_future).await {
            Ok(Ok(0)) => {
                // Connection closed by client / health probe before sending data
                return Ok(());
            }
            Ok(Ok(n)) => n,
            Ok(Err(err)) => return Err(err),
            Err(_) => {
                debug!("[{}] Handshake read timed out", peer_addr);
                return Ok(());
            }
        };

        buffer.extend_from_slice(&chunk.as_slice()[..n]);

        // Protocol Sniffing:
        // Signal TLS ClientHello always starts with ContentType 0x16 (Handshake).
        // If the first byte is NOT 0x16, this is a plain HTTP request from a web browser!
        if !buffer.is_empty() && buffer[0] != 0x16 {
            return handle_http(&mut client, &buffer, peer_addr).await;
        }

        if buffer.len() >= 5 {
            match get_record_expected_length(&buffer) {
                Ok(expected_len) => {
                    if buffer.len() >= expected_len {
                        // Entire TLS record is available, parse SNI
                        match parse_sni(&buffer) {
                            Ok(hostname) => break hostname,
                            Err(err) => {
                                warn!("[{}] Failed to parse SNI from ClientHello: {}", peer_addr, err);
                                return Ok(());
                            }
                        }
                    }
                }
                Err(SniError::Incomplete { .. }) => {
                    // Need more bytes to determine full record length
                }
                Err(err) => {
                    warn!("[{}] Invalid TLS record header: {}", peer_addr, err);
                    return Ok(());
                }
            }
        }

        if buffer.len() > MAX_RECORD_SIZE {
            warn!("[{}] ClientHello exceeded maximum record size", peer_addr);
            return Ok(());
        }
    };

    // Validate SNI against official Signal domains whitelist
    if !is_allowed_domain(&sni) {
        warn!(
            "[{}] Access denied: destination SNI '{}' is not an authorized Signal domain",
            peer_addr, sni
        );
        return Ok(());
    }

    debug!("[{}] Routing traffic to authorized Signal endpoint: {}", peer_addr, sni);

    // Connect to official Signal upstream on port 443
    let upstream_addr = format!("{}:443", sni);
    let mut upstream = match tokio::time::timeout(UPSTREAM_TIMEOUT, TcpStream::connect(&upstream_addr)).await {
        Ok(Ok(stream)) => stream,
        Ok(Err(err)) => {
            error!(
                "[{}] Failed to connect to Signal upstream {}: {}",
                peer_addr, sni, err
            );
            return Ok(());
        }
        Err(_) => {
            error!(
                "[{}] Connection to Signal upstream {} timed out",
                peer_addr, sni
            );
            return Ok(());
        }
    };

    let _ = upstream.set_nodelay(true);

    // Forward the initial buffered ClientHello record to upstream
    upstream.write_all(&buffer).await?;

    // Bidirectional full-duplex streaming until either side disconnects
    match tokio::io::copy_bidirectional(&mut client, &mut upstream).await {
        Ok((client_to_upstream, upstream_to_client)) => {
            debug!(
                "[{}] Tunnel closed: uploaded {} bytes, downloaded {} bytes via {}",
                peer_addr, client_to_upstream, upstream_to_client, sni
            );
        }
        Err(err) => {
            debug!(
                "[{}] Tunnel terminated with error for {}: {}",
                peer_addr, sni, err
            );
        }
    }

    Ok(())
}

/// Handles standard HTTP requests from web browsers by responding with the Web UI.
async fn handle_http(client: &mut TcpStream, buffer: &[u8], peer_addr: SocketAddr) -> std::io::Result<()> {
    let host = crate::web::parse_http_host(buffer).unwrap_or_else(|| {
        std::env::var("FLY_APP_NAME")
            .map(|n| format!("{}.fly.dev", n))
            .unwrap_or_else(|_| "localhost".to_string())
    });

    debug!("[{}] Serving Web UI dashboard for host: {}", peer_addr, host);
    let resp = crate::web::build_http_response(&host);
    client.write_all(&resp).await?;
    client.flush().await?;
    Ok(())
}
