# ADR-010: 多模态内容理解引擎

> **状态**：Approved (Phase 1) ｜ **日期**：2026-07-25 ｜ **决策者**：TPM + Chief Architect
>
> **关联需求**：REQ-MM-001 ~ REQ-MM-004（SRS §4.12）
>
> **调研来源**（铁律八互联网搜索 + rs-pro 参考项目，2026-07-25）：
> - pdfium-render v0.9.3（crates.io 下载 1.7M，Chrome Pdfium C++ 绑定）
> - ocrs v0.12.2 + rten v0.24.0（纯 Rust OCR，PaddleOCR PP-OCRv4，下载 180K + 1M）
> - tesseract-rs v0.3.0（Tesseract C++ 绑定，下载 58K，需系统安装）
> - rusty-tesseract v1.1.10（Tesseract CLI 包装，下载 114K）
> - lopdf v0.44.0（纯 Rust PDF 操作，下载 13M，仅文本提取不支持渲染）
> - rs-pro 参考项目已验证 ocrs + rten 方案可用
> - ColPali / Jina CLIP v2（远期候选）
> - BYOK VLM（GPT-4o / Claude 3.5 Sonnet 经 OpenAI 兼容 Vision API）

---

## 1. 问题背景

当前 PDF 导入仅提取文本层（lopdf），三类内容完全丢失：

| 丢失内容 | 场景 | 影响 |
|---|---|---|
| **图片中的文字** | 扫描版 PDF、截图文档 | 文字无法被检索 |
| **图片中的表格** | 财务报告、学术论文 | 表格数据无法被检索 |
| **图片中的甘特图/图表** | 项目计划、建筑图纸 | 图表信息无法被检索 |

### 三层难度递增

```
难度递增 →
┌──────────┐    ┌──────────┐    ┌──────────────┐
│ 图片文字  │    │ 图片表格  │    │ 图表/甘特图   │
│ (印刷体)  │    │ (结构化)  │    │ (语义化)      │
├──────────┤    ├──────────┤    ├──────────────┤
│ OCR ✅    │    │ OCR ⚠️   │    │ OCR ❌       │
│ 解决     │    │ 部分     │    │ 无法解决     │
│          │    │          │    │              │
│ Tesseract│    │ Tesseract│    │ 需要 VLM     │
│ 本地运行 │    │ + 表格   │    │ GPT-4o/Claude│
│ 零网络   │    │ 识别     │    │ BYOK Vision  │
└──────────┘    └──────────┘    └──────────────┘
     ↑               ↑                   ↑
  Phase 1        Phase 1+           Phase 2
  (本地)         (本地增强)          (BYOK)
```

---

## 2. 技术选型对比

### 2.1 PDF 页面渲染引擎

| 方案 | 语言 | 体积 | 图片提取 | 页面渲染 | Rust binding | 决策 |
|---|---|---|---|---|---|---|
| **pdfium-render v0.9.3** | C++ | ~6MB | ✅ XObject | ✅ 全页渲染 | pdfium-render crate（下载 1.7M） | **采用** |
| mupdf | C | ~15MB | ✅ | ✅ | mupdf-rs（crates.io 未找到） | 不采用 |
| lopdf v0.44.0（当前） | Rust | ~2MB | ❌ 仅文本 | ❌ | 原生（下载 13M） | 保留文本提取 |

**决策**：保留 lopdf 做文本提取，新增 `pdfium-render` v0.9.3 做页面渲染。

**铁律九论证**：crates.io 和 GitHub 搜索确认 Rust 生态无可用的纯 Rust PDF 页面渲染库（lopdf 仅支持文本提取，不支持渲染为位图）。pdfium 是 Google Chrome 使用的 PDF 引擎，业界标准，C++ 绑定是唯一可行方案。pdfium 二进制文件（~6MB）可随 Tauri 应用打包分发，无需系统安装。

### 2.2 OCR 引擎

| 方案 | 语言 | 体积 | 中英文 | 表格识别 | Rust 生态 | 决策 |
|---|---|---|---|---|---|---|
| **ocrs v0.12.2 + rten v0.24.0** | **纯 Rust** | ~17MB | ✅ chi_sim+eng | ❌ | ✅ 纯 Rust，无 C 依赖 | **采用** |
| tesseract-rs v0.3.0 | C++ | ~40MB | ✅ chi_sim+eng | ⚠️ 基础 | 需系统安装 Tesseract | 不采用 |
| rusty-tesseract v1.1.10 | CLI | ~40MB | ✅ | ⚠️ | CLI 包装，非库绑定 | 不采用 |
| PaddleOCR | Python | ~200MB | ✅ | ✅ PP-Structure | ❌ 需 Python | 不采用 |

**决策**：`ocrs` v0.12.2 + `rten` v0.24.0（纯 Rust PaddleOCR PP-OCRv4）。

**铁律九论证**：
1. `ocrs` 是纯 Rust 实现（基于 `rten` 推理引擎），无 C/C++ 依赖，完全符合铁律九「优先纯 Rust」原则
2. 铁律九已明确列出：「OCR: `ocrs` + `rten` — 纯 Rust PaddleOCR PP-OCRv4」
3. rs-pro 参考项目已验证可用（`ocrs = "0.12"`, `rten = "0.24"`），有完整 `LocalOcrEngine` 实现
4. PP-OCRv4 模型仅 17MB，支持中英文，比 Tesseract（~40MB）更轻量
5. 无需系统安装（Tesseract 需系统安装 + 语言包），Tauri 应用应自包含
6. 下载量：ocrs 180K + rten 1M >> tesseract-rs 58K，生态更活跃
7. SRS 原提 Tesseract，但铁律九优先纯 Rust 方案，经架构评审确认 ocrs 更优

**SRS 变更**：REQ-MM-002 描述从「Tesseract（C 库 + Rust binding）」更新为「ocrs（纯 Rust PaddleOCR PP-OCRv4 + rten 推理引擎）」。

### 2.3 图片理解（表格/图表/甘特图）

| 方案 | 路径 | 隐私 | 能力 | 决策 |
|---|---|---|---|---|
| **BYOK VLM** | 本地渲染 → 用户 LLM Vision API | BYOK 控制 | 表格→MD，甘特→Mermaid | **采用** |
| 本地 VLM (LLaVA) | 本地 ONNX 推理 | 完全本地 | 推理慢，模型大(~4GB) | 远期候选 |
| ColPali 视觉嵌入 | 页面级视觉嵌入 | 完全本地 | 仅 Python，无 Rust | 远期候选 |

**决策**：BYOK VLM（Phase 2），本地 VLM（Phase 3 远期）。

### 2.4 多模态嵌入

| 方案 | 模型 | Rust 生态 | 嵌入维度 | 决策 |
|---|---|---|---|---|
| Jina CLIP v2 | 文本+图像统一 | ❌ fastembed 不支持 | 1024 | 远期 |
| ColPali | 页面级视觉 | ❌ 仅 Python | 128 | 远期 |
| 当前 all-MiniLM-L6-v2 | 纯文本 | ✅ fastembed 支持 | 384 | **保留** |

**决策**：Phase 1~2 保留纯文本嵌入（将 OCR/VLM 输出的文本统一嵌入），Phase 3 远期评估 CLIP。

---

## 3. Chief Architect 架构方案

### 3.1 新增端口（core 层）

```rust
/// PDF 页面渲染端口（REQ-MM-001）
pub trait PageRenderer: Send + Sync {
    /// 将指定页面渲染为位图（PNG 字节），供 OCR 或 VLM 处理。
    async fn render_page(&self, pdf_path: &str, page_num: usize, dpi: u32) -> anyhow::Result<Vec<u8>>;
}

/// OCR 文字识别端口（REQ-MM-002）
pub trait OcrEngine: Send + Sync {
    /// 对位图执行 OCR，返回识别出的文本。
    async fn recognize(&self, image_bytes: &[u8]) -> anyhow::Result<String>;
}

/// VLM 图片理解端口（REQ-MM-003）
pub trait VisionLLM: Send + Sync {
    /// 将图片发送到 VLM，获取结构化文本描述。
    /// 系统提示引导：表格→Markdown表格，甘特图→Mermaid语法，流程图→Mermaid flowchart。
    async fn describe_image(&self, image_bytes: &[u8], prompt: &str) -> anyhow::Result<String>;
}
```

### 3.2 新增适配器（infra 层）

| 端口 | 适配器 | 文件 | 依赖 |
|---|---|---|---|
| PageRenderer | `PdfiumRenderer` | `crates/infra/src/pdfium_renderer.rs` | pdfium-render v0.9.3（C++ pdfium 二进制随应用打包） |
| OcrEngine | `OcrsEngine` | `crates/infra/src/ocrs_engine.rs` | ocrs v0.12.2 + rten v0.24.0（纯 Rust，PP-OCRv4 模型 ~17MB） |
| VisionLLM | `OpenAIVisionProvider` | `crates/infra/src/openai_vision.rs` | reqwest（复用现有） |

### 3.3 多模态导入管线编排

```
PdfLoader.load(file_path)
  ↓
lopdf 文本层提取（现有逻辑）
  ↓
PageRenderer.render_page() ← 检测含图片的页面
  ↓
  ├── OcrEngine.recognize() ← 本地 OCR（零网络）
  └── VisionLLM.describe_image() ← BYOK VLM（可选，用户开启）
  ↓
合并：页面文本 + OCR 文本 + VLM 描述
  ↓
SemanticSplitter.split()（现有逻辑）
  ↓
Embedder.embed_batch()（现有逻辑）
  ↓
Storage.add_chunk() + add_embedding()（现有逻辑）
```

### 3.4 进度反馈扩展

`doc-status-changed` 事件 payload 扩展子阶段：

```rust
pub struct DocStatusPayload {
    pub status: String,      // indexing / done / error
    pub message: String,     // 可读进度文案
    pub sub_phase: String,   // text_extracting / image_rendering / ocr / vlm_enhancing（新增）
}
```

### 3.5 架构约束

- **不改变六边形架构**：新端口位于 core，适配器位于 infra
- **不改变现有端口**：Loader / Splitter / Embedder / Storage 端口签名不变
- **复用现有基础设施**：spawn_blocking / WAL / OnceCell 等模式不变
- **渐进式集成**：OCR 先行，VLM 后加，互不依赖

---

## 4. Security Officer 安全方案

### 4.1 隐私边界分析

| 组件 | 数据流向 | 隐私级别 | 安全措施 |
|---|---|---|---|
| PDF 页面渲染 | 本地 | 🟢 零网络 | pdfium 本地渲染 |
| Tesseract OCR | 本地 | 🟢 零网络 | C 库本地推理 |
| BYOK VLM | 本地 → 用户 LLM 端点 | 🟡 BYOK 控制 | 仅发往用户配置的 base_url |

### 4.2 安全官要求

1. **VLM 开关**：用户必须在设置中**主动开启** VLM 增强，默认关闭
2. **明确告知**：开启时弹出确认弹窗：「图片将发送到你配置的 LLM 端点（{base_url}），是否继续？」
3. **网络审计**：VLM 请求的 URL 必须与 `llm_config.base_url` 完全一致（网络监控断言）
4. **OCR 无网络**：`ocrs` 为纯 Rust 本地推理，零网络请求，CI 中加网络监控断言
5. **图片不落盘**：渲染的位图在 OCR/VLM 处理后立即从内存清除，不持久化到磁盘

### 4.3 安全一票否决权

若发现 ocrs 存在隐式网络回传，或 VLM 请求绕过用户配置的 base_url，安全官有权否决发布。

---

## 5. Growth Strategist 商业方案

### 5.1 多模态功能付费墙

| 功能 | 免费版 | Pro 版 |
|---|---|---|
| PDF 文本层提取 | ❌（PDF 本身就是 Pro 功能） | ✅ |
| PDF 图片页面渲染 | ❌ | ✅ |
| 本地 OCR（印刷文字） | ❌ | ✅ |
| BYOK VLM（表格/图表/甘特图） | ❌ | ✅ |
| VLM 增强开关 | — | ✅ 可关闭 |

### 5.2 差异化定位

| 竞品 | 图片理解能力 | EchoMind 差异化 |
|---|---|---|
| ChatGPT Desktop | 云端 VLM | 本地 OCR + BYOK VLM（隐私可控） |
| LLM Wiki (⭐15,260) | 不支持 | OCR + VLM 双引擎 |
| Obsidian + 插件 | 需手动 OCR | 自动化管线 |

---

## 6. 实施路线图

| Phase | 内容 | 依赖 | Crate | 状态 |
|---|---|---|---|---|
| **Phase 1** | PDF 页面渲染 + 本地 OCR | REQ-MM-001/002 | pdfium-render v0.9.3 + ocrs v0.12.2 + rten v0.24.0 | ✅ 架构评审通过 |
| **Phase 2** | BYOK VLM 图片理解 | Phase 1 + REQ-MM-003 | 复用 OpenAIProvider Vision API | ✅ 完成（VisionLanguageModel 端口 + OpenAIVisionProvider 适配器 + NoVlm 占位 + TC-MM-003/003b/003c 全绿） |
| **Phase 3** | 多模态导入管线编排 | Phase 1+2 + REQ-MM-004 | 状态机 + 进度反馈 | ✅ 完成 |
| **Phase 4（远期）** | CLIP 多模态嵌入 / 本地 VLM | fastembed 支持 / Florence-2 ONNX | 待 Rust 生态成熟 | 远期 |

### 关键技术风险

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| pdfium 二进制跨平台打包 | 中 | 中 | Tauri resource 机制 + pdfium-render 的 dynamic loading |
| ocrs 模型下载失败 | 低 | 低 | 复用多源下载器（HuggingFace + hf-mirror.com） |
| VLM 返回格式不稳定 | 高 | 中 | Prompt 工程 + 容错解析 |
| OCR 中文识别准确率 | 中 | 中 | PP-OCRv4 原生支持中文，rs-pro 已验证 |

---

## 7. 各角色签字

| 角色 | 签字 | 日期 | 备注 |
|---|---|---|---|
| TPM | ✅ Approved | 2026-07-25 | Phase 1 架构评审完成 |
| Chief Architect | ✅ Approved | 2026-07-25 | pdfium-render + ocrs + rten 确认 |
| Security Officer | ✅ Approved | 2026-07-25 | ocrs 纯 Rust 零网络；VLM 开关 + 网络审计 |
| QA Lead | ⏳ 待登记 | — | TC-MM-001~004 |
| Growth Strategist | ✅ Approved | 2026-07-25 | Pro 专属功能 |
