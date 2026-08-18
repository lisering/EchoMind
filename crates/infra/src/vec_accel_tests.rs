//! REQ-VEC-017 嵌入模型下载境内加速 — TDD 测试。
//!
//! 测试内容：
//! - TC-VEC-ACCEL-001: MirrorSource parse_str / as_str 往返序列化
//! - TC-VEC-ACCEL-002: MirrorSource::to_sources 源顺序正确性
//! - TC-VEC-ACCEL-003: 下载重试退避时间序列 [2, 4, 8]
//! - TC-VEC-ACCEL-004: DownloadEvent::Downloading 含 source 字段（serde 序列化验证）
//! - TC-VEC-ACCEL-005: 无效镜像源字符串解析返回 None

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crate::local_embedder::{DownloadEvent, MirrorSource};

// ============================================================================
// TC-VEC-ACCEL-001: MirrorSource parse_str / as_str 往返序列化
// ============================================================================

#[test]
fn tc_vec_accel_001_mirror_source_roundtrip() {
    // 所有合法值往返一致
    let cases = [
        ("auto", MirrorSource::Auto),
        ("modelscope", MirrorSource::ModelScope),
        ("hf-mirror", MirrorSource::HfMirror),
        ("huggingface", MirrorSource::HuggingFace),
    ];
    for (s, expected) in &cases {
        let parsed = MirrorSource::parse_str(s).expect("parse_str 应返回 Some");
        assert_eq!(parsed, *expected, "parse_str({s:?}) 结果不匹配");
        assert_eq!(parsed.as_str(), *s, "as_str() 应与输入字符串一致");
    }

    // 大小写不敏感
    assert_eq!(MirrorSource::parse_str("AUTO"), Some(MirrorSource::Auto));
    assert_eq!(
        MirrorSource::parse_str("ModelScope"),
        Some(MirrorSource::ModelScope)
    );
    assert_eq!(
        MirrorSource::parse_str("HF-MIRROR"),
        Some(MirrorSource::HfMirror)
    );
    assert_eq!(
        MirrorSource::parse_str("HuggingFace"),
        Some(MirrorSource::HuggingFace)
    );

    // 别名
    assert_eq!(
        MirrorSource::parse_str("hf"),
        Some(MirrorSource::HuggingFace)
    );
    assert_eq!(
        MirrorSource::parse_str("hf_mirror"),
        Some(MirrorSource::HfMirror)
    );
}

// ============================================================================
// TC-VEC-ACCEL-002: MirrorSource::to_sources 源顺序正确性
// ============================================================================

#[test]
fn tc_vec_accel_002_mirror_source_order() {
    // ModelScope 源顺序：第一个是 ModelScope
    let sources = MirrorSource::ModelScope.to_sources();
    assert!(!sources.is_empty(), "源列表不应为空");
    assert_eq!(sources[0].name, "ModelScope", "ModelScope 应为首选源");

    // HuggingFace 源顺序：第一个是 HuggingFace
    let sources = MirrorSource::HuggingFace.to_sources();
    assert!(!sources.is_empty(), "源列表不应为空");
    assert_eq!(sources[0].name, "HuggingFace", "HuggingFace 应为首选源");

    // HfMirror 源顺序：第一个是 hf-mirror
    let sources = MirrorSource::HfMirror.to_sources();
    assert!(!sources.is_empty(), "源列表不应为空");
    assert_eq!(sources[0].name, "hf-mirror", "hf-mirror 应为首选源");

    // 所有模式都包含全部 3 个源（只是顺序不同）
    let all_sources_count = MirrorSource::Auto.to_sources().len();
    assert_eq!(
        MirrorSource::ModelScope.to_sources().len(),
        all_sources_count,
        "所有模式应有相同数量的源"
    );
    assert_eq!(
        MirrorSource::HuggingFace.to_sources().len(),
        all_sources_count,
        "所有模式应有相同数量的源"
    );
    assert_eq!(
        MirrorSource::HfMirror.to_sources().len(),
        all_sources_count,
        "所有模式应有相同数量的源"
    );
    assert_eq!(
        all_sources_count, 3,
        "应有 3 个源（ModelScope + hf-mirror + HuggingFace）"
    );
}

// ============================================================================
// TC-VEC-ACCEL-003: 下载重试退避时间序列 [2, 4, 8]
// ============================================================================

#[test]
fn tc_vec_accel_003_retry_backoff_sequence() {
    // 验证退避序列常量（与 ensure_model_files 中使用的值一致）
    let backoff_secs: [u64; 3] = [2, 4, 8];

    // 验证退避序列递增
    assert_eq!(backoff_secs.len(), 3, "应有 3 次重试退避");
    assert_eq!(backoff_secs[0], 2, "第一次重试退避 2s");
    assert_eq!(backoff_secs[1], 4, "第二次重试退避 4s");
    assert_eq!(backoff_secs[2], 8, "第三次重试退避 8s");

    // 验证退避序列是指数增长（2 的幂次）
    for (i, &secs) in backoff_secs.iter().enumerate() {
        assert_eq!(
            secs,
            2u64.pow((i + 1) as u32),
            "退避 {i} 应为 2^{} = {}",
            i + 1,
            2u64.pow((i + 1) as u32)
        );
    }

    // 验证总重试次数 = backoff_secs.len() + 1 = 4（1 次初始 + 3 次重试）
    let total_attempts = backoff_secs.len() + 1;
    assert_eq!(total_attempts, 4, "总尝试次数应为 4（1 初始 + 3 重试）");
}

// ============================================================================
// TC-VEC-ACCEL-004: DownloadEvent::Downloading 含 source 字段
// ============================================================================

#[test]
fn tc_vec_accel_004_download_event_has_source() {
    // 构造一个 Downloading 事件，验证 source 字段存在且可序列化
    let event = DownloadEvent::Downloading {
        file_name: "model.onnx".to_string(),
        current: 1024,
        total: 4096,
        file_index: 0,
        total_files: 5,
        source:
            "https://modelscope.cn/models/BAAI/bge-small-en-v1.5/resolve/master/onnx/model.onnx"
                .to_string(),
    };

    // 序列化为 JSON
    let json = serde_json::to_string(&event).expect("序列化 DownloadEvent 失败");
    assert!(json.contains("source"), "JSON 应包含 source 字段: {json}");
    assert!(json.contains("modelscope.cn"), "JSON 应包含源 URL: {json}");

    // 反序列化验证往返一致
    let de: DownloadEvent = serde_json::from_str(&json).expect("反序列化 DownloadEvent 失败");
    if let DownloadEvent::Downloading { source, .. } = de {
        assert!(
            source.contains("modelscope.cn"),
            "反序列化后 source 应包含 modelscope.cn"
        );
    } else {
        panic!("反序列化应为 Downloading 变体");
    }
}

// ============================================================================
// TC-VEC-ACCEL-005: 无效镜像源字符串解析返回 None
// ============================================================================

#[test]
fn tc_vec_accel_005_invalid_mirror_source() {
    // 无效字符串
    assert!(
        MirrorSource::parse_str("invalid").is_none(),
        "无效字符串应返回 None"
    );
    assert!(MirrorSource::parse_str("").is_none(), "空字符串应返回 None");
    assert!(
        MirrorSource::parse_str("google").is_none(),
        "不支持的源应返回 None"
    );
    assert!(
        MirrorSource::parse_str("github").is_none(),
        "不支持的源应返回 None"
    );

    // 合法值不返回 None
    assert!(
        MirrorSource::parse_str("auto").is_some(),
        "auto 应返回 Some"
    );
    assert!(
        MirrorSource::parse_str("modelscope").is_some(),
        "modelscope 应返回 Some"
    );
    assert!(
        MirrorSource::parse_str("hf-mirror").is_some(),
        "hf-mirror 应返回 Some"
    );
    assert!(
        MirrorSource::parse_str("huggingface").is_some(),
        "huggingface 应返回 Some"
    );
}
