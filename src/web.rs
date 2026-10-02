//! Embedded Web UI for Signal TLS Proxy.
//!
//! Serves a self-contained, responsive dashboard when accessed via a web browser,
//! dynamically adapting to the requested Host domain without external dependencies.

/// Embedded HTML template loaded at compile time from the independent `web/` directory.
const HTML_TEMPLATE: &str = include_str!("../web/index.html");

/// Renders a dynamic HTML page displaying the Signal proxy share link.
pub fn render_html_page(host: &str) -> String {
    let share_url = format!("https://signal.tube/#{}", host);
    HTML_TEMPLATE
        .replace("{{HOST}}", host)
        .replace("{{SHARE_URL}}", &share_url)
}

/// Parses headers from raw HTTP request bytes.
pub fn parse_http_host(buf: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(buf).ok()?;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.to_lowercase().starts_with("host:") {
            let parts: Vec<&str> = trimmed.splitn(2, ':').collect();
            if parts.len() == 2 {
                // Strip optional port (e.g. "sinel.fly.dev:443" -> "sinel.fly.dev")
                let host_part = parts[1].trim();
                let clean_host = host_part.split(':').next().unwrap_or(host_part).trim();
                if !clean_host.is_empty() {
                    return Some(clean_host.to_string());
                }
            }
        }
    }
    None
}

/// Constructs a complete HTTP 200 OK response with the rendered HTML.
pub fn build_http_response(host: &str) -> Vec<u8> {
    let html = render_html_page(host);
    let response = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         Cache-Control: no-cache, no-store, must-revalidate\r\n\
         \r\n\
         {}",
        html.len(),
        html
    );
    response.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_host_simple() {
        let req = b"GET / HTTP/1.1\r\nHost: sinel.fly.dev\r\nUser-Agent: curl\r\n\r\n";
        assert_eq!(parse_http_host(req), Some("sinel.fly.dev".to_string()));
    }

    #[test]
    fn test_parse_host_with_port() {
        let req = b"GET / HTTP/1.1\r\nHost: signal.example.com:443\r\nAccept: */*\r\n\r\n";
        assert_eq!(parse_http_host(req), Some("signal.example.com".to_string()));
    }

    #[test]
    fn test_render_html_contains_host() {
        let html = render_html_page("sinel.fly.dev");
        assert!(html.contains("https://signal.tube/#sinel.fly.dev"));
        assert!(html.contains("sinel.fly.dev:443"));
    }
}
