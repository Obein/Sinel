# Sinel: Signal TLS Proxy for Fly.io (Rust Edition)

English | [中文](README-zh.md)

> An ultra-lightweight **Signal TLS-in-TLS Obfuscation Reverse Proxy** tailored specifically for **Fly.io's Free Allowance ($5 monthly invoice waiver policy)**. Written in pure asynchronous Rust with a minimal memory footprint (< 15 MB), completely eliminating the operational burden of managing Nginx containers and Let's Encrypt certificate renewals.

---

## Architecture & How It Works

This proxy accurately reproduces the core obfuscation mechanism of the official `Signal-TLS-Proxy`, while leveraging Fly.io's global Anycast edge network:

```
[ Signal Client (Mobile / Desktop) ]
           │
           │  1. Outer TLS connection (Port 443, SNI: your-domain.com)
           ▼
[ Fly.io Global Edge (443) ]
           │  • Automated Let's Encrypt certificate issuance and renewals
           │  • handlers = ["tls"] (terminates outer TLS without HTTP parsing)
           ▼
[ Rust Core Proxy Service (:8080) ]
           │  2. Receives decrypted raw TCP stream (containing inner TLS ClientHello or HTTP request)
           │  3. Zero-copy protocol inspection:
           │     ├─ If TLS (0x16): Sniffs inner SNI (e.g. chat.signal.org) & validates against whitelist
           │     └─ If HTTP (GET /): Intercepts and serves minimalist Swiss Design Web UI
           ▼
[ Official Signal Servers (chat.signal.org:443) ]
           • Inner TLS remains encrypted end-to-end directly with Signal servers;
             the proxy has zero knowledge of any chat history, voice, or message payloads.
```

### Key Features
- **Pure Rust Asynchronous Engine**: Built on Tokio with a zero-copy TLS ClientHello SNI parser for high-performance, full-duplex TCP streaming.
- **Built-in Swiss Design Web UI**: When accessed via a standard web browser, it renders a modern Swiss-style landing page with English/Chinese toggle, dynamic hostname detection for `https://signal.tube/#<your_host>`, one-click link copying, and deep-link button to open the Signal app.
- **Zero Certificate Maintenance**: Fully automated edge certificates via Fly.io. No need to run `certbot`, cron jobs, or mount persistent volumes.
- **Zero-Knowledge Privacy**: End-to-end encryption takes place directly between the user's device and official Signal servers. The proxy cannot inspect or tamper with any decrypted communication.
- **Anti-Abuse Whitelist**: Enforces a strict domain whitelist covering official Signal services (`chat.signal.org`, `storage.signal.org`, `sfu.voip.signal.org`, etc.), preventing port scanners from using your instance as an open relay.

---

## Free Tier & Cost Guarantee

Fly.io enforces an official billing rule: **"Monthly invoices under $5.00 USD are automatically waived ($0 charge)"**.

| Resource | Specification | Estimated Cost | Final Invoiced Amount |
| :--- | :--- | :--- | :--- |
| **Compute Instance** | 1 × `shared-cpu-1x` (256MB RAM) running 24/7 | ~$1.94 / month | **$0.00** (Waived under $5 rule) |
| **Always On** | `auto_stop_machines = false` (never sleeps) | Included above | **$0.00** |
| **Outbound Data** | 100 GB / month included free | $0.00 | **$0.00** |
| **SSL/TLS Certificates** | Automated Let's Encrypt managed by Fly.io | Included free | **$0.00** |

> **Tip**: Keep the machine size at `shared-cpu-1x` with `256MB` RAM on a single instance, and your monthly Fly.io bill will remain permanently at **$0.00**.

---

## Quick Start: One-Click Fork & Web Dashboard Deployment (Recommended)

> 💡 **No local development tools required** (no `flyctl`, no Docker, no Rust compiler). The entire setup can be completed via GitHub and the Fly.io Web Console with **zero API tokens**.

### Step 1: Fork This Repository
Click the **Fork** button at the top-right corner of this repository to copy it into your own GitHub account.

---

### Step 2: Deploy from the Fly.io Web Console
1. Visit and log in to the [Fly.io Dashboard](https://fly.io/dashboard) (new accounts require a credit card for verification; actual billing is $0 if kept under $5/mo).
2. Click **Launch an app from GitHub**.
3. Authorize Fly.io and select your forked `Sinel` repository.
4. Fill in the deployment form with the following settings:

| Field | Recommended Value | Notes |
| :--- | :--- | :--- |
| **App Name** | Unique custom name | e.g. `my-signal-proxy-2026` (must be globally unique) |
| **Region** | **`sin - Singapore, Singapore`** or **`nrt - Tokyo, Japan`** | Lower latency for Asian regions. For US West, choose `sjc - San Jose` or `lax - Los Angeles`. |
| **Internal port** | **`8080`** | **CRITICAL!** Must be `8080` (the Rust proxy listens on internal port 8080). |
| **Machine Sizes**<br>• CPU(s)<br>• Memory | <br>**`shared-cpu-1x`**<br>**`256MB`** | **Free Tier Requirement!** This configuration costs only ~$1.94/month, falling well below the $5 waiver limit. |
| **Environment Variables** | Leave blank | Code includes sensible pre-configured defaults. |
| **Database** | **Leave unchecked** (Managed Postgres) | Not needed; proxy runs entirely in memory. |
| **Working directory** | Leave blank (default `./`) | Keep default. |
| **Config path** | Leave blank (default `./fly.toml`) | Automatically applies the included `fly.toml` with `handlers = ["tls"]`. |

5. Click the purple **Deploy** button at the bottom. Fly.io will build and launch your application automatically.

---

### Step 3: Allocate Public IP Addresses (Crucial Step!)
Newly created Fly.io applications do not have public IP addresses by default (`fly.dev` will not resolve until an IP is assigned):

1. In the [Fly.io Dashboard](https://fly.io/dashboard), navigate to your app's **Overview** page;
2. Scroll down to the **Networking** section. Under **IP addresses**, you will see `This app has no IP addresses`;
3. **Allocate Free IPs (Please follow carefully)**:
   * **Click `Assign Shared IPv4`**: **[REQUIRED! 100% FREE $0.00]** Fly.io's Anycast Shared IPv4 routes incoming connections using TLS SNI, matching the TLS-in-TLS proxy architecture seamlessly.
   * **Click `Assign Dedicated IPv6`**: **[RECOMMENDED! 100% FREE $0.00]** Grants a free dedicated IPv6 address.
   * ⚠️ **DO NOT click `Assign Dedicated IPv4`**: Dedicated IPv4 incurs a charge of $2.00/month and is unnecessary for this project.
   * **Ignore `Assign Flycast IPv6`**: This is for internal 6PN private networks only and cannot be accessed from the public internet.
4. Once allocated, your public IP addresses will be displayed, and your proxy is now accessible from the internet!

---

### Step 4: Configure Custom Domain & TLS Certificates (Optional Anti-Censorship)
Direct `fly.dev` domains are frequently blocked by national firewalls. Setting up a custom domain is strongly recommended:

1. **Add Certificate in Fly.io**:
   * In your app's left sidebar, click **Certificates**;
   * Click **Add a Certificate** in the top right;
   * Enter your subdomain (e.g. `sinel.yourdomain.com`) and click **Create Certificate**;
   * Fly.io will display the required DNS configuration records.
2. **Configure DNS Records**:
   * Log into your DNS provider (e.g., Cloudflare, Namecheap, DNSPod, etc.);
   * Add the following DNS records:
     * **A Record**: Name `sinel` -> Value: Your assigned **Shared IPv4** (If using Cloudflare, **ensure proxy status is set to DNS-Only / gray cloud**).
     * **AAAA Record**: Name `sinel` -> Value: Your assigned **Dedicated IPv6**.
3. **Verify Certificate Status**:
   * Once DNS propagates, the certificate status on the Fly.io Certificates page will turn into a green checkmark (`Ready`) within 1–2 minutes.

---

## Connecting Clients & Sharing Links

Once configured, generate your official standard share link:
```
https://signal.tube/#signal.yourdomain.com
```

**Client Setup**:
1. **Web UI Direct Setup**: Visit `https://sinel.yourdomain.com` in your browser. The built-in Swiss Design Web UI allows you to copy the share link or click "Open in Signal" to configure the proxy automatically.
2. **Manual Setup**: Open Signal App -> **Settings** -> **Data and Storage** -> **Use Proxy** -> Toggle **ON** and enter `sinel.yourdomain.com:443`.

---

## Advanced: Developer CLI Guide

If you prefer using the terminal, you can perform the full setup using `flyctl`:

```bash
# 1. Log in to Fly CLI
fly auth login

# 2. Launch and deploy
fly launch --no-deploy
fly deploy

# 3. Allocate free public IPs and bind custom domain
fly ips allocate-v4 --shared
fly ips allocate-v6
fly certs add signal.yourdomain.com
```

---

## Environment Variables

Customizable via the `[env]` block in `fly.toml` or the **Secrets** tab in the Fly.io console:

| Variable | Default | Description |
| :--- | :--- | :--- |
| `PORT` | `8080` | Internal listening port inside the container |
| `HOST` | `0.0.0.0` | Internal listening IP address |
| `RUST_LOG` | `sinel=info,warn` | Log level (`debug` enables detailed handshake logs) |
| `ALLOW_ALL_SIGNAL_SUBDOMAINS` | `true` | Allows all `*.signal.org` and `*.voip.signal.org` subdomains |
| `EXTRA_ALLOWED_DOMAINS` | *(empty)* | Comma-separated list of additional allowed destination domains |

---

## Local Development & Testing

```bash
# Run unit tests (includes ClientHello parsing and whitelist validation)
cargo test

# Run proxy server locally
cargo run
```

---

## Adapted from

* [Signal-TLS-Proxy](https://github.com/signalapp/Signal-TLS-Proxy)

---

## License

This project is open-source under the [MIT License](LICENSE).
