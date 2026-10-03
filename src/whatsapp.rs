//! WhatsApp Text Proxy module for Sinel.
//!
//! Handles incoming decrypted WhatsApp Noise protocol connections on port 443,
//! prepends the required HAProxy PROXY Protocol v1 header, and streams data
//! directly to the official WhatsApp messaging infrastructure (g.whatsapp.net:5222).

use log::{debug, error, warn};
use std::env;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

/// Default upstream destination for WhatsApp messaging servers
pub const DEFAULT_WHATSAPP_UPSTREAM: &str = "g.whatsapp.net:5222";

/// Timeout for establishing upstream connection to WhatsApp servers
const UPSTREAM_TIMEOUT: Duration = Duration::from_secs(10);

/// Checks whether the WhatsApp proxy feature is enabled.
pub fn is_whatsapp_enabled() -> bool {
    env::var("ENABLE_WHATSAPP_PROXY")
        .map(|val| val.to_lowercase() != "false" && val != "0")
        .unwrap_or(true)
}

/// Retrieves the configured WhatsApp upstream endpoint.
pub fn get_whatsapp_upstream() -> String {
    env::var("WHATSAPP_UPSTREAM").unwrap_or_else(|_| DEFAULT_WHATSAPP_UPSTREAM.to_string())
}

/// Determines whether the payload represents a WhatsApp Noise protocol initiation.
///
/// WhatsApp clients begin their Noise handshake with the ASCII identifier "WA" (0x57 0x41)
/// followed by the protocol version byte (e.g. 0x04, 0x05, 0x06).
pub fn is_whatsapp_handshake(buf: &[u8]) -> bool {
    buf.len() >= 2 && buf[0] == 0x57 && buf[1] == 0x41
}

/// Builds an RFC-compliant HAProxy PROXY Protocol v1 header line.
///
/// Format: `PROXY <TCP4|TCP6> <src_ip> <dst_ip> <src_port> 443\r\n`
pub fn build_proxy_v1_header(peer_addr: SocketAddr, local_addr: Option<SocketAddr>) -> String {
    let dst_port = 443;
    match (peer_addr, local_addr) {
        (SocketAddr::V4(src), Some(SocketAddr::V4(dst))) => {
            format!("PROXY TCP4 {} {} {} {}\r\n", src.ip(), dst.ip(), src.port(), dst_port)
        }
        (SocketAddr::V6(src), Some(SocketAddr::V6(dst))) => {
            format!("PROXY TCP6 {} {} {} {}\r\n", src.ip(), dst.ip(), src.port(), dst_port)
        }
        (SocketAddr::V4(src), _) => {
            format!("PROXY TCP4 {} 127.0.0.1 {} {}\r\n", src.ip(), src.port(), dst_port)
        }
        (SocketAddr::V6(src), _) => {
            format!("PROXY TCP6 {} ::1 {} {}\r\n", src.ip(), src.port(), dst_port)
        }
    }
}

/// Relays decrypted WhatsApp chat traffic to the official WhatsApp backend.
pub async fn handle_whatsapp(
    client: &mut TcpStream,
    initial_buffer: &[u8],
    peer_addr: SocketAddr,
) -> std::io::Result<()> {
    if !is_whatsapp_enabled() {
        warn!("[{}] WhatsApp proxy request received but feature is disabled", peer_addr);
        return Ok(());
    }

    let upstream_addr = get_whatsapp_upstream();
    debug!("[{}] Routing WhatsApp chat connection to upstream: {}", peer_addr, upstream_addr);

    let local_addr = client.local_addr().ok();
    let proxy_header = build_proxy_v1_header(peer_addr, local_addr);

    let mut upstream = match tokio::time::timeout(UPSTREAM_TIMEOUT, TcpStream::connect(&upstream_addr)).await {
        Ok(Ok(stream)) => stream,
        Ok(Err(err)) => {
            error!(
                "[{}] Failed to connect to WhatsApp upstream {}: {}",
                peer_addr, upstream_addr, err
            );
            return Ok(());
        }
        Err(_) => {
            error!(
                "[{}] Connection to WhatsApp upstream {} timed out",
                peer_addr, upstream_addr
            );
            return Ok(());
        }
    };

    let _ = upstream.set_nodelay(true);

    // 1. Send HAProxy PROXY Protocol v1 header
    upstream.write_all(proxy_header.as_bytes()).await?;

    // 2. Forward the buffered initial client payload (starting with b"WA...")
    upstream.write_all(initial_buffer).await?;

    // 3. Bidirectional full-duplex streaming until either side disconnects
    match tokio::io::copy_bidirectional(client, &mut upstream).await {
        Ok((client_to_upstream, upstream_to_client)) => {
            debug!(
                "[{}] WhatsApp tunnel closed: uploaded {} bytes, downloaded {} bytes via {}",
                peer_addr, client_to_upstream, upstream_to_client, upstream_addr
            );
        }
        Err(err) => {
            debug!(
                "[{}] WhatsApp tunnel terminated with error for {}: {}",
                peer_addr, upstream_addr, err
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6};

    #[test]
    fn test_is_whatsapp_handshake() {
        assert!(is_whatsapp_handshake(b"WA\x06\x05\x00\x00"));
        assert!(is_whatsapp_handshake(b"WA\x04\x01"));
        assert!(is_whatsapp_handshake(b"WA"));

        // Non-WhatsApp payloads
        assert!(!is_whatsapp_handshake(b"GET / HTTP/1.1"));
        assert!(!is_whatsapp_handshake(b"\x16\x03\x01\x00"));
        assert!(!is_whatsapp_handshake(b"W"));
        assert!(!is_whatsapp_handshake(b""));
    }

    #[test]
    fn test_build_proxy_v1_header_ipv4() {
        let src = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(203, 0, 113, 195), 51234));
        let dst = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 1), 8080));

        let header = build_proxy_v1_header(src, Some(dst));
        assert_eq!(
            header,
            "PROXY TCP4 203.0.113.195 198.51.100.1 51234 443\r\n"
        );
    }

    #[test]
    fn test_build_proxy_v1_header_ipv6() {
        let src = SocketAddr::V6(SocketAddrV6::new(
            Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1),
            56789,
            0,
            0,
        ));
        let dst = SocketAddr::V6(SocketAddrV6::new(
            Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 2),
            8080,
            0,
            0,
        ));

        let header = build_proxy_v1_header(src, Some(dst));
        assert_eq!(
            header,
            "PROXY TCP6 2001:db8::1 2001:db8::2 56789 443\r\n"
        );
    }

    #[test]
    fn test_build_proxy_v1_header_fallback() {
        let src = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 1), 45678));
        let header = build_proxy_v1_header(src, None);
        assert_eq!(header, "PROXY TCP4 192.0.2.1 127.0.0.1 45678 443\r\n");
    }
}
