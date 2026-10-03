# Sinel: Signal TLS Proxy for Fly.io (Rust Edition)

[English](README.md) | 中文

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
           │  2. 接收解密后的 TCP 裸流 (内含内层 TLS ClientHello 或 Web HTTP 请求)
           │  3. 零拷贝嗅探首包协议：
           │     ├─ 若为 TLS (0x16)：提取内层 SNI (如 chat.signal.org) 并通过白名单校验
           │     └─ 若为普通 HTTP (GET /)：接管并返回极简瑞士风格 Web UI
           ▼
[ Signal 官方服务器 (chat.signal.org:443) ]
           • 内层 TLS 端到端直连加密，代理服务器无法解密任何聊天记录与消息内容
```

### 核心特性
- **纯粹的 Rust 异步实现**：基于 Tokio 异步网络引擎与零拷贝 TLS SNI 解析器，极速全双工数据流转。
- **内置瑞士风格 Web 引导页 (Swiss Design UI)**：浏览器直接访问域名时，自动呈现现代瑞士国际排版风格引导页，支持中英双语切换，动态提取当前主机名生成 `signal.tube/#<your_host>` 链接，支持一键复制与唤起 Signal 客户端。
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

> **提示**：保持机器配置为 `shared-cpu-1x` + `256MB` 且单个实例常驻，即可永久处于 $0 账单区间。

---

## 快速开始：Fork 一键网页直连部署（最推荐，零门槛、无需 Token）

> 💡 **无需安装任何工具（无 `flyctl`、无 Docker、无 Rust 编译环境）**，整个过程通过 GitHub 与 Fly.io 网页控制台完成，且**无需生成或填写任何 Token**。

### 步骤 1：Fork 本仓库到你的账号
点击本页面右上角的 **Fork** 按钮，将项目完整复制到你自己的 GitHub 账号下。

---

### 步骤 2：在 Fly.io 网页控制台一键直连部署
1. 访问并登录 [Fly.io 控制台 (https://fly.io/dashboard)](https://fly.io/dashboard)（新用户需在个人设置中绑定一张外币信用卡验资，月账单低于 $5 实际扣费为 $0）。
2. 在控制台点击 **Launch an app from GitHub**（从现有 GitHub 仓库创建应用）。
3. 授权并选中你刚才 Fork 的仓库，页面会展示部署表单，对照以下要求填写：

| 表单配置项 | 推荐填写值 | 说明与注意事项 |
| :--- | :--- | :--- |
| **App Name** | 自定义唯一名称 | 例如 `my-signal-proxy-2026`（全局唯一） |
| **Region** | **`sin - Singapore, Singapore`** 或 **`nrt - Tokyo, Japan`** | 中国大陆境内及亚太地区访问延迟较低；美西可备选 `sjc - San Jose` 或 `lax - Los Angeles`。 |
| **Internal port** | **`8080`** | **重点** 必须填写 `8080`（我们的 Rust 代理服务在容器内监听 8080 端口）。 |
| **Machine Sizes**<br>• CPU(s)<br>• Memory | <br>**`shared-cpu-1x`**<br>**`256MB`** | **免费层关键保障。** 保持该最低规格，月度计费仅约 $1.94，自动触发 Fly.io 低于 $5 全额免单政策。 |
| **Environment Variables** | 留空（无需添加） | 代码中已全部内置合理的默认参数（包含官方全量域名白名单）。 |
| **Database** | **不勾选**（Managed Postgres） | 纯内存流式网络代理，不需要任何数据库。 |
| **Working directory** | 留空（默认 `./`） | 默认即可。 |
| **Config path** | 留空（默认 `./fly.toml`） | 默认即可，Fly.io 会自动读取项目中的 `fly.toml` 挂载 443 端口 `handlers = ["tls"]` 外层解密规则。 |

4. 检查无误后，直接点击底部的紫色 **Deploy** 按钮。Fly.io 云端会自动拉取你的仓库并编译上线。

---

### 步骤 3：在 Fly.io 网页端分配公网 IP 地址（关键）
新创建的 Fly 应用默认没有分配公网 IP 地址（因此 `fly.dev` 域名在分配 IP 前是无法从互联网解析访问的）：

1. 在 [Fly.io Dashboard](https://fly.io/dashboard) 点击进入刚创建的应用 **Overview（概览）** 页面；
2. 向下滚动到 **Networking** 区域，在 **IP addresses** 下方会看到 `This app has no IP addresses`；
3. **点击分配免费 IP（请仔细对照选择）**：
   * **点击 `Assign Shared IPv4`**：**【必点，完全免费 $0.00】** Fly.io 官方提供的 Anycast 共享 IPv4，它是基于 TLS 握手中的 SNI（域名）来识别并路由流量的，与本项目的 TLS-in-TLS 架构完美契合。
   * **点击 `Assign Dedicated IPv6`**：**【建议点击，完全免费 $0.00】** 免费获得一个独立的专用 IPv6 地址。
   * **切勿点击 `Assign Dedicated IPv4`**：这是独立独享 IPv4，每月需收取 $2.00 费用，本项目完全不需要。
   * **忽略 `Assign Flycast IPv6`**：这是 Fly 平台内部私有网络专用的，外部公网无法访问。
4. 分配完成后，下方会立即列出生成的公共 IP 地址，此时你的应用正式获得公网入口。

---

### 步骤 4：在 Fly.io 网页端绑定自定义域名与证书（可选抗封锁）
直接使用 `fly.dev` 公共域名在一些地区可能会被防火墙列入黑名单，可以**绑定你自己的独立域名**才能更好起到伪装效果：

1. **添加域名**：
   * 在应用左侧菜单栏点击 **Certificates**；
   * 点击右上角紫色按钮 **Add a Certificate**；
   * 输入你的二级域名（例如 `sinel.yourdomain.com`），点击 **Create Certificate**；
   * 页面会给出详细的 DNS 配置指令（包含需要指向的 IP 地址或 CNAME 值）。
2. **设置域名 DNS 解析**：
   * 前往你的域名 DNS 服务商（如 Cloudflare / DNSPod / 阿里云 / NameSilo 等）；
   * 添加两条解析记录：
     * **A 记录**：主机记录填 `sinel` -> 记录值填刚才分配的 **Shared IPv4**（若使用 Cloudflare，**必须保持灰色云朵 DNS Only**）。
     * **AAAA 记录**：主机记录填 `sinel` -> 记录值填刚才分配的 **Dedicated IPv6**。
3. **验证证书就绪**：
   * DNS 生效后，Fly.io 网页端 Certificates 页面的域名状态会在 1~2 分钟内变为绿色勾选的 `Ready`，说明外层 TLS 证书已自动就绪。

---

## 获取分享链接与客户端连接

代理配置生效后，即可生成官方标准分享短链：
```
https://signal.tube/#sinel.yourdomain.com
```

**客户端使用方式**：
1. **网页直达 / 自动配置**：浏览器访问 `https://sinel.yourdomain.com`，使用内置的 Swiss Design Web UI 一键复制链接或点击“在客户端中打开”自动唤醒 Signal App。
2. **手动配置**：打开 Signal App -> **设置** -> **数据与存储** -> **使用代理** -> 开启开关并填入 `sinel.yourdomain.com:443`。

---

## 进阶：终端开发者本地 CLI 操作指引

如果你习惯使用终端命令行，也可以通过 `flyctl` 完成所有步骤：

```bash
# 1. 登录终端
fly auth login

# 2. 本地直接发布 (可选择 sin 或 nrt 区域)
fly launch --no-deploy
fly deploy

# 3. 分配免费 IP 并配置证书
fly ips allocate-v4 --shared
fly ips allocate-v6
fly certs add signal.yourdomain.com
```

---

## 环境变量配置

可通过 `fly.toml` 的 `[env]` 区域或 Fly 控制台 **Secrets** 页面进行自定义：

| 变量名 | 默认值 | 作用说明 |
| :--- | :--- | :--- |
| `PORT` | `8080` | 容器内部监听端口 |
| `HOST` | `0.0.0.0` | 容器内部监听 IP |
| `RUST_LOG` | `sinel=info,warn` | 日志详细级别（可调为 `debug` 查看实时转发日志） |
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

---

## 迁移自

* [Signal-TLS-Proxy](https://github.com/signalapp/Signal-TLS-Proxy)

---

## 许可证 (License)

本项目采用 [MIT 许可证](LICENSE)。
