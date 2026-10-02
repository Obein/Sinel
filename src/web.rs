//! Embedded Web UI for Signal TLS Proxy.
//!
//! Serves a self-contained, responsive dashboard when accessed via a web browser,
//! dynamically adapting to the requested Host domain without external dependencies.


/// Renders a dynamic HTML page displaying the Signal proxy share link.
pub fn render_html_page(host: &str) -> String {
    let share_url = format!("https://signal.tube/#{}", host);

    format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Signal TLS Proxy - {host}</title>
  <link rel="icon" href="data:image/svg+xml,<svg xmlns=%22http://www.w3.org/2000/svg%22 viewBox=%220 0 100 100%22><text y=%22.9em%22 font-size=%2290%22>🛡️</text></svg>">
  <style>
    :root {{
      --bg: #0b1120;
      --card-bg: #151f32;
      --card-border: #1e293b;
      --primary: #2c6bed;
      --primary-hover: #1d4ed8;
      --text-main: #f8fafc;
      --text-muted: #94a3b8;
      --success: #10b981;
      --success-bg: rgba(16, 185, 129, 0.1);
    }}
    * {{ box-sizing: border-box; margin: 0; padding: 0; }}
    body {{
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
      background-color: var(--bg);
      color: var(--text-main);
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      min-height: 100vh;
      padding: 24px;
    }}
    .container {{
      width: 100%;
      max-width: 540px;
      background: var(--card-bg);
      border: 1px solid var(--card-border);
      border-radius: 16px;
      padding: 36px 28px;
      box-shadow: 0 20px 40px -15px rgba(0, 0, 0, 0.5);
    }}
    .header {{
      display: flex;
      align-items: center;
      gap: 12px;
      margin-bottom: 8px;
    }}
    .badge {{
      display: inline-flex;
      align-items: center;
      gap: 6px;
      padding: 4px 10px;
      border-radius: 9999px;
      background: var(--success-bg);
      color: var(--success);
      font-size: 13px;
      font-weight: 500;
      margin-bottom: 20px;
    }}
    .badge-dot {{
      width: 8px;
      height: 8px;
      border-radius: 50%;
      background: var(--success);
      box-shadow: 0 0 10px var(--success);
      animation: pulse 2s infinite;
    }}
    @keyframes pulse {{
      0%, 100% {{ opacity: 1; }}
      50% {{ opacity: 0.4; }}
    }}
    h1 {{
      font-size: 24px;
      font-weight: 700;
      letter-spacing: -0.02em;
    }}
    p.desc {{
      color: var(--text-muted);
      font-size: 14px;
      line-height: 1.6;
      margin-bottom: 24px;
    }}
    .box {{
      background: #0f172a;
      border: 1px solid #334155;
      border-radius: 10px;
      padding: 12px 14px;
      display: flex;
      align-items: center;
      gap: 10px;
      margin-bottom: 16px;
    }}
    .box input {{
      background: transparent;
      border: none;
      color: var(--text-main);
      font-family: monospace;
      font-size: 14px;
      flex: 1;
      outline: none;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }}
    .btn {{
      display: inline-flex;
      align-items: center;
      justify-content: center;
      gap: 8px;
      background: var(--primary);
      color: #fff;
      font-weight: 600;
      font-size: 14px;
      padding: 12px 20px;
      border-radius: 8px;
      border: none;
      cursor: pointer;
      text-decoration: none;
      transition: background 0.15s ease;
      width: 100%;
    }}
    .btn:hover {{ background: var(--primary-hover); }}
    .btn-copy {{
      background: #334155;
      color: #cbd5e1;
      padding: 8px 14px;
      font-size: 13px;
      border-radius: 6px;
      width: auto;
    }}
    .btn-copy:hover {{ background: #475569; }}
    .btn-copy.copied {{
      background: var(--success);
      color: white;
    }}
    .actions {{
      display: flex;
      flex-direction: column;
      gap: 10px;
      margin-top: 8px;
    }}
    .instructions {{
      margin-top: 28px;
      padding-top: 20px;
      border-top: 1px solid var(--card-border);
      color: var(--text-muted);
      font-size: 13px;
      line-height: 1.6;
    }}
    .instructions ol {{
      margin-left: 18px;
      margin-top: 8px;
    }}
    .instructions li {{
      margin-bottom: 4px;
    }}
    .footer {{
      margin-top: 24px;
      text-align: center;
      color: #64748b;
      font-size: 12px;
    }}
  </style>
</head>
<body>
  <div class="container">
    <div class="header">
      <span style="font-size: 28px;">🛡️</span>
      <h1>Signal TLS Proxy</h1>
    </div>

    <div class="badge">
      <div class="badge-dot"></div>
      服务正常运行 (Fly.io)
    </div>

    <p class="desc">
      这是一个高可用、端到端加密的 Signal 抗审查专属代理通道。使用下方链接即可在 Signal 客户端上一键连接。
    </p>

    <div class="box">
      <input type="text" id="shareUrl" value="{share_url}" readonly>
      <button class="btn btn-copy" id="copyBtn" onclick="copyUrl()">复制</button>
    </div>

    <div class="actions">
      <a href="{share_url}" class="btn" target="_blank">一键连接 Signal</a>
    </div>

    <div class="instructions">
      <strong>使用方法：</strong>
      <ol>
        <li><strong>自动配置</strong>：点击上方“一键连接 Signal”按钮，手机会自动唤起 Signal App 并提示开启代理。</li>
        <li><strong>手动配置</strong>：在 Signal 中打开 <code>设置 &gt; 数据与存储 &gt; 使用代理</code>，填入地址 <code>{host}:443</code>。</li>
      </ol>
      <p style="margin-top: 10px; font-size: 12px; color: #64748b;">
        🔒 隐私保证：本代理基于 TLS-in-TLS 零知识隧道架构，所有聊天、附件及音视频通话均由端到端公钥加密，中继节点无权且无法查看任何通讯内容。
      </p>
    </div>
  </div>

  <div class="footer">
    Powered by Signal TLS Proxy (Rust Edition) &bull; Host: {host}
  </div>

  <script>
    function copyUrl() {{
      const input = document.getElementById('shareUrl');
      input.select();
      navigator.clipboard.writeText(input.value).then(() => {{
        const btn = document.getElementById('copyBtn');
        btn.textContent = '已复制!';
        btn.classList.add('copied');
        setTimeout(() => {{
          btn.textContent = '复制';
          btn.classList.remove('copied');
        }}, 2000);
      }});
    }}
  </script>
</body>
</html>"#,
        host = host,
        share_url = share_url
    )
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
