# ADR-009: 富内容可视化渲染引擎

> **状态**：Approved (Phase 1+2) ｜ **日期**：2026-07-25 ｜ **决策者**：TPM + Chief Architect
>
> **关联需求**：REQ-VIZ-001 ~ REQ-VIZ-004（SRS §4.11）
>
> **调研来源**（铁律八互联网搜索，2026-07-25）：
> - Mermaid.js v11.16.0（GitHub ⭐89,399）
> - Chart.js v4.5.1
> - KaTeX（LaTeX 数学公式）
> - 全职业场景调研覆盖 12 个领域

---

## 1. 决策背景

当前答案区仅支持 Markdown 基础语法 + 代码高亮（marked.js → DOMPurify → highlight.js 链路）。LLM 天然擅长生成结构化文本语法（Mermaid / LaTeX / CSV），集成富内容渲染引擎后，一次投入即可覆盖 12+ 职业场景的 10+ 种图表类型，是本产品核心价值差异化所在。

## 2. 技术选型决策

### 2.1 选型对比

| 方案 | Stars | 支持图表数 | 体积 | 本地 vendored | 流式兼容 | 决策 |
|---|---|---|---|---|---|---|
| **Mermaid.js v11.16.0** | 89,399 | 12+ 种 | ~2.5MB | ✅ 独立 min 文件 | ✅ chat_done 后渲染 | **采用** |
| Chart.js v4.5.1 | — | 6 种数据图 | ~200KB | ✅ | ✅ | **采用（Phase 2）** |
| **KaTeX v0.18.1** | — | 数学公式 | ~280KB | ✅ 含字体 | ✅ 实时渲染 | **采用（Phase 2）✅** |
| D3.js | 113,269 | 自定义 | ~280KB | ✅ | ✅ | 候选（Phase 3） |
| ECharts 6.1.0 | — | 全类型 | ~1MB | ✅ | ✅ | 不采用（体积过大） |
| PlantUML | 13,199 | UML 专用 | 服务端 | ❌ 需 Java | ❌ | 不采用 |
| Three.js | 113,973 | 3D | ~600KB | ✅ | ⚠️ | 候选（Phase 3） |

### 2.2 决策结论

- **Phase 1（P0）**：Mermaid.js — 覆盖流程图/甘特/序列/类图/状态/ER/饼图/思维导图/时间线/旅程/C4/Git 图
- **Phase 2（P1）**：KaTeX + mhchem — 覆盖数学/物理/化学公式
- **Phase 3（P1）**：Chart.js — 覆盖数据可视化（柱/折/饼/雷达）
- **Phase 4+（P2）**：G6/D3/Three.js — 按需引入专业领域方案

**Rust 生态调研结论**（铁律九）：crates.io 搜索确认 Rust 生态无可用的前端图表渲染方案，必须使用 JS 库。此例外经架构评审批准。

---

## 3. TPM 契约冻结（体系二阶段 3）

### 3.1 前端渲染链路契约

```
流式 token 到达 (chat_token 事件)
    ↓
currentRawMarkdown += token
    ↓
scheduleRender() (rAF 节流)
    ↓
renderMarkdown()
    ├── marked.js 解析 Markdown → HTML
    ├── ```mermaid 代码块 → <div class="mermaid-source" data-raw="源码">占位提示</div>
    ├── $...$ / $$...$$ → <span class="katex-pending" data-latex="公式">公式源码</span>
    ├── DOMPurify 消毒（白名单，不放松 SVG 标签）
    └── highlight.js 语法高亮（跳过 mermaid 语言）
    ↓
chat_done 事件
    ↓
renderRichContent()
    ├── mermaid.run({ query: '.mermaid-source' }) → SVG 图表
    ├── katex.render() → 数学公式
    └── 表格数据检测 → 「切换图表」按钮
```

### 3.2 RAG Prompt 契约（REQ-VIZ-004）

`build_rag_prompt()` 在系统提示词末尾追加图表引导指令：

```text
当回答涉及流程、架构、关系、时间线时，使用 ```mermaid 代码块输出图表语法；
当涉及数学公式时，使用 $...$ 或 $$...$$ 语法；
当涉及数据对比时，优先使用 Markdown 表格。
图表语法必须完整闭合，不得在代码块中途截断。
```

**契约约束**：
- 仅修改 `crates/core/src/chat.rs` 的 `build_rag_prompt()` 函数
- 不改变 RAG 检索逻辑（Retriever / Storage 端口不变）
- 不新增 IPC 命令（纯前端渲染 + Prompt 变更）
- 不改变 Tauri 事件契约（chat_token / chat_done 事件不变）

### 3.3 新增 vendor 文件清单

| 文件 | 来源 | 体积 | Phase |
|---|---|---|---|
| `ui/vendor/mermaid.min.js` | mermaid-js/mermaid v11.16.0 | ~2.5MB | Phase 1 |
| `ui/vendor/katex.min.js` | KaTeX 官方 release | ~280KB | Phase 2 |
| `ui/vendor/katex.min.css` | KaTeX 官方 release | ~20KB | Phase 2 |
| `ui/vendor/katex/fonts/` | KaTeX woff2 字体 | ~300KB | Phase 2 |
| `ui/vendor/katex-mhchem.min.js` | KaTeX mhchem 插件 | ~15KB | Phase 2 |
| `ui/vendor/chart.umd.min.js` | chartjs/Chart.js v4.5.1 | ~200KB | Phase 3 |

---

## 4. Chief Architect 架构方案

### 4.1 渲染时序与生命周期

```
┌──────────────────────────────────────────────────────────┐
│  流式阶段 (streaming=true)                                │
│  ┌─────────┐     ┌──────────┐     ┌───────────┐           │
│  │ token   │────▶│ marked   │────▶│ DOMPurify │           │
│  │ append  │     │ parse    │     │ sanitize  │           │
│  └─────────┘     └──────────┘     └─────┬─────┘           │
│                                         │                 │
│                              ┌──────────▼──────────┐      │
│                              │ mermaid 代码块 →     │      │
│                              │ <div class=          │      │
│                              │  "mermaid-source">   │      │
│                              │  占位提示             │      │
│                              │ </div>               │      │
│                              │                      │      │
│                              │ $...$ →              │      │
│                              │ <span class=         │      │
│                              │  "katex-pending">    │      │
│                              │  源码                │      │
│                              │ </span>              │      │
│                              └─────────────────────┘      │
└──────────────────────────────────────────────────────────┘
                           │
                    chat_done 事件
                           ▼
┌──────────────────────────────────────────────────────────┐
│  富内容渲染阶段 (post-stream)                              │
│  ┌─────────────────┐  ┌──────────────┐  ┌────────────┐   │
│  │ mermaid.run()   │  │ katex.render │  │ 表格→图表  │   │
│  │ → SVG 图表       │ │ () → 公式     │  │ 按钮注入   │   │
│  └────────┬────────┘  └──────┬───────┘  └─────┬──────┘   │
│           │                  │                 │          │
│           ▼                  ▼                 ▼          │
│      DOM 注入          DOM 注入          DOM 注入          │
└──────────────────────────────────────────────────────────┘
```

### 4.2 架构约束

- **无新增 IPC 命令**：富内容渲染完全在前端完成，不涉及后端 Rust 代码（REQ-VIZ-004 Prompt 变更除外）
- **不改变 hexagonal 架构**：`build_rag_prompt()` 位于 `crates/core/src/chat.rs`，修改不引入新依赖
- **不阻塞 async executor**：`mermaid.run()` 在前端 JS 主线程执行，不影响 Tauri 后端
- **DOMPurify 白名单不变**：采用"先消毒文本 → 后渲染图表"策略，不放松 SVG 标签白名单

### 4.3 性能考量

| 场景 | 方案 | 预期耗时 |
|---|---|---|
| Mermaid 渲染 10 个图表 | chat_done 后批量 `mermaid.run()` | ~200ms |
| KaTeX 渲染 50 个公式 | 流式过程中逐个 `katex.render()` | ~250ms (5ms/个) |
| Chart.js 渲染 3 个图表 | 用户点击切换时按需初始化 | ~30ms/个 |
| 流式期间 rAF 渲染 | scheduleRender 防抖 | 16ms/帧 |

---

## 5. Frontend Dev 实施方案

### 5.1 index.html 修改点

**新增 vendor 引入（`<head>` 内）**：
```html
<script src="vendor/mermaid.min.js"></script>
<script src="vendor/katex/katex.min.js"></script>
<link rel="stylesheet" href="vendor/katex/katex.min.css">
<script src="vendor/katex/katex-mhchem.min.js"></script>
<script src="vendor/chart.umd.min.js"></script>
```

**Mermaid 初始化配置（启动引导段）**：
```javascript
mermaid.initialize({
  startOnLoad: false,      // 不自动渲染，手动控制时序
  theme: 'dark',            // 暗色主题
  securityLevel: 'strict',  // 安全级别：严格模式
  fontFamily: 'inherit',    // 继承全局字体
});
```

**renderMarkdown() 增强**（marked.js 自定义渲染器）：
- ```mermaid 代码块 → 不走 highlight.js，输出 `<div class="mermaid-source">` 占位
- `$...$` 行内公式 → `katex.renderToString()` 实时渲染
- `$$...$$` 块级公式 → 居中 `katex.renderToString()`

**新增 renderRichContent() 函数**（chat_done 事件后调用）：
- 遍历 `.mermaid-source` 元素，调用 `mermaid.run()` 渲染
- 遍历 `.katex-pending` 元素（如有未渲染的），调用 `katex.render()`
- 检测 Markdown 表格，注入「📊 切换图表」按钮

### 5.2 流式渲染容错

| 场景 | 处理策略 |
|---|---|
| Mermaid 语法不完整（流式中） | 显示源码 + "图表渲染中…" 占位，不尝试渲染 |
| Mermaid 语法错误（chat_done 后） | `mermaid.run()` 捕获错误，显示"⚠ 图表语法错误"提示，保留源码 |
| KaTeX 半截语法 | `katex.render()` 抛出错误时保留原始 `$...$` 文本 |
| Chart.js 数据解析失败 | 保留原 Markdown 表格，不显示图表按钮 |

---

## 6. Security Officer 安全方案

### 6.1 XSS 防御策略

**核心原则**：DOMPurify 消毒发生在 Mermaid/KaTeX 渲染**之前**，对输入文本进行白名单消毒。Mermaid 和 KaTeX 的输出 SVG/HTML 不需要二次消毒（因为输入已是消毒后的文本）。

```
LLM 输出文本
    ↓
marked.js 解析为 HTML
    ↓
DOMPurify.sanitize() ← 白名单消毒（REQ-SEC-002）
    ↓
插入 DOM
    ↓
mermaid.run() / katex.render() ← 输入是已消毒文本，输出安全
    ↓
SVG / HTML 公式插入 DOM
```

### 6.2 安全审查项

| 风险 | 防御措施 | 测试用例 |
|---|---|---|
| Mermaid SVG 注入 `<script>` | `securityLevel: 'strict'` 配置 + TC-VIZ-006 断言 | TC-VIZ-006 |
| KaTeX 公式注入 HTML | KaTeX 内置转义 + DOMPurify 前置消毒 | TC-VIZ-005 |
| Chart.js 配置注入 | 仅从 Markdown 表格解析数据，不接受 LLM 输出的 JSON 配置 | TC-VIZ-007 |
| 前端库供应链安全 | 全部本地 vendored，无 CDN，无 `eval()` | CI 静态扫描 |

### 6.3 Mermaid securityLevel 配置

```javascript
mermaid.initialize({
  securityLevel: 'strict',  // 最高安全级别：禁用 HTML 标签注入
  // 'strict' 模式下，Mermaid 不会在 SVG 中插入 <script> 或事件属性
});
```

**安全官一票否决权**：若 Mermaid `securityLevel: 'strict'` 仍存在 SVG XSS 风险，安全官有权否决 Mermaid 集成方案，回退为纯文本代码块展示。

---

## 7. DevOps & 供应链方案

### 7.1 本地 vendored 流程

```bash
# Phase 1: Mermaid.js
curl -sL "https://unpkg.com/mermaid@11.16.0/dist/mermaid.min.js" -o ui/vendor/mermaid.min.js
# 完整性校验（SRI hash）
shasum -a 256 ui/vendor/mermaid.min.js

# Phase 2: KaTeX
curl -sL "https://unpkg.com/katex@0.16.22/dist/katex.min.js" -o ui/vendor/katex/katex.min.js
curl -sL "https://unpkg.com/katex@0.16.22/dist/katex.min.css" -o ui/vendor/katex/katex.min.css
# 下载 woff2 字体目录
# 下载 mhchem 插件

# Phase 3: Chart.js
curl -sL "https://unpkg.com/chart.js@4.5.1/dist/chart.umd.min.js" -o ui/vendor/chart.umd.min.js
```

### 7.2 CI 门禁新增

```yaml
# .github/workflows/e2e.yml 新增步骤
- name: 验证 vendor 完整性（SRI 校验）
  run: |
    # 校验所有 vendor 文件的 SHA-256 hash
    python3 scripts/verify_vendor_integrity.py

- name: 静态扫描：禁止 CDN 引用
  run: |
    # 扫描 ui/index.html 不含 http(s):// script/link 引用
    ! grep -E 'https?://' ui/index.html | grep -E '<script|<link'
```

### 7.3 vendor 文件清单管理

新增 `ui/vendor/MANIFEST.json` 记录所有 vendor 文件的版本、来源 URL 和 SRI hash：

```json
{
  "files": [
    {
      "path": "mermaid.min.js",
      "version": "11.16.0",
      "source": "https://unpkg.com/mermaid@11.16.0/dist/mermaid.min.js",
      "sha256": "..."
    }
  ]
}
```

---

## 8. Growth Strategist 商业方案

### 8.1 差异化定位

| 竞品 | 富内容渲染能力 | EchoMind 差异化 |
|---|---|---|
| LLM Wiki (⭐15,260) | 基础 Markdown | Rust 极速渲染 + 隐私不出域 |
| ChatGPT Desktop | Mermaid 流程图 | BYOK 买断制 + 本地知识库 RAG |
| Obsidian + 插件 | 需手动安装插件 | 开箱即用，零配置 |

### 8.2 Pro 付费墙策略

| 功能 | 免费版 | Pro 版 |
|---|---|---|
| Mermaid 图表渲染 | ✅ 基础（flowchart/gantt/pie） | ✅ 全部 12+ 种 |
| KaTeX 数学公式 | ✅ 基础公式 | ✅ + mhchem 化学方程式 |
| Chart.js 数据图表 | ❌ | ✅ |
| 图表点击放大 / 导出 PNG | ❌ | ✅ |

**增长策略师原则**：任何增长手段不得损害「隐私不出域」承诺。富内容渲染完全在本地完成，不发送任何数据到外部服务。

---

## 9. 实施里程碑

### Phase 1（Mermaid.js 图表引擎）

| 里程碑 | 内容 | 预计工时 | 前置条件 | 状态 |
|---|---|---|---|---|
| M1 | 下载 Mermaid.js 到 vendor + SRI 校验 | 0.5h | 无 | ✅ |
| M2 | 修改 renderMarkdown() 支持 mermaid 占位 | 2h | M1 | ✅ |
| M3 | 新增 renderRichContent() + mermaid.run() | 1h | M2 | ✅ |
| M4 | 修改 build_rag_prompt() 添加图表引导 | 0.5h | M3 | ✅ |
| M5 | E2E 桥接测试 TC-VIZ-001~004 | 2h | M4 | ✅ |
| M6 | 安全测试 TC-VIZ-006 | 1h | M5 | ✅ |
| **Phase 1 交付** | Mermaid 图表引擎上线 | **7h** | M1~M6 | ✅ |

### Phase 2（KaTeX 数学公式引擎）

| 里程碑 | 内容 | 预计工时 | 前置条件 | 状态 |
|---|---|---|---|---|
| M1 | 下载 KaTeX 0.18.1 到 vendor（JS+CSS+mhchem+20 woff2 字体）+ SRI 校验 | 0.5h | Phase 1 | ✅ |
| M2 | 修改 renderMarkdown() 支持 $...$ / $$...$$ 公式渲染 | 1.5h | M1 | ✅ |
| M3 | 修改 renderRichContent() 添加 KaTeX 重试渲染 | 0.5h | M2 | ✅ |
| M4 | tauri-stub.js 添加 KaTeX mock token 序列 | 0.5h | M3 | ✅ |
| M5 | E2E 测试 TC-VIZ-005/005b/005c/005d | 1.5h | M4 | ✅ |
| **Phase 2 交付** | KaTeX 数学公式引擎上线 | **4.5h** | M1~M5 | ✅ |

---

## 10. 风险与缓解

| 风险 | 概率 | 影响 | 缓解措施 |
|---|---|---|---|
| Mermaid.js 体积过大影响启动 | 中 | 低 | defer 加载，不影响首屏 |
| LLM 输出不完整 Mermaid 语法 | 高 | 中 | chat_done 后渲染 + 错误容错 |
| DOMPurify 与 Mermaid SVG 冲突 | 低 | 高 | 先消毒后渲染策略 + securityLevel:strict |
| KaTeX 字体加载失败 | 低 | 低 | woff2 本地 vendored + fallback 字体 |
| Chart.js 与 Markdown 表格解析不兼容 | 中 | 中 | 仅从表格解析数据，不接受 LLM JSON |

---

## 11. 决策结论

**Phase 1（Mermaid.js 图表引擎）已实施并验证**，覆盖 REQ-VIZ-001 和 REQ-VIZ-004。TC-VIZ-001~006 全部绿灯（5/5 E2E passed）。

**Phase 2（KaTeX 数学公式引擎）已实施并验证**，覆盖 REQ-VIZ-002。TC-VIZ-005/005b/005c/005d 全部绿灯（4/4 E2E passed）。KaTeX v0.18.1 + mhchem 插件，支持行内/块级 LaTeX 公式和化学方程式。

Phase 3（Chart.js）在 Phase 2 完成后启动。

| 角色 | 签字 | 日期 |
|---|---|---|
| TPM | ✅ Approved | 2026-07-25 |
| Chief Architect | ✅ Approved | 2026-07-25 |
| Security Officer | ✅ Approved（securityLevel:strict 已实施验证；KaTeX 内置转义 + DOMPurify 前置消毒已验证） | 2026-07-25 |
| QA Lead | ✅ TC-VIZ-001~006 绿灯；TC-VIZ-005/005b/005c/005d 绿灯；TC-VIZ-008/008b/008c 绿灯 | 2026-07-25 |
