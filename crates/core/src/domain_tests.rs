#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! QA 红灯测试：TC-DOMAIN-001~007 领域画像自动分类（REQ-VEC-013）。
//!
//! 测试策略：
//! - Mock Embedder：根据文本内容返回可控向量（同领域高相似、异领域低相似）
//! - 验证 16 领域定义完整性
//! - 验证分类结果的合法性
//! - 验证降级策略（空 chunks、嵌入失败）

use std::sync::Arc;

use anyhow::Result;

use crate::domain::{DOMAINS, EmbeddingDomainClassifier, is_valid_domain};
use crate::{DomainClassifier, Embedder};

// ================== Mock Embedder ==================

/// 语义感知 Mock Embedder：根据文本中的关键词返回对应的语义向量。
///
/// 设计：每个领域对应一个 16 维向量，在该领域的维度上为 1.0，其他维度为 0.0。
/// 这样同领域文本的余弦相似度为 1.0，异领域为 0.0，实现精确的分类测试。
struct SemanticMockEmbedder {
    /// 向量维度（等于领域数量）
    dim: usize,
}

impl SemanticMockEmbedder {
    fn new() -> Self {
        Self { dim: DOMAINS.len() }
    }

    /// 根据文本内容判断所属领域，返回对应的 one-hot 向量。
    fn text_to_vector(&self, text: &str) -> Vec<f32> {
        let mut vec = vec![0.0f32; self.dim];

        // 检测文本中的领域关键词
        let domain_idx = if text.contains("编程")
            || text.contains("programming")
            || text.contains("软件")
        {
            0 // technology
        } else if text.contains("法律") || text.contains("law") || text.contains("合同") {
            1 // legal
        } else if text.contains("医疗") || text.contains("medical") || text.contains("患者") {
            2 // medical
        } else if text.contains("金融") || text.contains("finance") || text.contains("财务") {
            3 // finance
        } else if text.contains("研究") || text.contains("research") || text.contains("实验") {
            4 // science
        } else if text.contains("教育") || text.contains("education") || text.contains("学生") {
            5 // education
        } else if text.contains("商业") || text.contains("business") || text.contains("管理") {
            6 // business
        } else if text.contains("工程") || text.contains("engineering") || text.contains("机械")
        {
            7 // engineering
        } else if text.contains("文学") || text.contains("literature") || text.contains("诗歌")
        {
            8 // literature
        } else if text.contains("政府") || text.contains("government") || text.contains("政策")
        {
            9 // government
        } else if text.contains("营销") || text.contains("marketing") || text.contains("品牌") {
            10 // marketing
        } else if text.contains("人力资源") || text.contains("recruitment") || text.contains("员工")
        {
            11 // hr
        } else if text.contains("设计") || text.contains("design") || text.contains("创意") {
            12 // design
        } else if text.contains("数据分析") || text.contains("analytics") || text.contains("统计")
        {
            13 // data
        } else if text.contains("安全") || text.contains("security") || text.contains("加密") {
            14 // security
        } else {
            15 // general
        };

        vec[domain_idx] = 1.0;
        vec
    }
}

impl Embedder for SemanticMockEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        Ok(self.text_to_vector(text))
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|t| self.text_to_vector(t)).collect())
    }
}

/// 总是失败的 Mock Embedder（测试降级策略）。
struct FailingEmbedder;

impl Embedder for FailingEmbedder {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>> {
        Err(anyhow::anyhow!("嵌入模型不可用"))
    }

    async fn embed_batch(&self, _texts: &[String]) -> Result<Vec<Vec<f32>>> {
        Err(anyhow::anyhow!("嵌入模型不可用"))
    }
}

// ================== TC-DOMAIN-001: 文档导入后自动分类 ==================

/// TC-DOMAIN-001：分类器正确分类科技类文档（REQ-VEC-013 AC-1）。
///
/// 验证 EmbeddingDomainClassifier 能将包含编程关键词的文档
/// 分类为 "technology" 领域。
#[tokio::test]
async fn tc_domain_001_classify_technology() {
    let embedder = SemanticMockEmbedder::new();
    let classifier = EmbeddingDomainClassifier::new(embedder)
        .await
        .expect("分类器初始化失败");

    let chunks = vec![
        "编程 软件开发 接口 算法 代码".to_string(),
        "软件工程 程序设计 版本控制".to_string(),
    ];

    let domain = classifier.classify(&chunks).await.expect("分类失败");

    assert_eq!(
        domain, "technology",
        "包含编程关键词的文档应分类为 technology"
    );
}

/// TC-DOMAIN-001b：分类器正确分类法律类文档。
#[tokio::test]
async fn tc_domain_001b_classify_legal() {
    let embedder = SemanticMockEmbedder::new();
    let classifier = EmbeddingDomainClassifier::new(embedder)
        .await
        .expect("分类器初始化失败");

    let chunks = vec!["法律 合同 法院 律师 法规".to_string()];

    let domain = classifier.classify(&chunks).await.expect("分类失败");

    assert_eq!(domain, "legal", "法律类文档应分类为 legal");
}

/// TC-DOMAIN-001c：分类器正确分类医疗类文档。
#[tokio::test]
async fn tc_domain_001c_classify_medical() {
    let embedder = SemanticMockEmbedder::new();
    let classifier = EmbeddingDomainClassifier::new(embedder)
        .await
        .expect("分类器初始化失败");

    let chunks = vec!["医疗 健康 患者 诊断 治疗".to_string()];

    let domain = classifier.classify(&chunks).await.expect("分类失败");

    assert_eq!(domain, "medical", "医疗类文档应分类为 medical");
}

// ================== TC-DOMAIN-002: 分类结果合法性 ==================

/// TC-DOMAIN-002：分类结果始终为 16 个预定义领域之一（REQ-VEC-013 AC-2）。
#[tokio::test]
async fn tc_domain_002_result_is_valid_domain() {
    let embedder = SemanticMockEmbedder::new();
    let classifier = EmbeddingDomainClassifier::new(embedder)
        .await
        .expect("分类器初始化失败");

    // 测试多种文档内容
    let test_cases = vec![
        vec!["编程 代码".to_string()],
        vec!["法律 合同".to_string()],
        vec!["医疗 患者".to_string()],
        vec!["未知领域的内容".to_string()],
    ];

    for chunks in test_cases {
        let domain = classifier.classify(&chunks).await.expect("分类失败");
        assert!(
            is_valid_domain(&domain),
            "分类结果 '{domain}' 不是有效的领域标识"
        );
    }
}

// ================== TC-DOMAIN-003: 零 LLM 成本 ==================

/// TC-DOMAIN-003：分类不调用 LLM（基于纯嵌入计算）（REQ-VEC-013 AC-3）。
///
/// 通过使用不涉及任何 LLM 的 Mock Embedder 验证分类流程
/// 完全不依赖 LLM 调用。EmbeddingDomainClassifier 仅使用 Embedder 端口，
/// 不依赖 LLMProvider 端口。
#[tokio::test]
async fn tc_domain_003_no_llm_involved() {
    let embedder = SemanticMockEmbedder::new();
    let classifier = EmbeddingDomainClassifier::new(embedder)
        .await
        .expect("分类器初始化失败");

    // 分类器仅持有 Embedder，不持有 LLMProvider
    // 分类过程仅调用 embed/embed_batch，不调用 chat_stream
    let domain = classifier
        .classify(&["测试文档".to_string()])
        .await
        .expect("分类失败");

    assert!(!domain.is_empty(), "分类结果不应为空");
}

// ================== TC-DOMAIN-004: 后台异步执行 ==================

/// TC-DOMAIN-004：分类在后台异步执行（REQ-VEC-013 AC-4）。
///
/// 验证 classify 方法是 async 的，可以在 tokio 运行时中并发执行。
#[tokio::test]
async fn tc_domain_004_async_classification() {
    let embedder = SemanticMockEmbedder::new();
    let classifier = Arc::new(
        EmbeddingDomainClassifier::new(embedder)
            .await
            .expect("分类器初始化失败"),
    );

    // 并发分类多个文档
    let chunks_list = vec![
        vec!["编程 代码".to_string()],
        vec!["法律 合同".to_string()],
        vec!["医疗 患者".to_string()],
    ];

    let results = futures::future::join_all(chunks_list.into_iter().map(|chunks| {
        let classifier = Arc::clone(&classifier);
        async move { classifier.classify(&chunks).await }
    }))
    .await;

    for result in &results {
        assert!(result.is_ok(), "并发分类不应失败");
        assert!(is_valid_domain(result.as_ref().unwrap()), "分类结果应有效");
    }
}

// ================== TC-DOMAIN-005: 降级策略 ==================

/// TC-DOMAIN-005：嵌入失败时降级为 general（REQ-VEC-013 AC-5）。
#[tokio::test]
async fn tc_domain_005_embedding_failure_degrades_to_general() {
    let embedder = FailingEmbedder;
    let classifier = EmbeddingDomainClassifier::new_with_centroids(
        embedder,
        vec![("general".to_string(), vec![1.0; 16])],
    );

    let chunks = vec!["测试文档".to_string()];
    let domain = classifier
        .classify(&chunks)
        .await
        .expect("分类不应返回 Err");

    assert_eq!(domain, "general", "嵌入失败时应降级为 general");
}

/// TC-DOMAIN-005b：空 chunks 降级为 general。
#[tokio::test]
async fn tc_domain_005b_empty_chunks_degrades_to_general() {
    let embedder = SemanticMockEmbedder::new();
    let classifier = EmbeddingDomainClassifier::new(embedder)
        .await
        .expect("分类器初始化失败");

    let domain = classifier.classify(&[]).await.expect("分类不应返回 Err");

    assert_eq!(domain, "general", "空 chunks 应降级为 general");
}

// ================== TC-DOMAIN-006: 16 领域完整性 ==================

/// TC-DOMAIN-006：16 个预定义领域全部有效（REQ-VEC-013 AC-2 补充）。
#[test]
fn tc_domain_006_all_16_domains_defined() {
    assert_eq!(DOMAINS.len(), 16, "必须定义 16 个领域");

    // 所有领域标识都应通过 is_valid_domain 校验
    for domain in DOMAINS {
        assert!(
            is_valid_domain(domain),
            "领域标识 '{domain}' 应通过合法性校验"
        );
    }

    // 验证预期领域全部存在
    let expected = [
        "technology",
        "legal",
        "medical",
        "finance",
        "science",
        "education",
        "business",
        "engineering",
        "literature",
        "government",
        "marketing",
        "hr",
        "design",
        "data",
        "security",
        "general",
    ];
    for exp in expected {
        assert!(DOMAINS.contains(&exp), "缺少预期领域: {exp}");
    }
}

// ================== TC-DOMAIN-007: 重新分类 ==================

/// TC-DOMAIN-007：重新分类应返回不同的领域（REQ-VEC-013 AC-7 旁证）。
///
/// 验证同一分类器对不同内容产生不同分类结果。
#[tokio::test]
async fn tc_domain_007_reclassify_different_content() {
    let embedder = SemanticMockEmbedder::new();
    let classifier = EmbeddingDomainClassifier::new(embedder)
        .await
        .expect("分类器初始化失败");

    // 第一次分类
    let tech_chunks = vec!["编程 代码 软件".to_string()];
    let domain1 = classifier.classify(&tech_chunks).await.expect("分类失败");

    // 第二次分类（不同内容）
    let legal_chunks = vec!["法律 合同 法院".to_string()];
    let domain2 = classifier.classify(&legal_chunks).await.expect("分类失败");

    assert_ne!(domain1, domain2, "不同内容的文档应分类到不同领域");
    assert_eq!(domain1, "technology");
    assert_eq!(domain2, "legal");
}

// ================== 辅助函数 ==================
