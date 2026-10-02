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
           • 内层 TLS 端到端直连加密，代理服务器无法解密任何聊天记录与私密数据
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

## 快速开始：Fork 本项目一键部署（推荐）

通过 GitHub Actions 自动化流水线，**你无需在本地安装 Docker 或 Rust 编译环境**，只要 Fork 本仓库并配置凭证，GitHub 与 Fly.io 云端就会自动完成编译与上线。

### 步骤 1：Fork 本仓库
1. 点击本页面右上角的 **Fork** 按钮，将项目复制到你自己的 GitHub 账号下。
2. 进入你 Fork 后的仓库，切换到 **Actions** 标签页，点击绿色按钮 **"I understand my workflows, go ahead and enable them"** 启用 GitHub Actions（GitHub 默认会对 Fork 仓库禁用工作流，必须手动点击一次启用）。

### 步骤 2：在 Fly.io 创建应用并获取 Deploy Token
如果你尚未安装 Fly.io 命令行工具 `flyctl`，可通过终端快速安装：
- **Windows (PowerShell)**:
  ```powershell
  iwr https://fly.io/install.ps1 -useb | iex
  ```
- **macOS / Linux**:
  ```bash
  curl -L https://fly.io/install.sh | sh
  ```

安装后登录并创建应用实体：
```bash
# 1. 登录 Fly.io (新注册用户需绑定一张外币信用卡进行反滥用验资，月账单低于 $5 不产生扣费)
fly auth login

# 2. 创建一个属于你的专属应用实体 (应用名全球唯一，如 my-signal-proxy-2026)
fly apps create <你的应用名称>

# 3. 为该应用生成专用的长期部署令牌 (Deploy Token)
fly tokens create deploy -a <你的应用名称> -x 999999h
```
终端会输出一段以 `FlyV1 ...` 开头的敏感 Token 字符串，将其复制备用。

> *注：你也可以直接在 [Fly.io Web 控制台](https://fly.io/dashboard) 界面上点击 "Create App" 并生成 Token。*

### 步骤 3：在 Fork 仓库中配置 Secret 与 Variable
打开你 Fork 后的 GitHub 仓库：

1. **添加部署 Token (Secret)**：
   * 前往 **Settings** -> 左侧 **Secrets and variables** -> **Actions** -> **Repository secrets**；
   * 点击 **New repository secret**：
     * **Name**: 填写 `FLY_API_TOKEN`
     * **Secret**: 粘贴刚才复制的 Token
   * 点击 **Add secret** 保存。

2. **配置应用名称 (Variable，二选一)**：
   * **方法 A（无需修改代码，推荐）**：在同一页面的 **Variables** 标签页中点击 **New repository variable**，添加：
     * **Name**: `FLY_APP_NAME`
     * **Value**: 填写你在步骤 2 中创建的应用名称
   * **方法 B（直接改代码）**：修改仓库根目录下的 `fly.toml` 文件第 8 行，将 `app = "signal-tls-proxy-fly"` 改为你创建的应用名称并提交。

### 步骤 4：触发自动部署
进入仓库的 **Actions** 标签页：
1. 点击左侧工作流 **"Deploy to Fly.io"**。
2. 点击右侧下拉菜单 **"Run workflow"** -> 点击绿色按钮 **"Run workflow"**。
3. GitHub Actions 将自动唤起 Fly.io 远程机器进行编译、多阶段打包并秒级完成部署！以后仓库代码有任何更新，也会在 push 时自动更新。

### 步骤 5：分配独立 IP 与绑定自定义域名
部署完成后，需要为代理分配 Anycast IP 并绑定你自己的域名（只需配置一次）：

在本地终端执行（或在 Fly.io 控制台页面操作）：
```bash
# 1. 为你的应用分配专用的 Anycast IPv4 与 IPv6
fly ips allocate-v4 -a <你的应用名称>
fly ips allocate-v6 -a <你的应用名称>

# 2. 将你的域名添加至 Fly.io 证书托管 (例如 signal.yourdomain.com)
fly certs add signal.yourdomain.com -a <你的应用名称>
```

根据命令返回的 IP 地址，前往你的域名 DNS 控制台（如 Cloudflare / DNSPod / 阿里云 / NameSilo）添加两条解析记录：
* **A 记录**：`signal` -> 指向分配的 IPv4 地址（若使用 Cloudflare，**必须保持灰色云朵 DNS Only**）。
* **AAAA 记录**：`signal` -> 指向分配的 IPv6 地址。

等待 1~2 分钟，运行以下命令验证证书签发状态：
```bash
fly certs check signal.yourdomain.com -a <你的应用名称>
```
显示 `Valid` 即表示证书与网络中继已完全就绪！

---

## 进阶：本地 CLI 命令行直接开发与部署

如果你克隆了代码到本地，习惯直接使用终端进行开发和发布：

```bash
# 1. 启动创建向导 (选择部署区域，例如 iad, nrt, hkg, sin 等)
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

代理配置生效后，即可生成官方标准分享短链：
```
https://signal.tube/#signal.yourdomain.com
```

**客户端使用方式**：
1. **自动配置**：手机端直接点击上述短链，Signal App 会自动弹出并提示“使用代理”。
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

## 本地开发与单元测试

```bash
# 运行单元测试 (包含 ClientHello 编解码与白名单测试)
cargo test

# 本地调试运行
cargo run
```
