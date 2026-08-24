//! 语音转写 IPC 命令（REQ-RAG-034 桌面应用方案）。
//! getUserMedia + MediaRecorder 录音 → IPC 发送到 OpenAI Whisper API
use super::super::*;

/// 语音转写：将音频数据发送到 OpenAI 兼容的 Whisper API 进行语音识别（REQ-RAG-034）。
///
/// 桌面应用方案：前端使用 `navigator.mediaDevices.getUserMedia` + `MediaRecorder`
/// 录制音频，通过 IPC 发送到 Rust 侧，Rust 侧调用 OpenAI `/audio/transcriptions`
/// 端点转写为文本。避免了 WKWebView 不支持 Web Speech API 的问题。
///
/// # 参数
/// - `audio_data`: 音频二进制数据（WebM/OGG 格式，MediaRecorder 默认输出）
/// - `mime_type`: MIME 类型（如 `"audio/webm"`、`"audio/ogg"`）
///
/// # 返回
/// 转写文本字符串。
///
/// # 错误
/// - `LLM: ` 前缀 — API 调用失败（网络错误、API Key 无效、服务不可用）
/// - `VALIDATION: ` 前缀 — 未配置 API Key 或 Base URL
/// 新增：STT 配置查询命令（前端设置面板使用）。
#[tauri::command]
pub async fn get_stt_config(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let stt_api_key = state
        .storage
        .get_setting("voice.stt_api_key")
        .await
        .map_err(|e| format!("{e:#}"))?
        .unwrap_or_default();
    let stt_base_url = state
        .storage
        .get_setting("voice.stt_base_url")
        .await
        .map_err(|e| format!("{e:#}"))?
        .unwrap_or_default();
    let stt_model = state
        .storage
        .get_setting("voice.stt_model")
        .await
        .map_err(|e| format!("{e:#}"))?
        .unwrap_or_else(|| "whisper-1".to_string());
    let stt_language = state
        .storage
        .get_setting("voice.stt_language")
        .await
        .map_err(|e| format!("{e:#}"))?
        .unwrap_or_else(|| "zh".to_string());
    // 掩码 API Key（安全）
    let masked_key = if stt_api_key.is_empty() {
        String::new()
    } else if stt_api_key.len() <= 8 {
        "****".to_string()
    } else {
        format!("****{}", &stt_api_key[stt_api_key.len() - 4..])
    };
    Ok(serde_json::json!({
        "stt_api_key_masked": masked_key,
        "stt_base_url": stt_base_url,
        "stt_model": stt_model,
        "stt_language": stt_language,
        "has_custom_config": !stt_api_key.is_empty() || !stt_base_url.is_empty()
    }))
}

/// 新增：STT 配置保存命令（前端设置面板使用）。
#[tauri::command]
pub async fn set_stt_config(
    stt_api_key: Option<String>,
    stt_base_url: Option<String>,
    stt_model: Option<String>,
    stt_language: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if let Some(key) = stt_api_key {
        // 空字符串表示清除专用配置，降级到 LLM 配置
        state
            .storage
            .set_setting("voice.stt_api_key", &key)
            .await
            .map_err(|e| format!("{e:#}"))?;
    }
    if let Some(url) = stt_base_url {
        state
            .storage
            .set_setting("voice.stt_base_url", &url)
            .await
            .map_err(|e| format!("{e:#}"))?;
    }
    if let Some(model) = stt_model {
        // 验证模型名非空
        let trimmed = model.trim();
        if trimmed.is_empty() {
            return Err(prefix_error(ERR_VALIDATION, "STT 模型名不能为空"));
        }
        state
            .storage
            .set_setting("voice.stt_model", trimmed)
            .await
            .map_err(|e| format!("{e:#}"))?;
    }
    if let Some(lang) = stt_language {
        let trimmed = lang.trim();
        if !trimmed.is_empty() {
            state
                .storage
                .set_setting("voice.stt_language", trimmed)
                .await
                .map_err(|e| format!("{e:#}"))?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn transcribe_audio(
    audio_data: Vec<u8>,
    mime_type: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    transcribe_audio_inner(&audio_data, &mime_type, state.inner()).await
}

/// 语音转写逻辑（命令与集成测试复用）。
///
/// 支持独立的 STT 配置（voice.stt_api_key / voice.stt_base_url / voice.stt_model /
/// voice.stt_language），未配置时降级到 LLM 配置。支持 Groq Whisper、OpenAI Whisper
/// 及任何 OpenAI 兼容的 /audio/transcriptions 端点。
pub async fn transcribe_audio_inner(
    audio_data: &[u8],
    mime_type: &str,
    state: &AppState,
) -> Result<String, String> {
    // 读取 STT 专用配置，降级到 LLM 配置
    let stt_key = state
        .storage
        .get_setting("voice.stt_api_key")
        .await
        .map_err(|e| format!("{e:#}"))?
        .unwrap_or_default();
    let api_key = if !stt_key.is_empty() {
        stt_key
    } else {
        state
            .storage
            .get_setting("llm.api_key")
            .await
            .map_err(|e| format!("{e:#}"))?
            .unwrap_or_default()
    };

    let stt_url = state
        .storage
        .get_setting("voice.stt_base_url")
        .await
        .map_err(|e| format!("{e:#}"))?
        .unwrap_or_default();
    let base_url = if !stt_url.is_empty() {
        stt_url
    } else {
        state
            .storage
            .get_setting("llm.base_url")
            .await
            .map_err(|e| format!("{e:#}"))?
            .unwrap_or_default()
    };

    // 读取 STT 模型（默认 whisper-1）
    let stt_model = state
        .storage
        .get_setting("voice.stt_model")
        .await
        .map_err(|e| format!("{e:#}"))?
        .unwrap_or_else(|| "whisper-1".to_string());

    // 读取 STT 语言（默认 zh，可配 en/ja 等）
    let stt_language = state
        .storage
        .get_setting("voice.stt_language")
        .await
        .map_err(|e| format!("{e:#}"))?
        .unwrap_or_else(|| "zh".to_string());

    if api_key.is_empty() {
        return Err(prefix_error(
            ERR_VALIDATION,
            "未配置 API Key，无法使用语音转写功能（请在设置中配置 LLM API Key 或专用 STT API Key）",
        ));
    }
    if base_url.is_empty() {
        return Err(prefix_error(
            ERR_VALIDATION,
            "未配置 API Base URL，无法使用语音转写功能",
        ));
    }

    // 构建 audio transcriptions URL（复用 chat_completions_url 的智能拼接逻辑）
    let base_url = base_url.trim_end_matches('/');
    let transcription_url = if base_url.ends_with("/audio/transcriptions") {
        base_url.to_string()
    } else if last_path_segment_is_version(base_url) {
        format!("{base_url}/audio/transcriptions")
    } else {
        format!("{base_url}/v1/audio/transcriptions")
    };

    // 构建 HTTP 客户端（禁止代理，铁律一）
    // 超时 90s 支持较长录音（Whisper API 通常 10-30s 处理 60s 音频）
    let client = reqwest::Client::builder()
        .no_proxy()
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(90))
        .build()
        .map_err(|e| prefix_error(ERR_LLM, &format!("构建 HTTP 客户端失败: {e}")))?;

    // 构建 multipart 请求
    // 文件扩展名根据 MIME 类型推断
    let ext = match mime_type {
        "audio/webm" => "webm",
        "audio/ogg" => "ogg",
        "audio/wav" => "wav",
        "audio/mp4" => "mp4",
        "audio/mpeg" => "mp3",
        _ => "webm", // 默认 webm（MediaRecorder 最常用格式）
    };

    let part = reqwest::multipart::Part::bytes(audio_data.to_vec())
        .file_name(format!("audio.{ext}"))
        .mime_str(mime_type)
        .map_err(|e| prefix_error(ERR_LLM, &format!("MIME 类型设置失败: {e}")))?;

    let form = reqwest::multipart::Form::new()
        .text("model", stt_model)
        .text("language", stt_language)
        .text("response_format", "json")
        .part("file", part);

    // 发送请求
    let resp = client
        .post(&transcription_url)
        .bearer_auth(&api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| {
            let msg = if e.is_timeout() {
                "语音转写请求超时（90s），请缩短录音后重试"
            } else if e.is_connect() {
                "无法连接语音转写服务，请检查网络或 API Base URL 配置"
            } else {
                "语音转写请求失败"
            };
            prefix_error(ERR_LLM, &format!("{msg}: {e}"))
        })?;

    // 检查响应状态
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        let truncated = truncate_error_message(&body, 500);
        return Err(prefix_error(
            ERR_LLM,
            &format!("语音转写 API 返回错误 {status}: {truncated}"),
        ));
    }

    // 解析响应 JSON
    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| prefix_error(ERR_LLM, &format!("解析语音转写响应失败: {e}")))?;

    let text = json
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();

    if text.is_empty() {
        return Err(prefix_error(
            ERR_LLM,
            "语音转写返回空文本，可能未检测到语音",
        ));
    }

    Ok(text)
}

/// 检查 URL 最后一段路径是否为 API 版本标识（如 "v1"、"v4"）。
fn last_path_segment_is_version(url: &str) -> bool {
    let last = url.rsplit('/').next().unwrap_or("");
    last.len() > 1 && last.starts_with('v') && last[1..].bytes().all(|b| b.is_ascii_digit())
}

/// 截断错误信息，避免超长响应体刷屏。
fn truncate_error_message(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        return text.to_string();
    }
    let mut end = max_len;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}
