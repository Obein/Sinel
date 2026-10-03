//! Domain whitelist verification for Signal TLS Proxy.
//!
//! Validates destination SNI against official Signal domains to prevent
//! the proxy from being abused as an open relay for arbitrary targets.

use std::collections::HashSet;
use std::env;
use std::sync::OnceLock;

/// Official production Signal services from Signal-TLS-Proxy
const DEFAULT_SIGNAL_DOMAINS: &[&str] = &[
    // Core messaging and storage
    "chat.signal.org",
    "storage.signal.org",
    // CDNs
    "cdn.signal.org",
    "cdn2.signal.org",
    "cdn3.signal.org",
    // Directory and contacts
    "cdsi.signal.org",
    "contentproxy.signal.org",
    "grpc.chat.signal.org",
    // Voice and video calling
    "sfu.voip.signal.org",
    // Key backup and discovery
    "svr2.signal.org",
    "svrb.signal.org",
    // Client updates
    "updates.signal.org",
    "updates2.signal.org",
    // Staging endpoints (optional fallback)
    "chat.staging.signal.org",
    "storage-staging.signal.org",
    "cdn-staging.signal.org",
    "cdn2-staging.signal.org",
    "cdn3-staging.signal.org",
    "cdsi.staging.signal.org",
    "grpc.chat.staging.signal.org",
    "sfu.staging.voip.signal.org",
    "svr2.staging.signal.org",
    "svrb.staging.signal.org",
];

static WHITELIST_SET: OnceLock<HashSet<String>> = OnceLock::new();
static ALLOW_ALL_SUBDOMAINS: OnceLock<bool> = OnceLock::new();

/// Initializes the whitelist cache once on startup.
fn get_whitelist() -> &'static HashSet<String> {
    WHITELIST_SET.get_or_init(|| {
        let mut set = HashSet::new();
        for &domain in DEFAULT_SIGNAL_DOMAINS {
            set.insert(domain.to_lowercase());
        }

        // Allow operator to add additional domains via environment variable
        if let Ok(extra) = env::var("EXTRA_ALLOWED_DOMAINS") {
            for d in extra.split(',') {
                let trimmed = d.trim().to_lowercase();
                if !trimmed.is_empty() {
                    set.insert(trimmed);
                }
            }
        }

        set
    })
}

/// Checks whether any *.signal.org subdomain is permitted.
fn is_subdomain_wildcard_enabled() -> bool {
    *ALLOW_ALL_SUBDOMAINS.get_or_init(|| {
        // Default to true to be future-proof against Signal CDN/service updates,
        // but can be set to false via env ALLOW_ALL_SIGNAL_SUBDOMAINS=false for strict mode.
        env::var("ALLOW_ALL_SIGNAL_SUBDOMAINS")
            .map(|val| val.to_lowercase() != "false" && val != "0")
            .unwrap_or(true)
    })
}

/// Checks if the requested SNI hostname is an authorized Signal domain.
pub fn is_allowed_domain(sni: &str) -> bool {
    let lower = sni.to_lowercase();

    // 1. Direct whitelist match
    if get_whitelist().contains(&lower) {
        return true;
    }

    // 2. Wildcard match for *.signal.org or *.voip.signal.org if enabled
    if is_subdomain_wildcard_enabled()
        && (lower.ends_with(".signal.org") || lower.ends_with(".voip.signal.org"))
        && !lower.starts_with('.')
        && !lower.contains("..")
    {
        return true;
    }

    // 3. Match WhatsApp endpoints when WhatsApp proxy is enabled
    if crate::whatsapp::is_whatsapp_enabled()
        && !lower.starts_with('.')
        && !lower.contains("..")
        && (lower == "whatsapp.net"
            || lower.ends_with(".whatsapp.net")
            || lower == "whatsapp.com"
            || lower.ends_with(".whatsapp.com"))
    {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_official_domains_allowed() {
        assert!(is_allowed_domain("chat.signal.org"));
        assert!(is_allowed_domain("storage.signal.org"));
        assert!(is_allowed_domain("cdn2.signal.org"));
        assert!(is_allowed_domain("sfu.voip.signal.org"));
        assert!(is_allowed_domain("grpc.chat.signal.org"));
    }

    #[test]
    fn test_case_insensitivity() {
        assert!(is_allowed_domain("CHAT.SIGNAL.ORG"));
        assert!(is_allowed_domain("Storage.Signal.Org"));
    }

    #[test]
    fn test_disallowed_domains() {
        assert!(!is_allowed_domain("google.com"));
        assert!(!is_allowed_domain("evil-signal.org.attacker.com"));
        assert!(!is_allowed_domain("signal.org.evil.com"));
        assert!(!is_allowed_domain("example.org"));
    }

    #[test]
    fn test_future_subdomain_allowed() {
        assert!(is_allowed_domain("cdn4.signal.org"));
        assert!(is_allowed_domain("svr3.signal.org"));
    }
}
