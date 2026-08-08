# EchoMind Obsidian 模式落地规划 — 从零到盈利的详细执行方案

> 制定日期：2026-07-30
> 目标：18 个月内达到月收入 $3,000+（年化 $36,000+），实现可持续盈利
> 核心策略：免费核心 + 增值服务（Sync/Publish）+ 买断 Pro + 商业许可
> 参照：Obsidian（年营收数千万美元，100% 用户支持，无 VC）

---

## 总体架构

```
┌──────────────────────────────────────────────────────────────────────┐
│                     EchoMind 商业架构（Obsidian 模式）                 │
│                                                                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────┐  ┌─────────────┐ │
│  │  核心应用     │  │  Pro 买断    │  │  Sync    │  │  Publish    │ │
│  │  完全免费     │  │  $39 一次性  │  │  $4/月   │  │  $8/月      │ │
│  │  MIT 开源    │  │  可选        │  │  可选    │  │  可选       │ │
│  │              │  │              │  │          │  │             │ │
│  │  全格式导入  │  │  多模态 OCR  │  │  跨设备  │  │  知识库     │ │
│  │  200 文件   │  │  VLM 图表    │  │  加密同步│  │  发布为     │ │
│  │  混合检索    │  │  文档审计    │  │  版本历史│  │  可搜索     │ │
│  │  富内容渲染  │  │  HNSW 索引   │  │          │  │  Web 站点   │ │
│  │  BYOK 对话   │  │  无限文件    │  │          │  │             │ │
│  └──────────────┘  └──────────────┘  └──────────┘  └─────────────┘ │
│     免费层（获客）     买断层（变现）      订阅层（持续收入）          │
│                                                                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐  │
│  │  Catalyst    │  │  Commercial  │  │  内容变现                 │  │
│  │  $25 一次性  │  │  $50/用户/年 │  │  课程/咨询/培训           │  │
│  │  支持开发    │  │  商业使用    │  │  $199-999/件             │  │
│  │  Beta 通道   │  │              │  │                          │  │
│  │  社区徽章    │  │              │  │                          │  │
│  └──────────────┘  └──────────────┘  └──────────────────────────┘  │
│     赞助层（情怀）     企业层（B2B）        生态层（杠杆）            │
└──────────────────────────────────────────────────────────────────────┘
```

---

## Phase 1：冷启动基建（第 1-4 周）

> **目标**：让用户能找到、能下载、能用起来。零收入，但建立全部基础设施。

### 1.1 调整免费版功能边界（代码改动）

**当前状态**（`crates/core/src/import.rs:21`）：
```rust
pub const FREE_TIER_MAX_FILES: usize = 50;
pub const ALLOWED_EXTENSIONS: [&str; 3] = ["md", "txt", "pdf"];
```

**目标状态**：
```rust
pub const FREE_TIER_MAX_FILES: usize = 200;  // 50 → 200
// 格式白名单不变，但 PDF 在 Free 版也允许（仅文本提取层，不含 OCR/VLM）
```

**具体改动文件与逻辑**：

#### 改动 1：`crates/core/src/import.rs` — 提高 Free 文件上限

文件：`crates/core/src/import.rs`
位置：第 21 行
改动：`FREE_TIER_MAX_FILES` 从 50 改为 200

#### 改动 2：`crates/core/src/import.rs` — Free 版允许 PDF 文本提取

当前逻辑：PDF 导入时检查 `is_pro`，非 Pro 直接返回 `ERR_PRO_REQUIRED`。

需要改为：Free 版允许 PDF 文本层提取（`MarkdownPdfLoader`），Pro 版才解锁多模态（OCR + VLM）。

具体位置：`import_one()` 方法中的 PDF 付费门逻辑。

#### 改动 3：`crates/tauri-app/src/commands.rs` — Pro 功能边界调整

当前 Pro 专属功能：
- PDF 导入（需改为 Free 也可文本提取）
- 多模态 OCR/VLM（保持 Pro 专属）
- 文档审计（保持 Pro 专属）
- HNSW 索引（保持 Pro 专属）
- 无限文件（保持 Pro 专属）

### 1.2 官网落地页

**技术方案**：纯静态 HTML，托管在 Cloudflare Pages（免费）

**文件结构**：
```
website/
  index.html       — 落地页（英文 + 中文切换）
  assets/
    screenshot.png — 应用截图
    demo.gif       — 30 秒 Demo 动图
    icon.svg       — Logo
  CNAME            — echomind.app
```

**落地页内容结构**：
1. Hero 区：标题 + 一句话价值主张 + Download 按钮
2. 功能区：6 个核心功能卡片（图标 + 标题 + 描述）
3. 对比区：与 Cherry Studio/AnythingLLM/Open WebUI 的对比表
4. 定价区：Free / Pro / Sync / Publish 四列
5. FAQ 区：10 个常见问题
6. Footer：GitHub 链接、社区链接、联系方式

### 1.3 GitHub Actions 发布 CI

**文件**：`.github/workflows/release.yml`

**功能**：
- 当推送 `v*` tag 时触发
- 三平台并行构建：macOS（Universal）、Windows（x64）、Linux（x64）
- 自动上传到 GitHub Releases
- 生成 SHA256 校验和

**关键配置**：
```yaml
strategy:
  matrix:
    include:
      - platform: macos-latest
        args: --target aarch64-apple-darwin --target x86_64-apple-darwin
      - platform: ubuntu-22.04
        args: ''
      - platform: windows-latest
        args: ''
```

### 1.4 README 优化

当前 README 缺少：
- 截图/GIF Demo
- 下载链接（直接指向 GitHub Releases）
- 社区链接（Discord/微信群）
- Star History

### 1.5 社区基建

| 平台 | 动作 | 预计耗时 |
|---|---|---|
| Discord | 创建服务器，设置 #general #support #feedback #dev-log 频道 | 1 小时 |
| 微信群 | 创建微信群，生成邀请二维码 | 30 分钟 |
| GitHub Discussions | 开启 Discussions，设置 Announcements/Q&A/Ideas/Show & Tell 分类 | 30 分钟 |
| Twitter/X | 创建 @EchoMindApp 账号，发布第一条推文 | 30 分钟 |

### 1.6 首次发布检查清单

- [ ] `cargo test` 全部通过
- [ ] `cargo clippy --all-targets -- -D warnings` 零警告
- [ ] `cargo build --release` 三平台成功
- [ ] GitHub Release 包含：安装包 + SHA256 + Release Notes
- [ ] 官网上线并可访问
- [ ] README 包含下载链接和截图
- [ ] Discord/微信群可加入
- [ ] V2EX 帖子草稿就绪
- [ ] Hacker News Show HN 帖子草稿就绪

---

## Phase 2：首次发布与冷启动（第 5-8 周）

> **目标**：GitHub Star 1000+，下载量 5000+，Discord 用户 300+

### 2.1 发布渠道与时间表

| 时间 | 渠道 | 内容 | 预期效果 |
|---|---|---|---|
| 第 5 周 周一 | GitHub Release | v0.1.0 正式发布 | 基准 |
| 第 5 周 周一 | V2EX | "用 Rust + Tauri 做了个本地 AI 知识库，零服务器成本，隐私不出域" | 2000-5000 浏览 |
| 第 5 周 周二 | Hacker News | "Show HN: EchoMind – Local-first AI RAG Knowledge Base in Rust" | 3000-10000 浏览 |
| 第 5 周 周三 | Reddit | r/rust + r/selfhosted + r/LocalLLaMA | 1000-3000 浏览 |
| 第 5 周 周四 | 少数派 | 深度文章 "隐私优先的 AI 知识库：EchoMind" | 2000-5000 浏览 |
| 第 5 周 周五 | Product Hunt | 精心策划的发布 | 500-2000 浏览 |
| 第 6 周 | B站 | 10 分钟上手视频 | 1000-3000 浏览 |
| 第 6 周 | 即刻 | 产品发布动态 | 500-1000 浏览 |

### 2.2 内容素材准备

#### 素材 1：V2EX/HN 帖子模板

标题：EchoMind — 用 Rust 做的本地 AI 知识库（隐私不出域，BYOK，买断制）

正文结构：
1. 一句话说清产品
2. 为什么做这个（痛点）
3. 技术亮点（Rust + Tauri 15MB vs Electron 150MB）
4. 与竞品对比（一句话差异化）
5. 开源地址 + 下载链接
6. 截图/GIF

#### 素材 2：30 秒 Demo GIF

录制内容：
1. 拖入 Markdown 文件 → 自动索引
2. 输入问题 → 流式回答 + 引用来源
3. 切换 LLM（DeepSeek → OpenAI）

工具：macOS 用 QuickTime 录屏 → ffmpeg 转 GIF

#### 素材 3：对比表

| | EchoMind | Cherry Studio | AnythingLLM | Open WebUI |
|---|---|---|---|---|
| 安装包大小 | 15MB | 150MB+ | 150MB+ | Docker 镜像 |
| 服务器成本 | $0 | $0 | 需云服务 | 需自部署 |
| 隐私 | 全本地 | 全本地 | 部分云端 | 部分云端 |
| 定价 | Free + $39 买断 | 免费 | $20/月起 | 免费 |

### 2.3 反馈闭环建立

| 渠道 | 响应时间目标 | 工具 |
|---|---|---|
| GitHub Issues | 24 小时内首次回复 | GitHub Notifications |
| Discord | 2 小时内 | Discord 移动端通知 |
| 微信群 | 4 小时内 | 微信 |
| V2EX/Reddit | 12 小时内 | 手动检查 |

### 2.4 首批用户访谈

目标：在第 5-8 周内完成 20 个用户深度访谈

问题清单：
1. 你之前用什么工具管理知识库？
2. 你为什么选择下载 EchoMind？
3. 第一次使用时卡在哪里了？
4. 什么功能让你觉得最有价值？
5. 什么功能让你觉得缺失？
6. 你愿意为哪些功能付费？
7. 你能接受的定价区间？
8. 你会推荐给朋友吗？为什么？

---

## Phase 3：Pro 买断变现（第 9-16 周）

> **目标**：Pro 买断收入 $500/月，累计付费用户 50+

### 3.1 支付系统搭建

#### 方案 A：Lemon Squeezy（国际，推荐）

**原因**：
- 支持 PayPal + 信用卡
- 自动处理 VAT/Tax
- 支持中国区定价
- Merchant of Record（帮你处理全球税务）
- API 可自动签发 License Key

**流程**：
```
用户点击 "升级 Pro"
  → 跳转 Lemon Squeezy 支付页
  → 支付成功 → Webhook 通知后端
  → 后端用 Ed25519 私钥签发 License Key
  → 邮件发送 License Key 给用户
  → 用户在 EchoMind 中输入 Key 激活
```

#### 方案 B：爱发电 + 手动签发（国内，初期）

**原因**：
- 支持微信/支付宝
- 无需企业资质
- 手动签发 License Key（初期量小可接受）

**流程**：
```
用户在爱发电支付 ¥99
  → 留言邮箱
  → 手动用 license-issuer CLI 签发 License Key
  → 邮件发送给用户
```

#### License 签发服务端

**技术方案**：Cloudflare Workers（免费额度 10 万次/天）

**文件结构**：
```
license-server/
  worker.js         — Cloudflare Worker 主逻辑
  wrangler.toml     — Cloudflare 配置
  package.json      — 依赖
```

**Worker 逻辑**：
1. 接收 Lemon Squeezy Webhook
2. 验证签名
3. 用 Ed25519 私钥签发 License（复用 `crates/license-issuer` 的逻辑，用 JS 重写或 WASM）
4. 存储订单记录到 Cloudflare KV
5. 发送包含 License Key 的邮件（用 Mailchannels API，免费）

### 3.2 Pro 功能价值强化

当前 Pro 功能：
- ✅ PDF 多模态（OCR + VLM）
- ✅ 文档审计
- ✅ HNSW 索引
- ✅ 无限文件

需要新增的 Pro 价值感知功能：

| 功能 | 说明 | 优先级 |
|---|---|---|
| **多知识库工作空间** | 支持创建多个独立知识库，按主题隔离 | P0 |
| **批量导出** | 导出对话为 Markdown/PDF | P0 |
| **高级分块策略** | SectionAwareSplitter 作为 Pro 功能 | P1 |
| **自定义系统提示词** | 用户可自定义 RAG 系统提示词 | P1 |
| **对话模板** | 预设问答模板，一键启动特定类型对话 | P2 |

### 3.3 定价实施

| 版本 | 国际价格 | 中国价格 | 学生价格 |
|---|---|---|---|
| Free | $0 | ¥0 | ¥0 |
| Pro 早鸟（前 500 名） | $19 | ¥39 | ¥9 |
| Pro 正式 | $39 | ¥99 | ¥19 |
| Catalyst | $25 | ¥50 | — |
| Commercial | $50/用户/年 | ¥350/用户/年 | — |

### 3.4 付费墙优化

当前付费墙（`ui/src/paywall.js`）只显示输入 License Key。

需要优化为：
1. **价值展示**：付费墙顶部展示 Pro 功能价值（3 个 bullet point + 截图）
2. **定价选择**：Free $0 / Pro $39 / Catalyst $25 三列对比
3. **购买入口**："立即购买"按钮跳转支付页面
4. **激活入口**："已有 License？输入激活"折叠区域
5. **早鸟倒计时**：前 500 名早鸟价 + 剩余名额

---

## Phase 4：EchoMind Cloud Sync（第 17-28 周）

> **目标**：Sync 订阅收入 $500/月，订阅用户 150+

这是整个盈利模型的核心——**持续可预测的订阅收入**。

### 4.1 同步服务架构

```
┌─────────────────────────────────────────────────────────────┐
│                   EchoMind Cloud Sync 架构                   │
│                                                             │
│  客户端（EchoMind 桌面应用）                                  │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  Sync Manager（新增 Rust 模块）                       │   │
│  │  1. 检测本地数据变更（SQLite WAL 变更监听）            │   │
│  │  2. 生成加密变更包（AES-256-GCM 端到端加密）          │   │
│  │  3. 上传到 Sync Server                               │   │
│  │  4. 拉取其他设备的变更并合并                          │   │
│  └─────────────────────────────────────────────────────┘   │
│                          ↕ HTTPS                            │
│  服务端（Cloudflare Workers + R2）                           │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  Sync API（Cloudflare Workers）                      │   │
│  │  - POST /sync/push   — 接收加密变更包                 │   │
│  │  - GET  /sync/pull   — 返回变更包列表                 │   │
│  │  - POST /sync/ack    — 确认已应用的变更               │   │
│  │  - GET  /sync/status — 返回同步状态                   │   │
│  │  存储：Cloudflare R2（零出口费用）                    │   │
│  │  元数据：Cloudflare KV（用户状态、版本号）             │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  安全保证：                                                  │
│  - 客户端生成 AES-256 密钥，密钥永不出客户端                 │
│  - 服务端只存储加密后的数据，无法解密                        │
│  - 传输层 HTTPS + 应用层 AES-256-GCM 双重加密               │
└─────────────────────────────────────────────────────────────┘
```

### 4.2 同步数据范围

| 数据 | 是否同步 | 说明 |
|---|---|---|
| documents 表 | ✅ | 文档元数据 |
| chunks 表 | ✅ | 分块内容 |
| embeddings 表 | ❌ | 向量太大，新设备本地重新嵌入 |
| conversations 表 | ✅ | 对话历史 |
| messages 表 | ✅ | 消息内容 |
| settings 表 | ⚠️ | 仅同步非敏感项（locale, hybrid_search），不同步 api_key |
| 原始文件 | ✅ | 文档副本（压缩后上传） |

### 4.3 客户端实现（Rust）

#### 新增 crate：`crates/sync`

```
crates/sync/
  Cargo.toml
  src/
    lib.rs           — SyncManager 主逻辑
    crypto.rs        — AES-256-GCM 端到端加密
    delta.rs         — 增量变更生成（diff SQLite 数据）
    transport.rs     — HTTPS 传输层（reqwest）
    merge.rs         — 变更合并（CRDT-like 冲突解决）
    store.rs         — 本地同步状态跟踪
```

#### 新增 IPC 命令（`crates/tauri-app/src/commands.rs`）

```rust
// 同步相关命令
pub async fn sync_login(email: String, password: String, state: State<'_, AppState>) -> Result<SyncStatus, String>
pub async fn sync_logout(state: State<'_, AppState>) -> Result<(), String>
pub async fn sync_now(state: State<'_, AppState>) -> Result<SyncResult, String>
pub async fn sync_get_status(state: State<'_, AppState>) -> Result<SyncStatus, String>
pub async fn sync_enable(state: State<'_, AppState>) -> Result<(), String>
pub async fn sync_disable(state: State<'_, AppState>) -> Result<(), String>
pub async fn sync_get_devices(state: State<'_, AppState>) -> Result<Vec<DeviceInfo>, String>
```

#### 新增前端模块（`ui/src/sync.js`）

```javascript
// 同步设置面板
export function initSync() { ... }
export function openSyncSettings() { ... }
export function showSyncStatus(status) { ... }
export function showSyncConflict(resolution) { ... }
```

### 4.4 服务端实现（Cloudflare Workers）

#### 文件结构

```
sync-server/
  src/
    index.ts         — Worker 入口
    auth.ts          — 用户认证（JWT）
    sync.ts          — 同步逻辑
    storage.ts       — R2/KV 存储抽象
    payment.ts       — 订阅状态验证（Lemon Squeezy API）
  wrangler.toml      — Cloudflare 配置
  package.json
  tsconfig.json
```

#### API 设计

```
POST /api/auth/register    — 注册（email + password → JWT）
POST /api/auth/login       — 登录
POST /api/auth/verify      — 验证 JWT + 订阅状态

POST /api/sync/push        — 上传加密变更包
  Body: { device_id, changeset: encrypted_blob, version: u64 }
  Response: { accepted: bool, server_version: u64 }

GET  /api/sync/pull?since=1234567890  — 拉取变更
  Response: { changesets: [...], latest_version: u64 }

POST /api/sync/ack         — 确认变更已应用
  Body: { version: u64 }

GET  /api/sync/status      — 同步状态
  Response: { last_sync, device_count, storage_used, plan }

GET  /api/devices          — 设备列表
DELETE /api/devices/:id    — 移除设备
```

#### 成本估算

| 资源 | 免费额度 | 预估使用 | 月成本 |
|---|---|---|---|
| Workers 请求 | 10 万次/天 | 5000 次/天（500 用户 × 10 次） | $0 |
| R2 存储 | 10 GB | 5 GB（500 用户 × 10MB） | $0 |
| R2 操作 | 100 万 A 类/月 | 5 万 | $0 |
| KV 读取 | 1000 万/月 | 15 万 | $0 |
| KV 写入 | 100 万/月 | 5 万 | $0 |

**结论**：500 用户以内，服务端成本 = $0

### 4.5 订阅验证流程

```
用户在 Lemon Squeezy 订阅 Sync ($4/月)
  → Webhook 通知 → 更新用户订阅状态
  → 客户端 Sync 启动时验证订阅状态
  → 订阅有效 → 允许同步
  → 订阅过期 → 提示续费，本地数据保留
```

### 4.6 同步 UI 设计

设置面板新增"同步"标签页：
1. 登录/注册区域
2. 同步状态指示器（上次同步时间、设备数、存储用量）
3. 立即同步按钮
4. 设备列表（设备名 + 最后同步时间 + 移除按钮）
5. 端到端加密说明（"你的数据使用 AES-256 加密，服务器无法解密"）
6. 订阅管理（续费/取消）

---

## Phase 5：EchoMind Publish（第 29-40 周）

> **目标**：Publish 订阅收入 $300/月，订阅用户 50+

### 5.1 Publish 功能

将知识库发布为可搜索的静态 Web 站点。

**流程**：
```
用户选择"发布知识库"
  → 选择要发布的文档/会话
  → EchoMind 生成静态站点（HTML + JS 搜索索引）
  → 上传到 Cloudflare Pages
  → 返回可访问的 URL（如 echomind.page/username/kb-name）
  → 可选：绑定自定义域名
```

### 5.2 技术方案

#### 静态站点生成器（客户端 Rust）

新增 `crates/publisher`：
```rust
pub struct SiteGenerator {
    output_dir: PathBuf,
}

impl SiteGenerator {
    pub fn generate(&self, documents: &[Document], chunks: &[Chunk]) -> Result<PathBuf> {
        // 1. 生成 index.html（搜索界面）
        // 2. 生成 search-index.json（chunks 的 MiniSearch 索引）
        // 3. 生成 doc pages（每个文档一个 HTML 页面）
        // 4. 生成 sitemap.xml
    }
}
```

#### 服务端（Cloudflare Pages）

- 用户发布时，Worker 接收静态文件
- 存储到 R2
- 通过 Cloudflare Pages 部署
- 返回 `username.echomind.page` 子域名

---

## Phase 6：内容变现与生态（第 41-72 周）

> **目标**：内容收入 $1000/月，生态贡献者 10+

### 6.1 技术博客（SEO 流量）

**目标**：每月 2-4 篇高质量技术文章

**文章主题清单**（已排序优先级）：

| 优先级 | 标题 | 目标关键词 | 预估流量 |
|---|---|---|---|
| P0 | "用 Rust + Tauri v2 构建 AI 桌面应用完全指南" | rust tauri tutorial | 3000/月 |
| P0 | "本地 RAG 知识库架构设计：从零到一" | local rag architecture | 2000/月 |
| P0 | "EchoMind vs AnythingLLM vs Cherry Studio 深度对比" | anythingllm alternative | 1500/月 |
| P1 | "用 DeepSeek API + EchoMind 构建私有知识库" | deepseek rag | 2000/月 |
| P1 | "fastembed + ONNX：在 Rust 中本地嵌入向量" | rust embedding onnx | 1000/月 |
| P1 | "SQLite 向量存储：不需要专用向量数据库的 RAG" | sqlite vector store | 1500/月 |
| P2 | "Tauri v2 系统菜单与 IPC 命令最佳实践" | tauri v2 menu | 800/月 |
| P2 | "Ed25519 离线 License 验证实现" | rust license verification | 500/月 |

### 6.2 视频教程

| 平台 | 内容 | 频率 |
|---|---|---|
| B站 | "10 分钟上手 EchoMind" | 一次性 |
| B站 | "用 Rust 构建 AI 知识库"系列（10 集） | 每月 1-2 集 |
| YouTube | 英文版同上 | 每月 1-2 集 |

### 6.3 付费课程

**课程**："用 Rust + Tauri 构建 AI 桌面应用 — 从零到上线"

**定价**：¥299（早鸟 ¥199）

**内容**（20 节，每节 15-30 分钟）：
1. Rust + Tauri v2 开发环境搭建
2. 六边形架构设计：models → core → infra → app
3. 文档加载器实现（Markdown/Text/PDF）
4. Token-based 文本分块器
5. fastembed + ONNX 本地嵌入
6. SQLite 向量存储与余弦相似度搜索
7. OpenAI 兼容 SSE 流式对话
8. RAG 检索增强生成完整管线
9. 混合检索：向量 + 关键词 RRF 融合
10. Tauri IPC 命令设计
11. 前端 SPA：无框架 vanilla JS
12. 富内容渲染：Markdown + Mermaid + KaTeX + Chart.js
13. Ed25519 离线 License 验证
14. 多模态 PDF 处理（OCR + VLM）
15. 文档一致性审计引擎
16. 跨平台构建与发布 CI
17. 端到端加密同步服务
18. 静态站点发布
19. 国际化（i18n）实现
20. 从 0 到盈利：独立软件变现实战

### 6.4 插件系统

**Phase 6 后期**：开放插件 API，让社区贡献：
- 文档加载器插件（DOCX/PPTX/Epub...）
- 分块策略插件
- 主题插件
- LLM Provider 插件

---

## 财务模型

### 月收入预估（18 个月路径）

| 月份 | 下载量 | MAU | Pro 买断 | Sync 订阅 | Publish | Catalyst | 内容 | 月收入 |
|---|---|---|---|---|---|---|---|---|
| M1 | 1000 | 300 | $0 | $0 | $0 | $0 | $0 | $0 |
| M2 | 2000 | 600 | $0 | $0 | $0 | $25 | $0 | $25 |
| M3 | 3000 | 1000 | $200 | $0 | $0 | $50 | $0 | $250 |
| M4 | 4000 | 1500 | $400 | $0 | $0 | $75 | $0 | $475 |
| M6 | 8000 | 3000 | $800 | $0 | $0 | $100 | $200 | $1,100 |
| M9 | 15000 | 6000 | $1,200 | $400 | $0 | $150 | $400 | $2,150 |
| M12 | 25000 | 10000 | $1,500 | $1,000 | $300 | $200 | $600 | $3,600 |
| M15 | 40000 | 15000 | $2,000 | $1,800 | $600 | $250 | $800 | $5,450 |
| M18 | 60000 | 25000 | $2,500 | $3,000 | $1,200 | $300 | $1,000 | $8,000 |

### 月成本预估

| 资源 | M1-6 | M7-12 | M13-18 |
|---|---|---|---|
| Cloudflare（Workers+R2+KV+Pages） | $0 | $5 | $20 |
| 域名 | $1 | $1 | $1 |
| Lemon Squeezy 手续费（5%+50¢） | $0 | $80 | $300 |
| 邮件服务 | $0 | $0 | $20 |
| **总成本** | **$1** | **$86** | **$341** |

### 净利润预估

| 月份 | 月收入 | 月成本 | 月净利润 |
|---|---|---|---|
| M6 | $1,100 | $1 | $1,099 |
| M12 | $3,600 | $86 | $3,514 |
| M18 | $8,000 | $341 | $7,659 |

### 关键假设与验证指标

| 假设 | 验证指标 | 验证时间 |
|---|---|---|
| 下载→MAU 转化率 30% | MAU / 下载量 | M3 |
| MAU→Pro 转化率 2% | Pro 用户 / MAU | M6 |
| MAU→Sync 转化率 3% | Sync 用户 / MAU | M12 |
| Sync 月留存率 90% | Sync 续费率 | M15 |
| 内容→下载转化率 5% | 文章访问 / 下载量 | M6 |

---

## 风险控制与 Plan B

### 风险 1：Pro 买断转化率 < 1%

**触发条件**：M6 时 Pro 用户 < 30

**Plan B**：
- 降价至 $19（限时 30 天）
- 增加 Pro 专属功能（多知识库、批量导出、自定义提示词）
- 考虑完全免费 + Sync/Publish 纯订阅模式

### 风险 2：Sync 服务技术复杂度超预期

**触发条件**：M20 时 Sync 未上线

**Plan B**：
- 先推出简化版 Sync（仅同步对话历史，不同步文档）
- 使用现成方案（如 replicasnet/lib0 CRDT 库）简化冲突解决
- 考虑使用 Cloudflare Durable Objects 替代自建状态管理

### 风险 3：竞品免费压制

**触发条件**：Cherry Studio 或其他竞品推出类似功能且完全免费

**Plan B**：
- 强化 Rust 性能优势（ benchmarks 对比）
- 深耕隐私场景（端到端加密同步是护城河）
- 转向企业市场（Commercial License + 团队功能）

### 风险 4：单开发者精力不足

**触发条件**：功能开发速度 < 用户期望

**Plan B**：
- 接受社区贡献（开源核心代码）
- 优先级管理：只做高 ROI 功能
- 考虑招募兼职协作者（用收入分成替代工资）

---

## 每周执行检查清单

### 第 1-4 周（基建期）

- [ ] 周 1：调整免费版功能边界（代码改动 + 测试）
- [ ] 周 1：创建官网落地页（HTML + 部署到 Cloudflare Pages）
- [ ] 周 2：配置 GitHub Actions 发布 CI
- [ ] 周 2：优化 README（截图 + 下载链接 + 社区链接）
- [ ] 周 3：创建 Discord 服务器 + 微信群 + GitHub Discussions
- [ ] 周 3：录制 30 秒 Demo GIF
- [ ] 周 4：准备发布内容素材（V2EX/HN/Reddit/PH 帖子）
- [ ] 周 4：全平台构建测试 + Release Notes 撰写

### 第 5-8 周（发布期）

- [ ] 周 5：GitHub Release v0.1.0
- [ ] 周 5：V2EX + Hacker News + Reddit 发布
- [ ] 周 6：少数派文章 + Product Hunt 发布
- [ ] 周 6：B站视频教程
- [ ] 周 7：用户访谈（目标 10 个）
- [ ] 周 8：Bug 修复 + 用户反馈整合 + v0.2.0 规划

### 第 9-16 周（变现期）

- [ ] 周 9：搭建 Lemon Squeezy 支付流程
- [ ] 周 10：License 签发 Cloudflare Worker
- [ ] 周 10：优化付费墙 UI（价值展示 + 购买入口）
- [ ] 周 11：Pro 新功能开发（多知识库工作空间）
- [ ] 周 12：Pro 正式上线 + 定价实施
- [ ] 周 13-14：内容营销（2 篇技术博客）
- [ ] 周 15-16：用户反馈收集 + v0.3.0 规划

### 第 17-28 周（Sync 期）

- [ ] 周 17-20：`crates/sync` 客户端开发
- [ ] 周 21-24：Cloudflare Workers 服务端开发
- [ ] 周 25-26：Sync 集成测试 + 端到端加密验证
- [ ] 周 27：Sync UI 开发
- [ ] 周 28：Sync 正式上线 + 付费用户内测

---

## 成功标准

| 时间点 | 必须达成 | 期望达成 |
|---|---|---|
| M3 | 1000 MAU, GitHub Star 500+ | 3000 MAU, Star 1000+ |
| M6 | Pro 收入 $500/月, Star 2000+ | Pro 收入 $1000/月, Star 3000+ |
| M12 | Sync 上线, 总收入 $3000/月 | 总收入 $5000/月 |
| M18 | 总收入 $5000/月, 可持续运转 | 总收入 $8000/月 |

**盈利定义**：月收入 > 月成本 + 基本生活开销（$2000/月）

即：**月净收入 > $2000 = 盈利**

预计达成时间：**M9-M12**

---

*本规划基于 EchoMind 代码库实际分析制定，所有技术方案均可在现有架构上实现。*
