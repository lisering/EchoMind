//! 领域画像自动分类模块（REQ-VEC-013）。
//!
//! 使用嵌入质心分类（Embedding Centroid Classification）将文档自动分类到 16 个
//! 预定义领域。分类完全基于本地嵌入模型（fastembed/ONNX），零 LLM 成本、
//! 完全离线可用。
//!
//! ## 原理
//!
//! 1. **质心预计算**：每个领域定义一组代表性关键词短语，嵌入后取均值作为该领域的质心向量。
//! 2. **文档分类**：取文档前 N 个 chunk 的嵌入均值，与各领域质心做余弦相似度比较，
//!    取最高者作为分类结果。
//!
//! ## 16 领域定义
//!
//! | 标识 | 中文名 | 代表性关键词 |
//! |---|---|---|
//! | technology | 科技/软件工程 | programming, software, API, algorithm, code |
//! | legal | 法律 | law, contract, court, legal, attorney, statute |
//! | medical | 医疗/健康 | medical, health, patient, diagnosis, treatment |
//! | finance | 金融/会计 | finance, accounting, investment, revenue, audit |
//! | science | 科学研究 | research, experiment, hypothesis, theory, data |
//! | education | 教育/学术 | education, learning, student, curriculum, academic |
//! | business | 商业/管理 | business, management, strategy, market, corporate |
//! | engineering | 工程/技术 | engineering, design, technical, system, mechanical |
//! | literature | 文学/人文 | literature, novel, poetry, culture, history, philosophy |
//! | government | 政府/公共政策 | government, policy, public, administration, political |
//! | marketing | 市场营销 | marketing, advertising, brand, campaign, customer |
//! | hr | 人力资源 | human resources, recruitment, employee, organization, training |
//! | design | 设计/创意 | design, creative, UI, UX, graphic, visual |
//! | data | 数据/分析 | data, analytics, statistics, machine learning, dataset |
//! | security | 安全/合规 | security, compliance, privacy, encryption, vulnerability |
//! | general | 通用/其他 | general, document, notes, reference, information |

use crate::{DomainClassifier, Embedder};

/// 16 个预定义领域标识。
///
/// 顺序固定，索引可用于质心向量数组的位置映射。
pub const DOMAINS: &[&str] = &[
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

/// 每个领域的代表性关键词短语列表。
///
/// 这些关键词经过精心选择，覆盖该领域的核心语义空间。
/// 嵌入后取均值作为领域质心向量。
///
/// 关键词同时包含中英文，以适应中英文混合的知识库文档。
const DOMAIN_KEYWORDS: &[(&str, &[&str])] = &[
    (
        "technology",
        &[
            "programming software development API algorithm code framework",
            "编程 软件开发 接口 算法 代码 框架 架构",
            "computer science machine learning artificial intelligence data structure",
            "软件工程 程序设计 版本控制 测试 部署 运维",
        ],
    ),
    (
        "legal",
        &[
            "law legal contract court attorney statute regulation compliance",
            "法律 合同 法院 律师 法规 合规 诉讼 判决",
            "legal agreement liability intellectual property copyright patent",
            "条款 权利 义务 违约 赔偿 仲裁 司法",
        ],
    ),
    (
        "medical",
        &[
            "medical health patient diagnosis treatment disease clinical symptom",
            "医疗 健康 患者 诊断 治疗 疾病 临床 症状",
            "medicine doctor hospital pharmacy prescription therapy surgery",
            "医学 药物 检查 手术 康复 预防 病理",
        ],
    ),
    (
        "finance",
        &[
            "finance accounting investment revenue profit financial audit tax",
            "金融 会计 投资 收入 利润 财务 审计 税务",
            "budget cost expense asset liability cash flow equity",
            "预算 成本 资产 负债 现金流 股权 报表",
        ],
    ),
    (
        "science",
        &[
            "research experiment hypothesis theory scientific data analysis method",
            "研究 实验 假设 理论 科学 数据 分析 方法",
            "physics chemistry biology mathematics observation measurement result",
            "物理 化学 生物 数学 观测 测量 结论 论文",
        ],
    ),
    (
        "education",
        &[
            "education learning student curriculum teaching academic school university",
            "教育 学习 学生 课程 教学 学术 学校 大学",
            "knowledge skill training course lesson textbook examination grade",
            "知识 技能 培训 教材 考试 成绩 课堂",
        ],
    ),
    (
        "business",
        &[
            "business management strategy market company corporate operations planning",
            "商业 管理 战略 市场 公司 企业 运营 规划",
            "enterprise industry trade commerce profit growth stakeholder",
            "企业 产业 贸易 商务 利润 增长 合作",
        ],
    ),
    (
        "engineering",
        &[
            "engineering design technical system mechanical electrical civil structural",
            "工程 设计 技术 系统 机械 电气 土木 结构",
            "specification component material manufacturing assembly calibration",
            "规格 组件 材料 制造 装配 校准 工艺",
        ],
    ),
    (
        "literature",
        &[
            "literature novel poetry culture history philosophy art language writing",
            "文学 小说 诗歌 文化 历史 哲学 艺术 语言",
            "author narrative character story essay criticism aesthetic",
            "作者 叙事 人物 故事 散文 评论 美学",
        ],
    ),
    (
        "government",
        &[
            "government policy public administration political regulation civic citizen",
            "政府 政策 公共 行政 政治 法规 公民",
            "legislation parliament committee authority governance reform",
            "立法 议会 委员会 权力 治理 改革",
        ],
    ),
    (
        "marketing",
        &[
            "marketing advertising brand campaign customer sales promotion strategy",
            "营销 广告 品牌 活动 客户 销售 推广",
            "market research target audience conversion engagement retention",
            "市场调研 目标受众 转化 互动 留存 渠道",
        ],
    ),
    (
        "hr",
        &[
            "human resources recruitment employee organization training workplace performance",
            "人力资源 招聘 员工 组织 培训 职场 绩效",
            "salary benefits compensation career development team culture",
            "薪酬 福利 职业发展 团队 文化 考核",
        ],
    ),
    (
        "design",
        &[
            "design creative visual graphic UI UX aesthetic layout typography color",
            "设计 创意 视觉 图形 界面 布局 排版 色彩",
            "prototype wireframe user experience interaction brand identity",
            "原型 线框 用户体验 交互 品牌 视觉识别",
        ],
    ),
    (
        "data",
        &[
            "data analytics statistics machine learning model dataset visualization dashboard",
            "数据 分析 统计 机器学习 模型 数据集 可视化",
            "pipeline processing mining prediction clustering regression classification",
            "管道 处理 挖掘 预测 聚类 回归 分类",
        ],
    ),
    (
        "security",
        &[
            "security compliance privacy encryption vulnerability threat protection access",
            "安全 合规 隐私 加密 漏洞 威胁 防护 访问",
            "authentication authorization firewall intrusion detection incident response",
            "认证 授权 防火墙 入侵检测 事件响应 审计",
        ],
    ),
    (
        "general",
        &[
            "general document notes reference information guide manual instructions",
            "通用 文档 笔记 参考 信息 指南 手册 说明",
            "summary overview introduction background context miscellaneous",
            "摘要 概述 介绍 背景 上下文 其他",
        ],
    ),
];

/// 分类时取样的最大 chunk 数。
///
/// 取前 5 个 chunk（通常是文档开头的摘要/概述部分）足以
/// 代表文档的领域特征，同时控制嵌入计算开销。
const MAX_SAMPLE_CHUNKS: usize = 5;

/// 嵌入质心领域分类器（REQ-VEC-013）。
///
/// 使用本地嵌入模型计算 16 个领域的质心向量，
/// 文档分类时取前 N 个 chunk 的嵌入均值与各质心做余弦相似度比较。
///
/// # 零 LLM 成本
///
/// 分类完全基于本地 ONNX 嵌入推理，不调用 LLM，零网络请求、零 API 费用。
///
/// # 线程安全
///
/// `Embedder` 端口要求 `Send + Sync`，`EmbeddingDomainClassifier` 本身也是
/// `Send + Sync`（内部无可变状态），可安全跨线程共享。
pub struct EmbeddingDomainClassifier<E: Embedder> {
    /// 嵌入模型（用于文档样本嵌入）
    embedder: E,
    /// 16 个领域的质心向量 (domain_name, centroid_vector)
    centroids: Vec<(String, Vec<f32>)>,
}

impl<E: Embedder> EmbeddingDomainClassifier<E> {
    /// 创建领域分类器，预计算所有领域的质心向量。
    ///
    /// 初始化时对每个领域的代表性关键词短语批量嵌入，取均值作为质心。
    /// 此操作在首次使用时触发嵌入模型加载（可能需要下载 ONNX 模型）。
    ///
    /// # 参数
    /// - `embedder`: 本地嵌入模型（如 `LocalEmbedder`）
    ///
    /// # 错误
    /// 嵌入模型初始化失败或嵌入计算失败时返回 Err。
    pub async fn new(embedder: E) -> anyhow::Result<Self> {
        let mut centroids = Vec::with_capacity(DOMAINS.len());

        for (domain, keywords) in DOMAIN_KEYWORDS {
            let texts: Vec<String> = keywords.iter().map(|s| s.to_string()).collect();
            let embeddings = embedder.embed_batch(&texts).await?;
            let centroid = average_vectors(&embeddings);
            centroids.push((domain.to_string(), centroid));
        }

        Ok(Self {
            embedder,
            centroids,
        })
    }

    /// 获取已计算的领域质心列表（用于测试和调试）。
    pub fn centroids(&self) -> &[(String, Vec<f32>)] {
        &self.centroids
    }

    /// 使用自定义质心创建分类器（仅供测试用）。
    ///
    /// 允许跳过关键词嵌入步骤，直接注入预计算的质心向量。
    #[cfg(test)]
    pub(crate) fn new_with_centroids(embedder: E, centroids: Vec<(String, Vec<f32>)>) -> Self {
        Self {
            embedder,
            centroids,
        }
    }
}

impl<E: Embedder> DomainClassifier for EmbeddingDomainClassifier<E> {
    /// 对文档内容进行领域分类（REQ-VEC-013 AC-1~AC-5）。
    ///
    /// 取文档前 `MAX_SAMPLE_CHUNKS` 个 chunk 的嵌入均值，
    /// 与 16 个领域质心做余弦相似度比较，取最高者。
    ///
    /// # 降级策略
    ///
    /// - 空 chunks → 返回 `"general"`（AC-5）
    /// - 嵌入失败 → 返回 `"general"`（AC-5 优雅降级）
    ///
    /// # 参数
    /// - `chunks`: 文档分块文本列表
    ///
    /// # 返回
    /// 16 个预定义领域之一的字符串标识。
    async fn classify(&self, chunks: &[String]) -> anyhow::Result<String> {
        // AC-5: 空 chunks 降级为 general
        if chunks.is_empty() {
            return Ok("general".to_string());
        }

        // 取前 N 个 chunk 作为样本
        let sample: Vec<String> = chunks.iter().take(MAX_SAMPLE_CHUNKS).cloned().collect();

        // AC-5: 嵌入失败时优雅降级为 general
        let embeddings = match self.embedder.embed_batch(&sample).await {
            Ok(emb) => emb,
            Err(e) => {
                eprintln!("领域分类嵌入失败，降级为 general: {e:#}");
                return Ok("general".to_string());
            }
        };

        // 计算文档样本的质心
        let doc_centroid = average_vectors(&embeddings);

        // 与各领域质心做余弦相似度比较，取最高者
        let mut best_domain = "general";
        let mut best_score = -1.0f32;

        for (domain, centroid) in &self.centroids {
            let score = cosine_similarity(&doc_centroid, centroid);
            if score > best_score {
                best_score = score;
                best_domain = domain;
            }
        }

        Ok(best_domain.to_string())
    }
}

/// 计算多个向量的均值（质心）。
///
/// 空输入返回空向量；各向量长度不一致时以第一个向量的长度为准（截断或填充零）。
fn average_vectors(vectors: &[Vec<f32>]) -> Vec<f32> {
    if vectors.is_empty() {
        return Vec::new();
    }
    let dim = vectors[0].len();
    let mut sum = vec![0.0f32; dim];
    for vec in vectors {
        for (i, &val) in vec.iter().take(dim).enumerate() {
            sum[i] += val;
        }
    }
    let count = vectors.len() as f32;
    for val in &mut sum {
        *val /= count;
    }
    sum
}

/// 计算两个向量的余弦相似度。
///
/// 返回值范围 [-1.0, 1.0]。任一向量为零向量时返回 0.0。
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let len = a.len().min(b.len());
    if len == 0 {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for i in 0..len {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 {
        return 0.0;
    }
    dot / denom
}

/// 验证领域标识是否为 16 个预定义领域之一（REQ-VEC-013 AC-2）。
///
/// 用于在写入 Document.domain 前校验分类结果的合法性。
pub fn is_valid_domain(domain: &str) -> bool {
    DOMAINS.contains(&domain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_average_vectors() {
        let v1 = vec![1.0, 2.0, 3.0];
        let v2 = vec![3.0, 4.0, 5.0];
        let avg = average_vectors(&[v1, v2]);
        assert_eq!(avg, vec![2.0, 3.0, 4.0]);
    }

    #[test]
    fn test_average_vectors_empty() {
        let avg = average_vectors(&[]);
        assert!(avg.is_empty());
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let v = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&v, &v);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn test_is_valid_domain() {
        assert!(is_valid_domain("technology"));
        assert!(is_valid_domain("general"));
        assert!(!is_valid_domain("invalid_domain"));
        assert!(!is_valid_domain(""));
    }

    #[test]
    fn test_domain_count() {
        assert_eq!(DOMAINS.len(), 16);
        assert_eq!(DOMAIN_KEYWORDS.len(), 16);
    }
}
