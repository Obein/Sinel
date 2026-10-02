# Signal TLS Proxy for Fly.io (Rust Edition)

> 一个专为 **Fly.io 免费层级（Free Allowance / $5 豁免政策）** 量身打造的超轻量级 **Signal TLS-in-TLS 混淆反向代理**。使用 Rust 构建，内存占用 < 15MB，免除维护 Nginx 多容器、Let's Encrypt 证书签发与续签的繁琐负担。

---

## 架构与工作原理

本代理精准还原了官方 `Signal-TLS-Proxy` 的核心能力，同时巧妙利用了 Fly.io 的网络基础设施：

```
[ Signal Client 手机端 ]
           │
           │  1. 外层 TLS 连接 (端口 443, SNI: your-domain.com)
           ▼
[ Fly.io Global Edge (443) ]
           │  • 自动配置并续签 Let's Encrypt 证书
           │  • handlers = ["tls"] (解开外层 TLS，不执行任何 HTTP 语法解析)
           ▼
[ Rust 核心代理服务 (:8080) ]
           │  2. 接收解密后的 TCP 裸流 (内含内层 TLS ClientHello)
           │  3. 零拷贝嗅探内层 SNI (如 chat.signal.org)
           │  4. 域名白名单防滥用校验 (*.signal.org)
           ▼
[ Signal 官方服务器 (chat.signal.org:443) ]
           • 内层 TLS 端到端直连加密，代理服务器无法解密任何聊天记录与消息内容
```

### 核心特性
- **纯粹的 Rust 异步实现**：基于 Tokio 异步网络引擎与零拷贝 TLS SNI 解析器，极速全双工数据流转。
- **免除证书维护**：依托 Fly.io 边缘托管证书，无需在容器内跑 `certbot` 或挂载卷。
- **零知识隐私安全**：端到端加密数据在手机与官方服务器之间直接握手，中继服务器无权且无法查看任何内容。
- **防滥用白名单**：内置官方完整服务域（`chat.signal.org`、`storage.signal.org`、`sfu.voip.signal.org` 等），拒绝非 Signal 目标的转发，防止被扫网者作为公开跳板。

---

## 免费层级（Free Tier）成本保障

Fly.io 实行 **"月度账单低于 $5.00 USD 自动全额免单"** 的官方政策：

| 项目 | 配置参数 | 月度预估费用 | 账单结果 |
| :--- | :--- | :--- | :--- |
| **计算实例** | 1 × `shared-cpu-1x` (256MB RAM) 常驻 24/7 | ~$1.94 / 月 | **$0.00**（触发低于 $5 免单） |
| **持久运行** | `auto_stop_machines = false` (永不休眠) | 已包含在上述费用中 | **$0.00** |
| **公网出站流量** | 每月免费赠送 100 GB | $0.00 | **$0.00** |
| **SSL/TLS 证书**| Fly.io 自动化 Let's Encrypt 证书 | 免费 | **$0.00** |

> **提示**：保持 `fly.toml` 中的 `memory = "256mb"` 与单个实例运行，即可永久处于 $0 账单区间。

---

## 部署方案 A：GitHub 仓库 + Actions 自动部署（推荐）

通过 GitHub Actions 持续集成与部署（CI/CD），**本地无需安装 Docker 或编译 Rust**，只要执行 `git push`，GitHub 与 Fly.io 云端就会自动构建并无缝更新服务。

### 1. 将项目推送到你的 GitHub 仓库
在本地项目根目录下执行：
```bash
git add .
git commit -m "feat: initial commit for Fly.io Signal TLS Proxy"
git branch -M main

# 关联你创建的 GitHub 仓库并推送 (请将 URL 替换为你自己的仓库地址)
git remote add origin https://github.com/<你的用户名>/<你的仓库名>.git
git push -u origin main
```

### 2. 在 Fly.io 创建应用并生成 Deploy Token
如果你尚未在本地安装 `flyctl`，可通过 PowerShell 快速安装：
```powershell
iwr https://fly.io/install.ps1 -useb | iex
```
然后登录并创建应用：
```bash
# 1. 登录 Fly.io
fly auth login

# 2. 检查 fly.toml 中的 app 名称（例如改为 signal-proxy-unique-name，需全球唯一）
# 创建对应的 Fly 远程应用实体：
fly apps create <你的应用名称>

# 3. 生成专用的长期部署令牌 (Deploy Token)
fly tokens create deploy -x 999999h
```
终端会输出一段以 `FlyV1 ...` 开头的敏感 Token 字符串，将其复制保存。

### 3. 在 GitHub 仓库添加 Secret
1. 打开你刚推送的 GitHub 仓库页面。
2. 依次点击：**Settings** -> 左侧 **Secrets and variables** -> **Actions**。
3. 点击绿色按钮 **New repository secret**：
   * **Name**: 填写 `FLY_API_TOKEN`
   * **Secret**: 粘贴刚才复制的 Fly Deploy Token
4. 点击 **Add secret** 保存。

### 4. 触发自动构建与发布
* 本仓库已预置自动化工作流文件 [`.github/workflows/fly-deploy.yml`](.github/workflows/fly-deploy.yml)。
* 此时在 GitHub 仓库页面点击 **Actions** 标签页，或本地只要执行 `git push`，就会自动触发流水线。
* GitHub Actions 会通过 Fly.io 官方远程机器完成多阶段 Docker 编译并全自动部署上线！

### 5. 分配独立 IP 与绑定自定义域名（仅需初次配置一次）
在本地终端执行（或通过 Fly.io 控制台操作）：
```bash
# 1. 为你的应用分配专用的 Anycast IPv4 与 IPv6
fly ips allocate-v4 -a <你的应用名称>
fly ips allocate-v6 -a <你的应用名称>

# 2. 添加你的自定义代理域名 (例如 signal.yourdomain.com)
fly certs add signal.yourdomain.com -a <你的应用名称>
```

根据命令返回的 IP 地址，前往你的域名 DNS 控制台（如 Cloudflare / DNSPod / 阿里云 / NameSilo）添加两条解析记录：
* **A 记录**：`signal` -> 指向分配的 IPv4 地址（若使用 Cloudflare，**必须保持灰色云朵 DNS Only**）。
* **AAAA 记录**：`signal` -> 指向分配的 IPv6 地址。

几分钟后运行以下命令检查证书状态：
```bash
fly certs check signal.yourdomain.com -a <你的应用名称>
```
显示 `Valid` 即表示证书与网络完全就绪！

---

## 部署方案 B：本地 CLI 命令行直接部署

如果你习惯在本地直接使用终端操作，也可以通过 `flyctl` 一键发布：

```bash
# 1. 启动创建向导 (选择区域，例如 iad, nrt, hkg, sin 等)
fly launch --no-deploy

# 2. 部署应用 (Fly 远程构建机自动编译 Dockerfile)
fly deploy

# 3. 分配 IP 并配置证书
fly ips allocate-v4
fly ips allocate-v6
fly certs add signal.yourdomain.com
```

---

## 获取分享链接与客户端连接

代理配置生效后，即可生成分享短链：
```
https://signal.tube/#signal.yourdomain.com
```

**客户端使用方式**：
1. **自动配置**：手机端直接点击上述链接，Signal App 会自动唤起并提示“使用代理”。
2. **手动配置**：打开 Signal App -> **设置** -> **数据与存储** -> **使用代理** -> 开启开关并填入 `signal.yourdomain.com:443`。

---

## 环境变量配置

可通过 `fly.toml` 的 `[env]` 区域或 `fly secrets set` 进行自定义：

| 变量名 | 默认值 | 作用说明 |
| :--- | :--- | :--- |
| `PORT` | `8080` | 容器内部监听端口 |
| `HOST` | `0.0.0.0` | 容器内部监听 IP |
| `RUST_LOG` | `signal_tls_proxy_fly=info,warn` | 日志详细级别（可调为 `debug` 查看实时转发日志） |
| `ALLOW_ALL_SIGNAL_SUBDOMAINS` | `true` | 是否允许所有 `*.signal.org` 和 `*.voip.signal.org` 子域，防止官方新增 CDN 节点时断联 |
| `EXTRA_ALLOWED_DOMAINS` | 空 | 额外允许的目标域名（英文逗号分隔） |

---

## 本地开发与测试

```bash
# 运行单元测试 (包含 ClientHello 编解码与白名单测试)
cargo test

# 本地调试运行
cargo run
```
