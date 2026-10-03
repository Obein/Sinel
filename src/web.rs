//! Embedded Web UI for Signal TLS Proxy.
//!
//! Serves a self-contained, responsive dashboard when accessed via a web browser,
//! dynamically adapting to the requested Host domain without external dependencies.

/// Embedded HTML template loaded at compile time from the independent `web/` directory.
const HTML_TEMPLATE: &str = include_str!("../web/index.html");

/// Validates that a hostname conforms strictly to RFC 1123 domain syntax.
///
/// Prevents Host Header Injection, CRLF injection, and XSS payload reflection.
pub fn is_valid_hostname(host: &str) -> bool {
    if host.is_empty() || host.len() > 253 {
        return false;
    }
    // Must not start or end with '.' or '-'
    if host.starts_with('.') || host.ends_with('.') || host.starts_with('-') || host.ends_with('-') {
        return false;
    }
    // Check each DNS label
    for label in host.split('.') {
        if label.is_empty() || label.len() > 63 {
            return false;
        }
        if label.starts_with('-') || label.ends_with('-') {
            return false;
        }
        if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return false;
        }
    }
    true
}

/// Escapes HTML special characters to prevent Cross-Site Scripting (XSS).
pub fn escape_html(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

/// Renders a dynamic HTML page displaying the Signal proxy share link.
pub fn render_html_page(host: &str) -> String {
    let safe_host = escape_html(host);
    let share_url = format!("https://signal.tube/#{}", safe_host);
    HTML_TEMPLATE
        .replace("{{HOST}}", &safe_host)
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
                // Strip optional port (e.g. "signal.example.com:443" -> "signal.example.com")
                let host_part = parts[1].trim();
                let clean_host = host_part.split(':').next().unwrap_or(host_part).trim();
                // Validate domain strictly against RFC 1123 syntax
                if is_valid_hostname(clean_host) {
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
         Content-Security-Policy: default-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'none'; frame-ancestors 'none';\r\n\
         X-Content-Type-Options: nosniff\r\n\
         X-Frame-Options: DENY\r\n\
         Referrer-Policy: no-referrer\r\n\
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
        let req = b"GET / HTTP/1.1\r\nHost: signal.example.com\r\nUser-Agent: curl\r\n\r\n";
        assert_eq!(parse_http_host(req), Some("signal.example.com".to_string()));
    }

    #[test]
    fn test_parse_host_with_port() {
        let req = b"GET / HTTP/1.1\r\nHost: signal.example.com:443\r\nAccept: */*\r\n\r\n";
        assert_eq!(parse_http_host(req), Some("signal.example.com".to_string()));
    }

    #[test]
    fn test_parse_host_rejects_xss_injection() {
        let req = b"GET / HTTP/1.1\r\nHost: test\"><script>alert(1)</script>\r\n\r\n";
        assert_eq!(parse_http_host(req), None);

        let req_crlf = b"GET / HTTP/1.1\r\nHost: example.com\r\nInjected: header\r\n\r\n";
        assert_eq!(parse_http_host(req_crlf), Some("example.com".to_string()));
    }

    #[test]
    fn test_is_valid_hostname() {
        assert!(is_valid_hostname("signal.example.com"));
        assert!(is_valid_hostname("my-proxy.sinel.io"));
        assert!(is_valid_hostname("localhost"));
        assert!(is_valid_hostname("a.b.c.d"));

        // Invalid hostnames
        assert!(!is_valid_hostname(""));
        assert!(!is_valid_hostname("signal..example.com"));
        assert!(!is_valid_hostname("-signal.com"));
        assert!(!is_valid_hostname("signal.com-"));
        assert!(!is_valid_hostname(".signal.com"));
        assert!(!is_valid_hostname("signal.com."));
        assert!(!is_valid_hostname("example.com/test"));
        assert!(!is_valid_hostname("example.com?foo=bar"));
        assert!(!is_valid_hostname("example.com\r\n"));
        assert!(!is_valid_hostname("<script>"));
    }

    #[test]
    fn test_escape_html() {
        assert_eq!(escape_html("normal-host.com"), "normal-host.com");
        assert_eq!(
            escape_html("test<script>\"'&"),
            "test&lt;script&gt;&quot;&#39;&amp;"
        );
    }

    #[test]
    fn test_render_html_contains_host() {
        let html = render_html_page("signal.example.com");
        assert!(html.contains("https://signal.tube/#signal.example.com"));
        assert!(html.contains("signal.example.com:443"));
    }
}
