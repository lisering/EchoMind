#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! KvCacheSnapshot 单元测试（Phase 4 Session 24）。
//!
//! 测试覆盖：快照创建、序列化→反序列化往返、文件保存/读取、
//! 魔数校验、版本检查、多层 K/V 数据完整性。

use super::kv_cache::*;
use tempfile::NamedTempFile;

// ---------------------------------------------------------------------------
// 辅助函数
// ---------------------------------------------------------------------------

/// 创建带 2 层数据的测试快照。
fn make_test_snapshot() -> KvCacheSnapshot {
    let mut snapshot = KvCacheSnapshot::new("test-model-v1".to_string(), 42);
    snapshot.add_layer(LayerKvCache::new(0, vec![0xAB; 64], vec![0xCD; 64], 42));
    snapshot.add_layer(LayerKvCache::new(1, vec![0x11; 32], vec![0x22; 32], 42));
    snapshot
}

// ---------------------------------------------------------------------------
// 测试用例
// ---------------------------------------------------------------------------

/// TC-KVC-001：创建空快照，字段正确。
#[test]
fn test_snapshot_creation() {
    let snapshot = KvCacheSnapshot::new("qwen2.5-3b".to_string(), 128);
    assert_eq!(snapshot.model_name(), "qwen2.5-3b");
    assert_eq!(snapshot.context_length(), 128);
    assert!(snapshot.layers().is_empty());
    assert_eq!(snapshot.layer_count(), 0);
}

/// TC-KVC-002：序列化→反序列化往返正确。
#[test]
fn test_serialize_deserialize() {
    let snapshot = make_test_snapshot();
    let bytes = snapshot.serialize().expect("序列化失败");
    let restored = KvCacheSnapshot::deserialize(&bytes).expect("反序列化失败");

    assert_eq!(restored.model_name(), snapshot.model_name());
    assert_eq!(restored.context_length(), snapshot.context_length());
    assert_eq!(restored.layer_count(), snapshot.layer_count());

    // 逐层验证
    for (orig, rest) in snapshot.layers().iter().zip(restored.layers().iter()) {
        assert_eq!(rest.layer_idx(), orig.layer_idx());
        assert_eq!(rest.k_bytes(), orig.k_bytes());
        assert_eq!(rest.v_bytes(), orig.v_bytes());
        assert_eq!(rest.seq_len(), orig.seq_len());
    }
}

/// TC-KVC-003：保存到文件→读回一致。
#[test]
fn test_save_load_file() {
    let snapshot = make_test_snapshot();

    // 创建临时文件路径
    let tmp = NamedTempFile::new().expect("创建临时文件失败");
    let path = tmp.path().to_path_buf();
    // NamedTempFile 句柄 drop 时会删除文件，先 drop
    drop(tmp);

    snapshot.save_to_file(&path).expect("保存失败");
    let restored = KvCacheSnapshot::load_from_file(&path).expect("加载失败");

    assert_eq!(restored.model_name(), snapshot.model_name());
    assert_eq!(restored.context_length(), snapshot.context_length());
    assert_eq!(restored.layer_count(), snapshot.layer_count());

    // 验证第一层 K/V 数据字节级一致
    assert_eq!(
        restored.layers()[0].k_bytes(),
        snapshot.layers()[0].k_bytes()
    );
    assert_eq!(
        restored.layers()[0].v_bytes(),
        snapshot.layers()[0].v_bytes()
    );

    // 清理临时文件
    let _ = std::fs::remove_file(&path);
}

/// TC-KVC-004：错误魔数拒绝。
#[test]
fn test_invalid_magic() {
    let snapshot = make_test_snapshot();
    let mut bytes = snapshot.serialize().expect("序列化失败");
    // 篡改魔数为 "XXXX"
    bytes[0] = b'X';
    bytes[1] = b'X';
    bytes[2] = b'X';
    bytes[3] = b'X';

    let result = KvCacheSnapshot::deserialize(&bytes);
    assert!(result.is_err(), "篡改魔数后应反序列化失败");
    let err_msg = format!("{}", result.err().unwrap());
    assert!(
        err_msg.contains("EMKV") || err_msg.contains("魔数"),
        "错误信息应提及魔数，实际: {err_msg}"
    );
}

/// TC-KVC-005：版本不匹配拒绝。
#[test]
fn test_version_check() {
    let snapshot = make_test_snapshot();
    let mut bytes = snapshot.serialize().expect("序列化失败");
    // 篡改版本号为 999（不兼容）
    bytes[4] = 0xE7; // 999 & 0xFF
    bytes[5] = 0x03; // 999 >> 8

    let result = KvCacheSnapshot::deserialize(&bytes);
    assert!(result.is_err(), "不兼容版本号应反序列化失败");
    let err_msg = format!("{}", result.err().unwrap());
    assert!(
        err_msg.contains("版本") || err_msg.contains("version"),
        "错误信息应提及版本，实际: {err_msg}"
    );
}

/// TC-KVC-006：多层 K/V 数据完整。
#[test]
fn test_multi_layer() {
    let mut snapshot = KvCacheSnapshot::new("multi-layer-model".to_string(), 256);
    // 添加 5 层，每层不同大小
    for i in 0..5 {
        let k = vec![i as u8 + 1; (i + 1) * 16];
        let v = vec![i as u8 + 0x80; (i + 1) * 16];
        snapshot.add_layer(LayerKvCache::new(i, k, v, 256));
    }
    assert_eq!(snapshot.layer_count(), 5);

    let bytes = snapshot.serialize().expect("序列化失败");
    let restored = KvCacheSnapshot::deserialize(&bytes).expect("反序列化失败");

    assert_eq!(restored.layer_count(), 5);

    for (i, layer) in restored.layers().iter().enumerate() {
        assert_eq!(layer.layer_idx(), i);
        assert_eq!(layer.seq_len(), 256);
        // K 数据：填充值为 i+1，长度 (i+1)*16
        let expected_k = vec![i as u8 + 1; (i + 1) * 16];
        assert_eq!(layer.k_bytes(), expected_k, "第 {i} 层 K 数据不匹配");
        // V 数据：填充值为 i+0x80，长度 (i+1)*16
        let expected_v = vec![i as u8 + 0x80; (i + 1) * 16];
        assert_eq!(layer.v_bytes(), expected_v, "第 {i} 层 V 数据不匹配");
    }
}
