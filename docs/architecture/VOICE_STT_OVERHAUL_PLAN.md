# 语音输入彻底可用改造计划

> **目标**：将 EchoMind 语音录入功能从「基本可用」提升到「彻底可用」
> **基准**：REQ-RAG-034 已实现 getUserMedia + MediaRecorder + Whisper 批处理方案
> **借鉴来源**：Claude Code（NAPI + Deepgram WebSocket 流式 STT）、OpenCode（插件式 STT 配置）

---

## 1. 问题诊断

| # | 问题 | 影响 | 严重度 |
|---|---|---|---|
| 1 | 录音时无任何视觉反馈 | 用户不知道麦克风是否在工作 | ★★★★★ |
| 2 | 无录音计时器 | 用户不知道已录多久 / 何时自动停止 | ★★★★☆ |
| 3 | STT 语言硬编码为 "zh" | 英文/日文用户无法使用 | ★★★★☆ |
| 4 | STT 模型硬编码为 "whisper-1" | 无法用 Groq Whisper-large-v3（更快） | ★★★☆☆ |
| 5 | STT 复用 LLM API Key/URL | 无法使用不同 STT 服务（如 Groq） | ★★★★☆ |
| 6 | 无超时/连接错误区分 | 用户看到笼统错误不知道如何修复 | ★★★☆☆ |
| 7 | 转写失败无重试提示 | 用户体验中断 | ★★☆☆☆ |
| 8 | 已有文本被覆盖 | 输入框有文字时录音结果替换而非追加 | ★★★☆☆ |

---

## 2. 解决方案

### 2.1 已实现改动（本次 Session）

#### S1: 后端增强（`crates/tauri-app/src/commands/features.rs`）

- **STT 独立配置**：新增 `voice.stt_api_key` / `voice.stt_base_url` / `voice.stt_model` / `voice.stt_language` 四个 settings 键
- **降级机制**：STT 专用配置为空时自动降级到 LLM 配置（向后兼容）
- **模型可配**：不再硬编码 `whisper-1`，支持 `whisper-large-v3`（Groq）等
- **语言可配**：不再硬编码 `zh`，支持 `en` / `ja` 等
- **超时改进**：连接超时 15s + 总超时 90s（支持较长录音）
- **错误分类**：区分超时 / 连接失败 / API 错误，返回不同错误消息
- **新增 IPC**：`get_stt_config`（查询 STT 配置，API Key 掩码）+ `set_stt_config`（保存 STT 配置）

#### S2: 前端重写（`ui/src/voice-input.js`）

- **状态机**：IDLE → REQUESTING → RECORDING → TRANSCRIBING → IDLE
- **音频电平可视化**：AnalyserNode → RMS 计算 → 5 条 CSS 电平条实时更新
- **录音计时器**：mm:ss 格式，50s 后变红警告接近最大时长
- **录音遮罩**：浮动卡片显示红点脉冲 + 电平条 + 计时器 + 提示文字
- **追加模式**：输入框有文本时在末尾追加而非替换
- **错误分类提示**：超时 / 连接失败 / API 未配置 分别显示不同 i18n 消息
- **音频质量优化**：128kbps 比特率 + 回声消除 + 噪声抑制 + 自动增益

#### S3: UI + 设置面板（`ui/index.html` + `ui/src/settings.js` + CSS + i18n）

- **录音遮罩 HTML**：`#recordingOverlay` 浮动卡片 + `#levelBarsContainer` 电平条 + `#recordingTimer` 计时器
- **STT 配置面板**：API Key（password 输入）/ Base URL / Model / Language 下拉 / 保存按钮
- **CSS**：脉冲动画 + 电平条激活态 + 警告态样式 + 减弱动画偏好支持
- **i18n**：中英文 14 个新增翻译键

#### S4: E2E 测试（`e2e-tests/tests/voice-enhanced.spec.ts`）

- TC-VOICE-ENH-001：录音遮罩在录音时显示
- TC-VOICE-ENH-002：音频电平条存在（≥5 条）
- TC-VOICE-ENH-003：录音计时器格式正确（mm:ss）
- TC-VOICE-ENH-004：STT 配置面板存在于设置中
- TC-VOICE-ENH-005：STT 配置可保存（Groq URL + whisper-large-v3 + en）
- TC-VOICE-ENH-006：转写文本追加到已有文本后

---

## 3. 架构对比

| 维度 | 旧方案 | 新方案 | Claude Code 方案 |
|---|---|---|---|
| 录音 | MediaRecorder | MediaRecorder + 128kbps + AEC/NS/AGC | NAPI 原生捕获 |
| 静音检测 | AnalyserNode RMS | AnalyserNode RMS（不变） | Deepgram VAD |
| 转写 | 批量 HTTP POST | 批量 HTTP POST（可配 STT 服务） | WebSocket 流式 |
| 反馈 | 仅按钮脉冲 | 电平条 + 计时器 + 遮罩 | 实时文字 |
| 配置 | 复用 LLM Key | 独立 STT 配置 + 降级 | 独立 Deepgram Key |
| 语言 | 硬编码 zh | 可配 zh/en/ja | 跟随 UI |
| 超时 | 60s | 90s + 错误分类 | N/A（流式） |

### 未采用 WebSocket 流式的原因

1. **复杂度高**：需引入 `tokio-tungstenite` 依赖 + WebSocket 生命周期管理
2. **供应商锁定**：Deepgram WebSocket 协议非标准，OpenAI 不提供 WebSocket STT
3. **收益有限**：批量方案在 Whisper API 上延迟 2-5s，流式仅省 1-2s 首字延迟
4. **保持兼容**：批量方案兼容所有 OpenAI 兼容端点（Groq / Azure / 本地 whisper.cpp）

---

## 4. 使用指南

### 4.1 默认行为（零配置）

用户已配置 LLM API Key + Base URL 即可使用语音输入，STT 自动复用 LLM 配置。

### 4.2 配置专用 STT 服务（如 Groq Whisper）

1. 打开设置 → 语音 → STT 配置
2. 填写 STT API Key（Groq Key）
3. 填写 STT Base URL：`https://api.groq.com/openai`
4. 填写 STT Model：`whisper-large-v3`
5. 选择识别语言
6. 保存

### 4.3 Groq vs OpenAI Whisper 性能对比

| 提供商 | 模型 | 延迟（10s 音频） | 费用 |
|---|---|---|---|
| OpenAI | whisper-1 | 3-5s | $0.006/min |
| Groq | whisper-large-v3 | 0.5-1s | $0.111/hr |
| Groq | whisper-large-v3-turbo | 0.3-0.5s | $0.111/hr |

---

## 5. 验收结果

| 验证项 | 结果 |
|---|---|
| cargo clippy --all-targets -- -D warnings | ✅ 零警告 |
| cargo fmt --check | ✅ 通过 |
| cargo test -p echomind-core | ✅ 全部通过 |
| npx tsc --noEmit | ✅ 0 errors |
| node scripts/build-ui.mjs | ✅ 成功 |
| voice.spec.ts (8 tests) | ✅ 8 passed |
| voice-enhanced.spec.ts (6 tests) | ✅ 6 passed |
