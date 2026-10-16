//! Embedded Web UI for Signal TLS Proxy.
//!
//! Serves a self-contained, responsive dashboard when accessed via a web browser,
//! dynamically adapting to the requested Host domain without external dependencies.

use std::sync::OnceLock;

/// Source templates loaded at compile time from the independent `web/` directory.
const HTML_SOURCE: &str = include_str!("../web/index.html");
const STYLE_CSS: &str = include_str!("../web/style.css");
const SCRIPT_JS: &str = include_str!("../web/app.js");

static ASSEMBLED_TEMPLATE: OnceLock<String> = OnceLock::new();

/// Assembles the separated HTML, CSS, and JS components into a self-contained
/// single-page template once on startup, maintaining zero runtime overhead and zero extra HTTP roundtrips.
fn get_assembled_template() -> &'static str {
    ASSEMBLED_TEMPLATE.get_or_init(|| {
        let style_tag = format!("<style>\n{}\n</style>", STYLE_CSS.trim());
        let script_tag = format!("<script>\n{}\n</script>", SCRIPT_JS.trim());

        HTML_SOURCE
            .replace(r#"<link rel="stylesheet" href="style.css">"#, &style_tag)
            .replace(r#"<script src="app.js"></script>"#, &script_tag)
    })
}

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

/// Renders a dynamic HTML page displaying the Signal proxy share link and active session count.
pub fn render_html_page(host: &str, active_connections: usize) -> String {
    let safe_host = escape_html(host);
    let share_url = format!("https://signal.tube/#{}", safe_host);
    get_assembled_template()
        .replace("{{HOST}}", &safe_host)
        .replace("{{SHARE_URL}}", &share_url)
        .replace("{{ACTIVE_CONNECTIONS}}", &active_connections.to_string())
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
pub fn build_http_response(host: &str, active_connections: usize) -> Vec<u8> {
    let html = render_html_page(host, active_connections);
    let response = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         Cache-Control: no-cache, no-store, must-revalidate\r\n\
         Content-Security-Policy: default-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none';\r\n\
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

/// Configuration for HTTP root camouflage and dashboard routing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CamouflageConfig {
    /// Optional HTTP status code for root ("/") requests: 302 or 404.
    pub root_status_code: Option<u16>,
    /// Target URL when `root_status_code` is 302.
    pub redirect_url: String,
    /// Normalized path where the dashboard is served (e.g. "/state" or "/").
    pub state_path: String,
}

impl CamouflageConfig {
    /// Constructs configuration from environment variables.
    pub fn from_env() -> Self {
        let root_status_code = std::env::var("ROOT_STATUS_CODE")
            .or_else(|_| std::env::var("root_status_code"))
            .ok()
            .and_then(|val| val.trim().parse::<u16>().ok());

        let redirect_url = std::env::var("REDIRECT_URL")
            .or_else(|_| std::env::var("redirect_url"))
            .or_else(|_| std::env::var("MASKED_URL"))
            .or_else(|_| std::env::var("MaskedURL"))
            .or_else(|_| std::env::var("masked_url"))
            .unwrap_or_else(|_| "https://www.google.com".to_string());

        let raw_state = std::env::var("STATE_PATH")
            .or_else(|_| std::env::var("state_path"))
            .or_else(|_| std::env::var("StatePath"))
            .unwrap_or_else(|_| "/state".to_string());

        Self::new(root_status_code, redirect_url, raw_state)
    }

    /// Explicit constructor with path normalization.
    pub fn new(root_status_code: Option<u16>, redirect_url: impl Into<String>, state_path: impl Into<String>) -> Self {
        Self {
            root_status_code,
            redirect_url: redirect_url.into(),
            state_path: normalize_path(&state_path.into()),
        }
    }
}

/// Normalizes a route path to guarantee a leading slash and no trailing slash (except for root "/").
pub fn normalize_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return "/".to_string();
    }
    let with_leading = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{}", trimmed)
    };
    let no_trailing = with_leading.trim_end_matches('/');
    if no_trailing.is_empty() {
        "/".to_string()
    } else {
        no_trailing.to_string()
    }
}

/// Extracts and parses the request target path from raw HTTP request bytes.
///
/// Strips absolute URI prefixes (`http://...`), query strings (`?lang=zh`), and hash fragments.
pub fn parse_http_path(buf: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(buf).ok()?;
    let first_line = text.lines().next()?;
    let mut parts = first_line.split_whitespace();
    let _method = parts.next()?;
    let target = parts.next()?;

    let path_part = if let Some(stripped) = target.strip_prefix("http://").or_else(|| target.strip_prefix("https://")) {
        if let Some(pos) = stripped.find('/') {
            &stripped[pos..]
        } else {
            "/"
        }
    } else {
        target
    };

    let clean_path = path_part.split('?').next().unwrap_or(path_part);
    let clean_path = clean_path.split('#').next().unwrap_or(clean_path);

    Some(clean_path.to_string())
}

/// Target HTTP route determined by path and camouflage routing rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpRoute {
    /// Operational endpoint returning active connection count JSON.
    Stats,
    /// Operational endpoint returning in-memory debug logs.
    Logs,
    /// Signal proxy dashboard web UI.
    Dashboard,
    /// HTTP 302 Found redirect to target URL.
    Redirect(String),
    /// HTTP 404 Not Found.
    NotFound,
}

/// Evaluates routing rules against the requested path.
pub fn match_route(raw_path: &str, config: &CamouflageConfig) -> HttpRoute {
    let norm_path = normalize_path(raw_path);

    // 1. Reserved operational endpoints (must remain functional for dashboard live polling)
    if norm_path == "/stats" || norm_path == "/api/stats" {
        return HttpRoute::Stats;
    }
    if norm_path == "/debug_logs" || norm_path == "/logs" {
        return HttpRoute::Logs;
    }

    // 2. State dashboard path matching
    // If state_path is configured as "/", root serves the dashboard unconditionally.
    if config.state_path == "/" {
        if norm_path == "/" {
            return HttpRoute::Dashboard;
        }
    } else if norm_path == config.state_path {
        return HttpRoute::Dashboard;
    }

    // 3. Root path ("/") behavior
    if norm_path == "/" {
        match config.root_status_code {
            Some(302) => HttpRoute::Redirect(config.redirect_url.clone()),
            Some(404) => HttpRoute::NotFound,
            _ => {
                // If ROOT_STATUS_CODE is unset or unrecognized, default to dashboard
                HttpRoute::Dashboard
            }
        }
    } else {
        // Unknown paths return 404
        HttpRoute::NotFound
    }
}

/// Constructs an HTTP 302 Found response redirecting to the specified location.
pub fn build_redirect_response(location: &str) -> Vec<u8> {
    let clean_location = location.replace(['\r', '\n'], "");
    let safe_body_location = escape_html(&clean_location);
    let body = format!(
        r#"<!DOCTYPE html><html><head><meta http-equiv="refresh" content="0;url={}"></head><body><a href="{}">Redirecting...</a></body></html>"#,
        safe_body_location, safe_body_location
    );
    let resp = format!(
        "HTTP/1.1 302 Found\r\n\
         Location: {}\r\n\
         Content-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         Cache-Control: no-cache, no-store, must-revalidate\r\n\
         \r\n\
         {}",
        clean_location,
        body.len(),
        body
    );
    resp.into_bytes()
}

/// Constructs a standard HTTP 404 Not Found response.
pub fn build_not_found_response() -> Vec<u8> {
    let body = "404 Not Found\n";
    let resp = format!(
        "HTTP/1.1 404 Not Found\r\n\
         Content-Type: text/plain; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         Cache-Control: no-cache, no-store, must-revalidate\r\n\
         \r\n\
         {}",
        body.len(),
        body
    );
    resp.into_bytes()
}

/// Constructs an HTTP 200 OK response with the JSON active connections payload.
pub fn build_stats_response(active_connections: usize) -> Vec<u8> {
    let body = format!(r#"{{"active_connections":{}}}"#, active_connections);
    let resp = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: application/json; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         Cache-Control: no-cache, no-store, must-revalidate\r\n\
         \r\n\
         {}",
        body.len(),
        body
    );
    resp.into_bytes()
}

/// Constructs an HTTP 200 OK response with the in-memory debug logs.
pub fn build_logs_response(logs: &str) -> Vec<u8> {
    let resp = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: text/plain; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {}",
        logs.len(),
        logs
    );
    resp.into_bytes()
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
        let html = render_html_page("signal.example.com", 5);
        assert!(html.contains(r#"<html lang="en-US">"#));
        assert!(html.contains("https://signal.tube/#signal.example.com"));
        assert!(html.contains("signal.example.com:443"));
        assert!(html.contains("5"));
        assert!(html.contains("https://github.com/Obein/Sinel"));
    }

    #[test]
    fn test_modular_web_assembly() {
        let html = render_html_page("signal.example.com", 0);
        // Verify inlined CSS and JS
        assert!(html.contains("<style>"));
        assert!(html.contains("--accent: #2c6bed;"));
        assert!(html.contains("<script>"));
        assert!(html.contains("function copyProxyUrl()"));
        // Verify external links are replaced
        assert!(!html.contains(r#"<link rel="stylesheet" href="style.css">"#));
        assert!(!html.contains(r#"<script src="app.js"></script>"#));
    }

    #[test]
    fn test_normalize_path() {
        assert_eq!(normalize_path(""), "/");
        assert_eq!(normalize_path("/"), "/");
        assert_eq!(normalize_path("///"), "/");
        assert_eq!(normalize_path("state"), "/state");
        assert_eq!(normalize_path("/state"), "/state");
        assert_eq!(normalize_path("/state/"), "/state");
        assert_eq!(normalize_path("/state///"), "/state");
        assert_eq!(normalize_path("custom/secret"), "/custom/secret");
    }

    #[test]
    fn test_parse_http_path() {
        let req1 = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
        assert_eq!(parse_http_path(req1), Some("/".to_string()));

        let req2 = b"GET /state HTTP/1.1\r\nHost: example.com\r\n\r\n";
        assert_eq!(parse_http_path(req2), Some("/state".to_string()));

        let req3 = b"GET /state?lang=zh HTTP/1.1\r\nHost: example.com\r\n\r\n";
        assert_eq!(parse_http_path(req3), Some("/state".to_string()));

        let req4 = b"GET /state#top HTTP/1.1\r\nHost: example.com\r\n\r\n";
        assert_eq!(parse_http_path(req4), Some("/state".to_string()));

        let req5 = b"GET http://proxy.example.com/state HTTP/1.1\r\n\r\n";
        assert_eq!(parse_http_path(req5), Some("/state".to_string()));

        let req6 = b"";
        assert_eq!(parse_http_path(req6), None);
    }

    #[test]
    fn test_match_route_default_backward_compatible() {
        // When ROOT_STATUS_CODE is not set, root ("/") defaults to dashboard.
        let config = CamouflageConfig::new(None, "https://www.google.com", "/state");
        assert_eq!(match_route("/", &config), HttpRoute::Dashboard);
        assert_eq!(match_route("/state", &config), HttpRoute::Dashboard);
        assert_eq!(match_route("/state/", &config), HttpRoute::Dashboard);
        assert_eq!(match_route("/stats", &config), HttpRoute::Stats);
        assert_eq!(match_route("/debug_logs", &config), HttpRoute::Logs);
        assert_eq!(match_route("/other", &config), HttpRoute::NotFound);
    }

    #[test]
    fn test_match_route_302_redirect() {
        let config = CamouflageConfig::new(Some(302), "https://example.com/target", "/state");
        assert_eq!(
            match_route("/", &config),
            HttpRoute::Redirect("https://example.com/target".to_string())
        );
        assert_eq!(match_route("/state", &config), HttpRoute::Dashboard);
        assert_eq!(match_route("/state/", &config), HttpRoute::Dashboard);
        assert_eq!(match_route("/stats", &config), HttpRoute::Stats);
        assert_eq!(match_route("/other", &config), HttpRoute::NotFound);
    }

    #[test]
    fn test_match_route_404_camouflage() {
        let config = CamouflageConfig::new(Some(404), "https://www.google.com", "/state");
        assert_eq!(match_route("/", &config), HttpRoute::NotFound);
        assert_eq!(match_route("/state", &config), HttpRoute::Dashboard);
        assert_eq!(match_route("/state/", &config), HttpRoute::Dashboard);
        assert_eq!(match_route("/stats", &config), HttpRoute::Stats);
        assert_eq!(match_route("/other", &config), HttpRoute::NotFound);
    }

    #[test]
    fn test_match_route_state_path_root() {
        // When STATE_PATH is explicitly "/", root always serves the dashboard even if 302/404 is set.
        let config = CamouflageConfig::new(Some(302), "https://www.google.com", "/");
        assert_eq!(match_route("/", &config), HttpRoute::Dashboard);
    }

    #[test]
    fn test_match_route_custom_secret_state() {
        let config = CamouflageConfig::new(Some(404), "https://www.google.com", "/my-secret-path");
        assert_eq!(match_route("/", &config), HttpRoute::NotFound);
        assert_eq!(match_route("/state", &config), HttpRoute::NotFound);
        assert_eq!(match_route("/my-secret-path", &config), HttpRoute::Dashboard);
        assert_eq!(match_route("/my-secret-path/", &config), HttpRoute::Dashboard);
    }

    #[test]
    fn test_build_redirect_response() {
        let resp = String::from_utf8(build_redirect_response("https://www.google.com")).unwrap();
        assert!(resp.starts_with("HTTP/1.1 302 Found\r\n"));
        assert!(resp.contains("Location: https://www.google.com\r\n"));
        assert!(resp.contains("Content-Type: text/html; charset=utf-8\r\n"));
        assert!(resp.contains("https://www.google.com"));
    }

    #[test]
    fn test_build_redirect_response_strips_crlf() {
        let malicious = "https://evil.com\r\nInjected-Header: evil";
        let resp = String::from_utf8(build_redirect_response(malicious)).unwrap();
        assert!(!resp.contains("Injected-Header:"));
        assert!(resp.contains("Location: https://evil.comInjected-Header: evil\r\n"));
    }

    #[test]
    fn test_build_not_found_response() {
        let resp = String::from_utf8(build_not_found_response()).unwrap();
        assert!(resp.starts_with("HTTP/1.1 404 Not Found\r\n"));
        assert!(resp.contains("Content-Length: 14\r\n"));
        assert!(resp.ends_with("\r\n\r\n404 Not Found\n"));
    }

    #[test]
    fn test_build_stats_response() {
        let resp = String::from_utf8(build_stats_response(42)).unwrap();
        assert!(resp.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(resp.contains(r#"{"active_connections":42}"#));
    }
}
