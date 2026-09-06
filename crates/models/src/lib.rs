//! EchoMind 数据契约层（六边形架构共享模型）。
//! 对应 SRS：REQ-ARCH-000；字段语义对应 REQ-ING-001（Document）、
//! REQ-VEC-001/004（Chunk / DocStatus）、REQ-RAG-002（RetrievalResult / ChatMessage）。
//! 本 crate 只定义纯数据结构，不含任何业务逻辑与外部服务依赖。

// ============================================================================
// 子模块声明
// ============================================================================

pub mod cache;
pub mod chat;
pub mod code_execution;
pub mod document;
pub mod download;
pub mod embed_comparison;
pub mod events;
pub mod mcp;
pub mod memory;
pub mod prompt_template;
pub mod rag_eval;
pub mod settings;
pub mod sync;
pub mod workflow;
pub mod workspace;

// ============================================================================
// 重新导出所有公共类型（保持向后兼容的 `echomind_models::TypeName` API）
// ============================================================================

pub use cache::*;
pub use chat::*;
pub use code_execution::*;
pub use document::*;
pub use download::*;
pub use embed_comparison::*;
pub use events::*;
pub use mcp::*;
pub use memory::*;
pub use prompt_template::*;
pub use rag_eval::*;
pub use settings::*;
pub use sync::*;
pub use workflow::*;
pub use workspace::*;

// ============================================================================
// 全局 serde 默认值辅助函数
// ============================================================================

/// 默认上下文 token 限制（REQ-RAG-017）。
pub fn default_context_token_limit() -> usize {
    4096
}

/// serde 默认值辅助：返回 `true`（用于 `#[serde(default = "default_true")]`）。
pub fn default_true() -> bool {
    true
}

/// 默认缓存启用开关（REQ-PERF-001）。
pub fn default_cache_enabled() -> bool {
    true
}

/// 默认缓存存活时间（秒，REQ-PERF-001）。
pub fn default_cache_ttl() -> u64 {
    86400
}

/// 默认语义匹配阈值（REQ-PERF-001）。
pub fn default_cache_semantic_threshold() -> f32 {
    0.92
}

/// 默认压缩触发阈值（S69 Token Budget）。
pub fn default_compaction_threshold() -> f32 {
    0.8
}

/// 默认最近消息保留比例（S69 Token Budget）。
pub fn default_recent_keep_ratio() -> f32 {
    0.67
}

/// 默认触发压缩的最小消息数（S69 Token Budget）。
pub fn default_min_messages_to_compact() -> usize {
    3
}

// ============================================================================
// 测试模块
// ============================================================================

#[cfg(test)]
mod llm_model_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn test_llm_mode_default() {
        assert_eq!(LlmMode::default(), LlmMode::Remote);
    }

    #[test]
    fn test_llm_mode_serde() {
        let mode = LlmMode::Remote;
        let json = serde_json::to_string(&mode).expect("serialize");
        let deserialized: LlmMode = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(mode, deserialized);
    }

    #[test]
    fn test_model_info_serialize() {
        let info = ModelInfo {
            filename: "qwen2.5-7b-instruct-q4_k_m.gguf".to_string(),
            path: "/data/models/llm/qwen2.5-7b-instruct-q4_k_m.gguf".to_string(),
            size_bytes: 4_400_000_000,
            architecture: "qwen2.5".to_string(),
            param_size: "7B".to_string(),
            quantization: "Q4_K_M".to_string(),
        };
        let json = serde_json::to_string(&info).expect("serialize");
        assert!(json.contains("qwen2.5-7b-instruct-q4_k_m.gguf"));
        assert!(json.contains("Q4_K_M"));
    }

    #[test]
    fn test_settings_payload_llm_mode_default() {
        let json = r#"{
            "has_llm_config": false,
            "base_url": "",
            "model": "",
            "api_key_masked": "",
            "vlm_enabled": false,
            "hybrid_search": true,
            "rerank_enabled": false,
            "hyde_enabled": false,
            "agent_enabled": false,
            "embedding_model": "all-MiniLM-L6-v2",
            "context_token_limit": 4096
        }"#;
        let payload: SettingsPayload = serde_json::from_str(json).expect("deserialize");
        assert_eq!(payload.llm_mode, "");
        assert_eq!(payload.local_model, "");
        assert!(payload.llm_sampling.is_none());
        assert_eq!(payload.token_budget, 0);
    }

    #[test]
    fn test_sampling_params_default_all_none() {
        let params = LlmSamplingParams::default();
        assert!(params.temperature.is_none());
        assert!(params.top_p.is_none());
        assert!(params.top_k.is_none());
        assert!(params.max_tokens.is_none());
        assert!(params.frequency_penalty.is_none());
        assert!(params.presence_penalty.is_none());
    }

    #[test]
    fn test_sampling_params_serde_roundtrip() {
        let params = LlmSamplingParams {
            temperature: Some(0.8),
            top_p: Some(0.95),
            top_k: Some(40),
            max_tokens: Some(2048),
            frequency_penalty: Some(0.5),
            presence_penalty: Some(0.2),
        };
        let json = serde_json::to_string(&params).expect("serialize");
        let deserialized: LlmSamplingParams = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_sampling_params_skip_none_fields() {
        let params = LlmSamplingParams {
            temperature: Some(0.7),
            top_p: None,
            top_k: None,
            max_tokens: Some(1024),
            frequency_penalty: None,
            presence_penalty: None,
        };
        let json = serde_json::to_string(&params).expect("serialize");
        assert!(json.contains("temperature"));
        assert!(json.contains("max_tokens"));
        assert!(!json.contains("top_p"));
        assert!(!json.contains("top_k"));
        assert!(!json.contains("frequency_penalty"));
        assert!(!json.contains("presence_penalty"));
    }

    #[test]
    fn test_settings_payload_with_sampling_params() {
        let params = LlmSamplingParams {
            temperature: Some(0.5),
            max_tokens: Some(512),
            ..Default::default()
        };
        let payload = SettingsPayload {
            has_llm_config: true,
            base_url: "https://api.openai.com".to_string(),
            model: "gpt-4o-mini".to_string(),
            api_key_masked: "****1234".to_string(),
            vlm_enabled: false,
            hybrid_search: true,
            rerank_enabled: false,
            hyde_enabled: false,
            agent_enabled: false,
            embedding_model: "all-MiniLM-L6-v2".to_string(),
            context_token_limit: 4096,
            llm_mode: "local".to_string(),
            local_model: "test.gguf".to_string(),
            llm_sampling: Some(params),
            token_budget: 0,
            cache_enabled: true,
            cache_ttl_secs: 86400,
            cache_semantic_threshold: 0.92,
            cache_privacy_mode: false,
            quality_gate_enabled: false,
            sub_agent_enabled: false,
            progressive_injection: false,
            speculative_enabled: false,
            graph_retriever_enabled: false,
            contextual_retrieval: true,
        };
        let json = serde_json::to_string(&payload).expect("serialize");
        let back: SettingsPayload = serde_json::from_str(&json).expect("deserialize");
        let sampling = back.llm_sampling.expect("sampling should exist");
        assert_eq!(sampling.temperature, Some(0.5));
        assert_eq!(sampling.max_tokens, Some(512));
        assert!(sampling.top_p.is_none());
    }

    #[test]
    fn test_token_usage_default() {
        let usage = TokenUsage::default();
        assert_eq!(usage.prompt_tokens, 0);
        assert_eq!(usage.completion_tokens, 0);
        assert_eq!(usage.total_tokens, 0);
    }

    #[test]
    fn test_token_usage_serde_roundtrip() {
        let usage = TokenUsage {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
        };
        let json = serde_json::to_string(&usage).expect("serialize");
        let back: TokenUsage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(usage, back);
        assert!(json.contains("prompt_tokens"));
        assert!(json.contains("completion_tokens"));
        assert!(json.contains("total_tokens"));
    }

    #[test]
    fn test_conversation_cost_default() {
        let cost = ConversationCost::default();
        assert_eq!(cost.total_prompt_tokens, 0);
        assert_eq!(cost.total_completion_tokens, 0);
        assert_eq!(cost.total_tokens, 0);
        assert_eq!(cost.exchange_count, 0);
        assert_eq!(cost.token_budget, 0);
    }

    #[test]
    fn test_conversation_cost_serde_roundtrip() {
        let cost = ConversationCost {
            total_prompt_tokens: 5000,
            total_completion_tokens: 2500,
            total_tokens: 7500,
            exchange_count: 10,
            token_budget: 100000,
        };
        let json = serde_json::to_string(&cost).expect("serialize");
        let back: ConversationCost = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(cost, back);
        assert!(json.contains("total_prompt_tokens"));
        assert!(json.contains("token_budget"));
        assert!(json.contains("exchange_count"));
    }

    #[test]
    fn test_cache_level_serde_roundtrip() {
        for level in [
            CacheLevel::Exact,
            CacheLevel::Semantic,
            CacheLevel::Retrieval,
        ] {
            let json = serde_json::to_string(&level).expect("serialize");
            let back: CacheLevel = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(level, back);
        }
    }

    #[test]
    fn test_cache_stats_default() {
        let stats = CacheStats::default();
        assert!(!stats.enabled);
        assert_eq!(stats.exact_hits, 0);
        assert_eq!(stats.semantic_hits, 0);
        assert_eq!(stats.retrieval_hits, 0);
        assert_eq!(stats.total_queries, 0);
        assert_eq!(stats.cache_size_entries, 0);
        assert_eq!(stats.estimated_token_saved, 0);
    }

    #[test]
    fn test_cache_settings_default() {
        let settings = CacheSettingsPayload::default();
        assert!(settings.enabled);
        assert_eq!(settings.ttl_secs, 86400);
        assert!((settings.semantic_threshold - 0.92).abs() < 1e-6);
        assert!(!settings.privacy_mode);
    }

    #[test]
    fn test_cache_settings_serde_roundtrip() {
        let settings = CacheSettingsPayload {
            enabled: false,
            ttl_secs: 3600,
            semantic_threshold: 0.95,
            privacy_mode: true,
        };
        let json = serde_json::to_string(&settings).expect("serialize");
        let back: CacheSettingsPayload = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(settings.enabled, back.enabled);
        assert_eq!(settings.ttl_secs, back.ttl_secs);
        assert!((settings.semantic_threshold - back.semantic_threshold).abs() < 1e-6);
        assert_eq!(settings.privacy_mode, back.privacy_mode);
    }

    #[test]
    fn test_settings_payload_cache_defaults() {
        let json = r#"{
            "has_llm_config": false,
            "base_url": "",
            "model": "",
            "api_key_masked": "",
            "vlm_enabled": false,
            "hybrid_search": true,
            "rerank_enabled": false,
            "hyde_enabled": false,
            "agent_enabled": false,
            "embedding_model": "all-MiniLM-L6-v2",
            "context_token_limit": 4096
        }"#;
        let payload: SettingsPayload = serde_json::from_str(json).expect("deserialize");
        assert!(payload.cache_enabled);
        assert_eq!(payload.cache_ttl_secs, 86400);
        assert!((payload.cache_semantic_threshold - 0.92).abs() < 1e-6);
        assert!(!payload.cache_privacy_mode);
    }

    #[test]
    fn test_prompt_template_serde_roundtrip() {
        let tmpl = PromptTemplate {
            id: "test-id-123".to_string(),
            name: "explain".to_string(),
            label: "解释概念".to_string(),
            description: "用通俗易懂的方式解释概念".to_string(),
            icon: "💡".to_string(),
            prompt_template: "请用通俗易懂的方式解释以下概念：{query}".to_string(),
            created_at: 1700000000,
            updated_at: 1700000001,
        };
        let json = serde_json::to_string(&tmpl).expect("serialize");
        let back: PromptTemplate = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(tmpl, back);
        assert!(json.contains("prompt_template"));
        assert!(json.contains("{query}"));
    }

    #[test]
    fn test_prompt_template_new() {
        let tmpl = PromptTemplate::new(
            "review".to_string(),
            "代码审查".to_string(),
            "审查代码质量".to_string(),
            "🔍".to_string(),
            "请审查以下代码：{query}".to_string(),
        );
        assert!(!tmpl.id.is_empty());
        assert_eq!(tmpl.name, "review");
        assert_eq!(tmpl.label, "代码审查");
        assert_eq!(tmpl.created_at, tmpl.updated_at);
    }

    #[test]
    fn test_prompt_template_valid_name() {
        assert!(PromptTemplate::is_valid_name("summary"));
        assert!(PromptTemplate::is_valid_name("my_command_1"));
        assert!(PromptTemplate::is_valid_name("a"));
        assert!(!PromptTemplate::is_valid_name(""));
        assert!(!PromptTemplate::is_valid_name("Summary"));
        assert!(!PromptTemplate::is_valid_name("my-command"));
        assert!(!PromptTemplate::is_valid_name("中文"));
        assert!(!PromptTemplate::is_valid_name(&"x".repeat(33)));
    }

    #[test]
    fn test_prompt_template_query_placeholder() {
        assert!(PromptTemplate::has_query_placeholder("请总结：{query}"));
        assert!(PromptTemplate::has_query_placeholder("{query}"));
        assert!(!PromptTemplate::has_query_placeholder("请总结以下内容"));
        assert!(!PromptTemplate::has_query_placeholder(""));
    }

    #[test]
    fn test_wiki_link_new() {
        let link = WikiLink::new(
            "doc-001".to_string(),
            "设计文档".to_string(),
            "chunk-001".to_string(),
        );
        assert!(!link.id.is_empty());
        assert_eq!(link.source_doc_id, "doc-001");
        assert_eq!(link.target, "设计文档");
        assert_eq!(link.chunk_id, "chunk-001");
        assert!(link.created_at > 0);
    }

    #[test]
    fn test_wiki_link_serde_roundtrip() {
        let link = WikiLink::new(
            "doc-abc".to_string(),
            "API Design".to_string(),
            "chunk-xyz".to_string(),
        );
        let json = serde_json::to_string(&link).expect("serialize");
        let back: WikiLink = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(link.id, back.id);
        assert_eq!(link.source_doc_id, back.source_doc_id);
        assert_eq!(link.target, back.target);
        assert_eq!(link.chunk_id, back.chunk_id);
        assert_eq!(link.created_at, back.created_at);
    }

    #[test]
    fn test_wiki_link_fields_non_empty() {
        let link = WikiLink::new(
            "source-doc".to_string(),
            "target-note".to_string(),
            "chunk-id".to_string(),
        );
        assert!(!link.id.is_empty(), "id 不应为空");
        assert!(!link.source_doc_id.is_empty(), "source_doc_id 不应为空");
        assert!(!link.target.is_empty(), "target 不应为空");
        assert!(!link.chunk_id.is_empty(), "chunk_id 不应为空");
    }
}

#[cfg(test)]
mod conversation_tree_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn test_conversation_tree_node_new() {
        let node =
            ConversationTreeNode::new("conv-001", "turn-abc", 1, 1700000000, "这是一条测试消息");
        assert_eq!(node.node_id, "turn-abc#1");
        assert_eq!(node.conversation_id, "conv-001");
        assert_eq!(node.turn_group, "turn-abc");
        assert_eq!(node.version, 1);
        assert_eq!(node.created_at, 1700000000);
        assert_eq!(node.preview, "这是一条测试消息");
        assert!(node.is_root());
        assert!(node.is_leaf());
        assert!(node.parent_message_id.is_none());
        assert!(node.child_message_ids.is_empty());
        assert!(node.active_child.is_none());
    }

    #[test]
    fn test_conversation_tree_empty_and_find() {
        let tree = ConversationTree::empty("conv-002");
        assert_eq!(tree.conversation_id, "conv-002");
        assert!(tree.nodes.is_empty());
        assert!(tree.root_ids.is_empty());
        assert!(tree.active_path.is_empty());
        assert!(tree.find("nonexistent").is_none());

        let node = ConversationTreeNode::new("conv-002", "turn-1", 1, 1700000000, "hello");
        let mut tree = ConversationTree::empty("conv-002");
        tree.nodes.push(node.clone());
        tree.root_ids.push(node.node_id.clone());
        let found = tree.find(&node.node_id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().node_id, node.node_id);
        assert_eq!(tree.root_ids.len(), 1);
    }

    #[test]
    fn test_preview_truncation() {
        let long_text = "a".repeat(200);
        let node = ConversationTreeNode::new("conv", "turn", 1, 0, &long_text);
        assert_eq!(node.preview.chars().count(), 101);
        assert!(node.preview.ends_with('…'));

        let short_text = "short message";
        let node2 = ConversationTreeNode::new("conv", "turn", 1, 0, short_text);
        assert_eq!(node2.preview, short_text);
    }

    #[test]
    fn test_preview_truncation_utf8_safe() {
        let long_chinese = "测".repeat(150);
        let node = ConversationTreeNode::new("conv", "turn", 1, 0, &long_chinese);
        assert_eq!(node.preview.chars().count(), 101);
        assert!(node.preview.ends_with('…'));
    }
}

#[cfg(test)]
mod pending_input_tests {
    use super::PendingInput;

    #[test]
    fn test_pending_input_new() {
        let pi = PendingInput::new(
            "conv-001".to_string(),
            "hello world".to_string(),
            "queue".to_string(),
        );
        assert!(!pi.id.is_empty());
        assert_eq!(pi.conversation_id, "conv-001");
        assert_eq!(pi.content, "hello world");
        assert_eq!(pi.delivery, "queue");
        assert!(pi.promoted_seq.is_none());
    }

    #[test]
    fn test_pending_input_steer() {
        let pi = PendingInput::new(
            "conv-002".to_string(),
            "stop and answer this".to_string(),
            "steer".to_string(),
        );
        assert_eq!(pi.delivery, "steer");
        assert!(pi.promoted_seq.is_none());
    }

    #[test]
    fn test_pending_input_unique_id() {
        let pi1 = PendingInput::new("c".to_string(), "a".to_string(), "queue".to_string());
        let pi2 = PendingInput::new("c".to_string(), "a".to_string(), "queue".to_string());
        assert_ne!(pi1.id, pi2.id);
    }

    #[test]
    fn test_pending_input_created_at() {
        let before = chrono::Utc::now().timestamp();
        let pi = PendingInput::new("c".to_string(), "a".to_string(), "queue".to_string());
        let after = chrono::Utc::now().timestamp();
        assert!(pi.created_at >= before && pi.created_at <= after);
    }
}

#[cfg(test)]
mod session_todo_tests {
    use super::*;

    #[test]
    fn test_session_todo_new() {
        let todo = SessionTodo::new("conv-001".to_string(), "完成数据分析".to_string(), 0);
        assert!(!todo.id.is_empty());
        assert_eq!(todo.conversation_id, "conv-001");
        assert_eq!(todo.content, "完成数据分析");
        assert_eq!(todo.status, TodoStatus::Pending);
        assert_eq!(todo.priority, TodoPriority::Medium);
        assert_eq!(todo.position, 0);
        assert!(todo.created_at > 0);
    }

    #[test]
    fn test_todo_status_roundtrip() {
        for status in [
            TodoStatus::Pending,
            TodoStatus::InProgress,
            TodoStatus::Completed,
        ] {
            let s = status.as_str();
            assert_eq!(TodoStatus::from_db_str(s), Some(status));
        }
        assert!(TodoStatus::from_db_str("invalid").is_none());
    }

    #[test]
    fn test_todo_priority_roundtrip() {
        for prio in [TodoPriority::Low, TodoPriority::Medium, TodoPriority::High] {
            let s = prio.as_str();
            assert_eq!(TodoPriority::from_db_str(s), Some(prio));
        }
        assert!(TodoPriority::from_db_str("invalid").is_none());
    }

    #[test]
    fn test_session_todo_unique_id() {
        let t1 = SessionTodo::new("c".to_string(), "a".to_string(), 0);
        let t2 = SessionTodo::new("c".to_string(), "a".to_string(), 0);
        assert_ne!(t1.id, t2.id);
    }
}

#[cfg(test)]
mod provenance_tag_tests {
    #![allow(clippy::unwrap_used)]
    use super::{GenerationParams, ProvenanceTag, RagParams, StripResult};

    #[test]
    fn test_provenance_tag_new() {
        let tag = ProvenanceTag::new(
            "conv-001".to_string(),
            3,
            "对话：知识产权法咨询 的第 3 轮".to_string(),
        );
        assert_eq!(tag.conversation_id, "conv-001");
        assert_eq!(tag.message_seq, 3);
        assert!(tag.captured_at > 0);
        assert_eq!(tag.source_label, "对话：知识产权法咨询 的第 3 轮");
    }

    #[test]
    fn test_provenance_tag_with_timestamp() {
        let tag = ProvenanceTag::with_timestamp(
            "conv-002".to_string(),
            1,
            1700000000,
            "对话：测试".to_string(),
        );
        assert_eq!(tag.captured_at, 1700000000);
    }

    #[test]
    fn test_provenance_tag_said_in_suffix() {
        let tag = ProvenanceTag::new(
            "conv-001".to_string(),
            2,
            "对话：法律咨询 的第 2 轮".to_string(),
        );
        let suffix = tag.said_in_suffix();
        assert_eq!(suffix, " (said in 对话：法律咨询 的第 2 轮)");
    }

    #[test]
    fn test_provenance_tag_serde_roundtrip() {
        let tag = ProvenanceTag::with_timestamp(
            "conv-003".to_string(),
            5,
            1700000000,
            "对话：Rust 讨论 的第 5 轮".to_string(),
        );
        let json = serde_json::to_string(&tag).unwrap();
        let deserialized: ProvenanceTag = serde_json::from_str(&json).unwrap();
        assert_eq!(tag, deserialized);
    }

    #[test]
    fn test_rag_params_default() {
        let params = RagParams::default();
        assert_eq!(params.top_k, 8);
        assert!((params.score_threshold - 0.0).abs() < f32::EPSILON);
        assert!(params.chunk_expansion_enabled);
        assert_eq!(params.chunk_expansion_window, 1);
    }

    #[test]
    fn test_rag_params_clamped() {
        let params = RagParams {
            top_k: 100,
            score_threshold: -1.5,
            chunk_expansion_enabled: false,
            chunk_expansion_window: 10,
        };
        let clamped = params.clamped();
        assert_eq!(clamped.top_k, 20);
        assert!((clamped.score_threshold - 0.0).abs() < f32::EPSILON);
        assert!(!clamped.chunk_expansion_enabled);
        assert_eq!(clamped.chunk_expansion_window, 3);
    }

    #[test]
    fn test_rag_params_serde_roundtrip() {
        let params = RagParams::default();
        let json = serde_json::to_string(&params).unwrap();
        let deserialized: RagParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_generation_params_default() {
        let params = GenerationParams::default();
        assert!((params.temperature - 0.7).abs() < f32::EPSILON);
        assert_eq!(params.max_tokens, 4096);
        assert!((params.top_p - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_generation_params_clamped() {
        let params = GenerationParams {
            temperature: 5.0,
            max_tokens: 100,
            top_p: 2.5,
        };
        let clamped = params.clamped();
        assert!((clamped.temperature - 2.0).abs() < f32::EPSILON);
        assert_eq!(clamped.max_tokens, 256);
        assert!((clamped.top_p - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_generation_params_serde_roundtrip() {
        let params = GenerationParams::default();
        let json = serde_json::to_string(&params).unwrap();
        let deserialized: GenerationParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_strip_result_serde_roundtrip() {
        let result = StripResult {
            stripped_count: 10,
            summary_inserted: true,
            stripped_message_ids: vec!["msg-1".to_string(), "msg-2".to_string()],
            estimated_tokens_saved: 500,
            summary: "用户讨论了 Rust 异步编程和 Tauri 框架".to_string(),
            kept_count: 6,
        };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: StripResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}

#[cfg(test)]
mod embed_metric_scores_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn metric(metric_type: RagMetricType, score: f32) -> RagEvalMetric {
        RagEvalMetric {
            metric_type,
            score,
            details: None,
        }
    }

    #[test]
    fn tc_embed_001_empty_metrics() {
        let scores = EmbedMetricScores::from_metrics_avg(&[]);
        assert!((scores.hit_rate - 0.0).abs() < f32::EPSILON);
        assert!((scores.mrr - 0.0).abs() < f32::EPSILON);
        assert!((scores.ndcg - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn tc_embed_002_single_metric() {
        let metrics = vec![vec![metric(RagMetricType::HitRate, 0.8)]];
        let scores = EmbedMetricScores::from_metrics_avg(&metrics);
        assert!((scores.hit_rate - 0.8).abs() < f32::EPSILON);
        assert!((scores.mrr - 0.0).abs() < f32::EPSILON);
        assert!((scores.ndcg - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn tc_embed_003_average_calculation() {
        let metrics = vec![
            vec![
                metric(RagMetricType::HitRate, 0.6),
                metric(RagMetricType::MRR, 0.4),
            ],
            vec![
                metric(RagMetricType::HitRate, 0.8),
                metric(RagMetricType::MRR, 0.6),
            ],
        ];
        let scores = EmbedMetricScores::from_metrics_avg(&metrics);
        assert!((scores.hit_rate - 0.7).abs() < f32::EPSILON);
        assert!((scores.mrr - 0.5).abs() < f32::EPSILON);
        assert!((scores.ndcg - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn tc_embed_004_mixed_metrics() {
        let metrics = vec![
            vec![
                metric(RagMetricType::HitRate, 0.9),
                metric(RagMetricType::MRR, 0.7),
                metric(RagMetricType::NDCG, 0.5),
            ],
            vec![
                metric(RagMetricType::HitRate, 0.7),
                metric(RagMetricType::MRR, 0.3),
                metric(RagMetricType::NDCG, 0.9),
            ],
            vec![
                metric(RagMetricType::HitRate, 0.5),
                metric(RagMetricType::MRR, 0.5),
                metric(RagMetricType::NDCG, 0.7),
            ],
        ];
        let scores = EmbedMetricScores::from_metrics_avg(&metrics);
        assert!((scores.hit_rate - 0.7).abs() < f32::EPSILON);
        assert!((scores.mrr - 0.5).abs() < f32::EPSILON);
        assert!((scores.ndcg - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn tc_embed_005_non_embedding_metrics_ignored() {
        let metrics = vec![vec![
            metric(RagMetricType::Faithfulness, 1.0),
            metric(RagMetricType::AnswerRelevance, 0.9),
            metric(RagMetricType::ContextPrecision, 0.8),
        ]];
        let scores = EmbedMetricScores::from_metrics_avg(&metrics);
        assert!((scores.hit_rate - 0.0).abs() < f32::EPSILON);
        assert!((scores.mrr - 0.0).abs() < f32::EPSILON);
        assert!((scores.ndcg - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn tc_embed_006_partial_metrics() {
        let metrics = vec![vec![metric(RagMetricType::HitRate, 1.0)]];
        let scores = EmbedMetricScores::from_metrics_avg(&metrics);
        assert!((scores.hit_rate - 1.0).abs() < f32::EPSILON);
        assert!((scores.mrr - 0.0).abs() < f32::EPSILON);
        assert!((scores.ndcg - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn tc_embed_007_all_zero_scores() {
        let metrics = vec![vec![
            metric(RagMetricType::HitRate, 0.0),
            metric(RagMetricType::MRR, 0.0),
            metric(RagMetricType::NDCG, 0.0),
        ]];
        let scores = EmbedMetricScores::from_metrics_avg(&metrics);
        assert!((scores.hit_rate - 0.0).abs() < f32::EPSILON);
        assert!((scores.mrr - 0.0).abs() < f32::EPSILON);
        assert!((scores.ndcg - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn tc_embed_008_all_one_scores() {
        let metrics = vec![vec![
            metric(RagMetricType::HitRate, 1.0),
            metric(RagMetricType::MRR, 1.0),
            metric(RagMetricType::NDCG, 1.0),
        ]];
        let scores = EmbedMetricScores::from_metrics_avg(&metrics);
        assert!((scores.hit_rate - 1.0).abs() < f32::EPSILON);
        assert!((scores.mrr - 1.0).abs() < f32::EPSILON);
        assert!((scores.ndcg - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn tc_embed_009_empty_inner_vecs() {
        let metrics = vec![vec![], vec![]];
        let scores = EmbedMetricScores::from_metrics_avg(&metrics);
        assert!((scores.hit_rate - 0.0).abs() < f32::EPSILON);
        assert!((scores.mrr - 0.0).abs() < f32::EPSILON);
        assert!((scores.ndcg - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn tc_embed_010_large_sample_average() {
        let mut inner = Vec::new();
        for i in 0..1000 {
            inner.push(metric(RagMetricType::HitRate, i as f32 / 1000.0));
        }
        let metrics = vec![inner];
        let scores = EmbedMetricScores::from_metrics_avg(&metrics);
        assert!((scores.hit_rate - 0.4995).abs() < 0.001);
    }

    #[test]
    fn tc_embed_011_zero_constructor() {
        let scores = EmbedMetricScores::zero();
        assert!((scores.hit_rate - 0.0).abs() < f32::EPSILON);
        assert!((scores.mrr - 0.0).abs() < f32::EPSILON);
        assert!((scores.ndcg - 0.0).abs() < f32::EPSILON);
    }
}

#[cfg(test)]
mod mcp_server_config_validate_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn valid_stdio_config() -> McpServerConfig {
        McpServerConfig::new_stdio("id-1", "Test Server", "npx")
    }

    fn valid_sse_config() -> McpServerConfig {
        McpServerConfig::new_sse("id-2", "SSE Server", "http://localhost:3000")
    }

    #[test]
    fn tc_mcp_valid_001_stdio_ok() {
        assert!(valid_stdio_config().validate().is_ok());
    }

    #[test]
    fn tc_mcp_valid_002_sse_ok() {
        assert!(valid_sse_config().validate().is_ok());
    }

    #[test]
    fn tc_mcp_valid_003_empty_name() {
        let mut cfg = valid_stdio_config();
        cfg.name = "".to_string();
        let err = cfg.validate().unwrap_err();
        assert!(err.contains("名称"), "错误应提及名称: {err}");
    }

    #[test]
    fn tc_mcp_valid_004_whitespace_name() {
        let mut cfg = valid_stdio_config();
        cfg.name = "   ".to_string();
        let err = cfg.validate().unwrap_err();
        assert!(err.contains("名称"), "空白名称应报错: {err}");
    }

    #[test]
    fn tc_mcp_valid_005_empty_id() {
        let mut cfg = valid_stdio_config();
        cfg.id = "".to_string();
        let err = cfg.validate().unwrap_err();
        assert!(err.contains("ID"), "错误应提及 ID: {err}");
    }

    #[test]
    fn tc_mcp_valid_006_whitespace_id() {
        let mut cfg = valid_stdio_config();
        cfg.id = "  ".to_string();
        let err = cfg.validate().unwrap_err();
        assert!(err.contains("ID"), "空白 ID 应报错: {err}");
    }

    #[test]
    fn tc_mcp_valid_007_stdio_missing_command() {
        let mut cfg = valid_stdio_config();
        cfg.command = None;
        let err = cfg.validate().unwrap_err();
        assert!(err.contains("command"), "缺少 command 应报错: {err}");
    }

    #[test]
    fn tc_mcp_valid_008_stdio_empty_command() {
        let mut cfg = valid_stdio_config();
        cfg.command = Some("".to_string());
        let err = cfg.validate().unwrap_err();
        assert!(err.contains("command"), "空 command 应报错: {err}");
    }

    #[test]
    fn tc_mcp_valid_009_stdio_whitespace_command() {
        let mut cfg = valid_stdio_config();
        cfg.command = Some("   ".to_string());
        let err = cfg.validate().unwrap_err();
        assert!(err.contains("command"), "空白 command 应报错: {err}");
    }

    #[test]
    fn tc_mcp_valid_010_sse_missing_url() {
        let mut cfg = valid_sse_config();
        cfg.url = None;
        let err = cfg.validate().unwrap_err();
        assert!(err.contains("url"), "缺少 url 应报错: {err}");
    }

    #[test]
    fn tc_mcp_valid_011_sse_empty_url() {
        let mut cfg = valid_sse_config();
        cfg.url = Some("".to_string());
        let err = cfg.validate().unwrap_err();
        assert!(err.contains("url"), "空 url 应报错: {err}");
    }

    #[test]
    fn tc_mcp_valid_012_sse_whitespace_url() {
        let mut cfg = valid_sse_config();
        cfg.url = Some("   ".to_string());
        let err = cfg.validate().unwrap_err();
        assert!(err.contains("url"), "空白 url 应报错: {err}");
    }

    #[test]
    fn tc_mcp_valid_013_name_and_id_empty() {
        let mut cfg = valid_stdio_config();
        cfg.name = "".to_string();
        cfg.id = "".to_string();
        let err = cfg.validate().unwrap_err();
        assert!(err.contains("名称"), "名称检查应在 ID 检查之前: {err}");
    }

    #[test]
    fn tc_mcp_valid_014_stdio_ignores_url() {
        let mut cfg = valid_stdio_config();
        cfg.url = Some("http://example.com".to_string());
        assert!(cfg.validate().is_ok(), "stdio 不应检查 url");
    }

    #[test]
    fn tc_mcp_valid_015_sse_ignores_command() {
        let mut cfg = valid_sse_config();
        cfg.command = Some("npx".to_string());
        assert!(cfg.validate().is_ok(), "SSE 不应检查 command");
    }

    #[test]
    fn tc_mcp_valid_016_all_fields_whitespace() {
        let mut cfg = valid_stdio_config();
        cfg.name = " \t\n ".to_string();
        cfg.id = " \t\n ".to_string();
        cfg.command = Some(" \t\n ".to_string());
        let err = cfg.validate().unwrap_err();
        assert!(err.contains("名称"), "空白名称应报错: {err}");
    }
}
