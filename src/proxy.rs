//! Asynchronous TCP proxy engine for Signal TLS traffic.
//!
//! Handles incoming decrypted TCP connections from Fly.io, extracts the inner SNI,
//! applies whitelist filtering, and streams bidirectional data to Signal's official servers.

use crate::sni::{get_record_expected_length, parse_sni, SniError};
use crate::whitelist::is_allowed_domain;
use log::{debug, error, info, warn};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;

/// Maximum allowed size for a single TLS ClientHello record (16 KB)
const MAX_RECORD_SIZE: usize = 16384;

/// Maximum allowed size for HTTP request headers (4 KB)
const MAX_HTTP_HEADER_SIZE: usize = 4096;

/// Maximum concurrent client connections to protect the 256MB VM from OOM
const MAX_CONCURRENT_CONNECTIONS: usize = 2048;

/// Cumulative timeout for client to send initial TLS ClientHello or HTTP headers
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// Timeout for establishing upstream connection to Signal servers
const UPSTREAM_TIMEOUT: Duration = Duration::from_secs(10);

/// Starts the proxy listener and runs the event loop with connection bounding.
pub async fn run_server(bind_addr: &str) -> std::io::Result<()> {
    let listener = TcpListener::bind(bind_addr).await?;
    info!("Signal TLS Proxy running on {}", bind_addr);

    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_CONNECTIONS));

    loop {
        match listener.accept().await {
            Ok((stream, peer_addr)) => {
                let permit = match semaphore.clone().try_acquire_owned() {
                    Ok(permit) => permit,
                    Err(_) => {
                        warn!(
                            "[{}] Active connection limit ({}) reached, rejecting connection",
                            peer_addr, MAX_CONCURRENT_CONNECTIONS
                        );
                        // stream drops here, closing the connection immediately
                        continue;
                    }
                };

                tokio::spawn(async move {
                    let _permit = permit;
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

enum HandshakeResult {
    Tls(String),
    HttpHandled,
    WhatsApp,
}

/// Reads the incoming payload with protocol sniffing.
///
/// Bounded by cumulative timeout caller-side to prevent Slowloris attacks.
async fn read_initial_payload(
    client: &mut TcpStream,
    buffer: &mut Vec<u8>,
    peer_addr: SocketAddr,
) -> std::io::Result<HandshakeResult> {
    let mut chunk = [0u8; 1024];

    loop {
        let n = client.read(chunk.as_mut_slice()).await?;
        if n == 0 {
            // Connection closed before complete handshake (e.g. health probe)
            return Ok(HandshakeResult::HttpHandled);
        }

        buffer.extend_from_slice(&chunk.as_slice()[..n]);

        if buffer.len() < 2 {
            continue;
        }

        // WhatsApp Noise protocol check
        if crate::whatsapp::is_whatsapp_handshake(buffer) {
            return Ok(HandshakeResult::WhatsApp);
        }

        // Protocol Sniffing:
        // Signal TLS ClientHello always starts with ContentType 0x16 (Handshake).
        // If the first byte is NOT 0x16, this is a plain HTTP request from a web browser!
        if buffer[0] != 0x16 {
            // Buffer until complete HTTP headers have arrived (\r\n\r\n or \n\n) or limit reached
            let has_headers_end = buffer.windows(4).any(|w| w == b"\r\n\r\n")
                || buffer.windows(2).any(|w| w == b"\n\n");

            if has_headers_end || buffer.len() >= MAX_HTTP_HEADER_SIZE {
                handle_http(client, buffer, peer_addr).await?;
                return Ok(HandshakeResult::HttpHandled);
            }
            continue;
        }

        // TLS Handshake processing
        if buffer.len() >= 5 {
            match get_record_expected_length(buffer) {
                Ok(expected_len) => {
                    if buffer.len() >= expected_len {
                        // Entire TLS record is available, parse SNI
                        match parse_sni(buffer) {
                            Ok(hostname) => return Ok(HandshakeResult::Tls(hostname)),
                            Err(err) => {
                                warn!("[{}] Failed to parse SNI from ClientHello: {}", peer_addr, err);
                                return Ok(HandshakeResult::HttpHandled);
                            }
                        }
                    }
                }
                Err(SniError::Incomplete { .. }) => {
                    // Need more bytes to determine full record length
                }
                Err(err) => {
                    warn!("[{}] Invalid TLS record header: {}", peer_addr, err);
                    return Ok(HandshakeResult::HttpHandled);
                }
            }
        }

        if buffer.len() > MAX_RECORD_SIZE {
            warn!("[{}] ClientHello exceeded maximum record size", peer_addr);
            return Ok(HandshakeResult::HttpHandled);
        }
    }
}

/// Handles a single incoming TCP stream.
async fn handle_connection(mut client: TcpStream, peer_addr: SocketAddr) -> std::io::Result<()> {
    // Optimize TCP latency
    let _ = client.set_nodelay(true);

    let mut buffer = Vec::with_capacity(2048);

    // Cumulative timeout across the entire handshake payload read
    let handshake_result = match tokio::time::timeout(
        HANDSHAKE_TIMEOUT,
        read_initial_payload(&mut client, &mut buffer, peer_addr),
    )
    .await
    {
        Ok(Ok(res)) => res,
        Ok(Err(err)) => return Err(err),
        Err(_) => {
            debug!("[{}] Handshake read timed out", peer_addr);
            return Ok(());
        }
    };

    let sni = match handshake_result {
        HandshakeResult::Tls(hostname) => hostname,
        HandshakeResult::HttpHandled => return Ok(()),
        HandshakeResult::WhatsApp => {
            return crate::whatsapp::handle_whatsapp(&mut client, &buffer, peer_addr).await;
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
            .ok()
            .and_then(|n| {
                let candidate = format!("{}.fly.dev", n);
                if crate::web::is_valid_hostname(&candidate) {
                    Some(candidate)
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "localhost".to_string())
    });

    debug!("[{}] Serving Web UI dashboard for host: {}", peer_addr, host);
    let resp = crate::web::build_http_response(&host);
    client.write_all(&resp).await?;
    client.flush().await?;
    Ok(())
}
