//! 文档一致性审计引擎（REQ-AUDIT-001~005）。
//!
//! 与标准 RAG（query → top-k 检索）不同，审计模式通过 `Storage.list_chunks()`
//! 全量扫描文档所有分块，执行 Decompose-Then-Verify 三阶段流水线：
//! 1. **Decompose**——逐 chunk 提取原子声明（Claim）
//! 2. **Verify**——embedding 预筛 + LLM 精判跨 chunk 矛盾
//! 3. **Report**——聚合为结构化审计报告
//!
//! 调研依据：FIND 基准(arXiv:2512.18601)、Decompose-Then-Verify 范式、
//! Google PAT(arXiv:2606.28277)、SIFiD(arXiv:2403.07557)、SummaC(arXiv:2111.09525)。
//! 零新依赖：复用 `Embedder` + `Storage` + `LLMProvider` 端口。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use futures::StreamExt;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};

use crate::{Embedder, LLMProvider, Storage};

/// 声明类型分类（REQ-AUDIT-002）。
/// 标识原子声明的语义类别，用于分组比对与报告分类。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimType {
    /// 数值参数（如"温度为 25°C"、"误差小于 5%"）
    NumericParameter,
    /// 定义声明（如"X 定义为 Y 的函数"）
    Definition,
    /// 因果声明（如"A 导致 B"）
    Causal,
    /// 结论声明（如"结果表明算法收敛"）
    Conclusion,
    /// 其他可验证断言
    Other,
}

/// 原子声明（REQ-AUDIT-002）。
/// 从文档 chunk 中提取的可验证断言，用于跨 chunk 矛盾检测。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claim {
    /// 声明唯一标识
    pub claim_id: String,
    /// 来源 chunk ID
    pub chunk_id: String,
    /// 来源 chunk 在文档中的序号
    pub sequence: usize,
    /// 声明文本
    pub text: String,
    /// 声明类型
    pub claim_type: ClaimType,
    /// 涉及的实体（关键词，用于分组比对）
    pub entities: Vec<String>,
}

/// 矛盾判定结果（REQ-AUDIT-003）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// 矛盾——两声明互相冲突
    Contradiction,
    /// 一致——两声明不矛盾
    Consistent,
    /// 无法判定——信息不足或语义模糊
    Unverifiable,
}

/// 严重等级（REQ-AUDIT-004）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// 高——核心参数/结论矛盾
    High,
    /// 中——次要参数矛盾
    Medium,
    /// 低——表述差异但语义可调和
    Low,
}

/// 矛盾对（REQ-AUDIT-003）。
/// 两个来自不同 chunk 的声明经 LLM 判定后的结构化记录。
/// 包含所有通过 embedding 预筛的声明对（含一致和无法判定），
/// verdict 字段标识具体判定结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContradictionPair {
    /// 矛盾对唯一标识
    pub pair_id: String,
    /// 声明 A（含来源信息）
    pub claim_a: Claim,
    /// 声明 B（含来源信息）
    pub claim_b: Claim,
    /// LLM 判定结果
    pub verdict: Verdict,
    /// LLM 给出的解释说明
    pub explanation: String,
    /// 严重等级
    pub severity: Severity,
}

/// 审计报告（REQ-AUDIT-004）。
/// 聚合全部审计发现的结构化报告，可转换为 Markdown 渲染。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditReport {
    /// 审计文档名
    pub doc_name: String,
    /// 文档 chunk 总数
    pub total_chunks: usize,
    /// 提取的声明总数
    pub total_claims: usize,
    /// 发现的矛盾列表（含所有通过预筛的声明对，verdict 字段区分具体判定）
    pub contradictions: Vec<ContradictionPair>,
    /// 审计耗时（毫秒）
    pub elapsed_ms: u64,
}

/// 审计输出（REQ-AUDIT-001）。
pub enum AuditOutcome {
    /// 审计完成——含完整报告
    Completed {
        /// 结构化审计报告
        report: AuditReport,
    },
    /// 文档无 chunk（空文档或不存在）
    NoChunks,
    /// 审计被取消——含部分报告（已提取的声明和已发现的矛盾保留）
    Cancelled {
        /// 部分审计报告
        partial_report: AuditReport,
    },
}

/// 审计取消信号（零依赖：不引入 tokio_util，core 层使用 AtomicBool）。
/// IPC 层（commands.rs）将 CancellationToken 转换为此标志。
pub type AuditCancelFlag = Arc<AtomicBool>;

/// SIFiD 预筛 cosine 相似度阈值（REQ-AUDIT-003-AC-2）。
/// 相似度高于此阈值的声明对（讨论同一主题）进入 LLM 精判阶段，
/// 低于此阈值的声明对（语义无关）被过滤，将 O(n²) 降至 O(k)。
const PREFILTER_THRESHOLD: f32 = 0.65;

/// 审计引擎（REQ-AUDIT-001）。
///
/// 六边形架构：仅依赖 `Embedder` + `Storage` + `LLMProvider` 端口。
/// 与 `ChatEngine` 平级，走全量扫描路径（`list_chunks`）而非向量检索路径（`vector_search`）。
pub struct AuditEngine<E: Embedder, S: Storage, L: LLMProvider> {
    embedder: E,
    storage: S,
    llm: L,
}

impl<E: Embedder, S: Storage, L: LLMProvider> AuditEngine<E, S, L> {
    /// 构造审计引擎。
    pub fn new(embedder: E, storage: S, llm: L) -> Self {
        Self {
            embedder,
            storage,
            llm,
        }
    }

    /// 执行全文审计（REQ-AUDIT-001）。
    ///
    /// 通过 `Storage.list_chunks(doc_id)` 获取文档全部 chunk（全量扫描），
    /// 执行 Decompose → Verify → Report 三阶段流水线。
    /// 取消信号在每个阶段之间检查，触发后保留已完成的 partial 结果。
    ///
    /// # 参数
    /// - `doc_id`: 待审计文档 ID
    /// - `doc_name`: 文档显示名（用于报告标题）
    /// - `cancel`: 取消信号（AtomicBool，true 表示请求取消）
    ///
    /// # 返回
    /// - `AuditOutcome::Completed`: 审计正常完成
    /// - `AuditOutcome::NoChunks`: 文档无 chunk（空或不存在）
    /// - `AuditOutcome::Cancelled`: 审计被取消，含部分报告
    pub async fn audit(
        &self,
        doc_id: &str,
        doc_name: &str,
        cancel: AuditCancelFlag,
    ) -> anyhow::Result<AuditOutcome> {
        let start = Instant::now();

        // ---- Phase 0: 全量扫描（REQ-AUDIT-001-AC-1）----
        // 走 list_chunks 路径，不走 vector_search top-k 检索
        let chunks = self.storage.list_chunks(doc_id).await?;
        if chunks.is_empty() {
            return Ok(AuditOutcome::NoChunks);
        }

        // 取消检查：list_chunks 后、Decompose 前
        if is_cancelled(&cancel) {
            return Ok(AuditOutcome::Cancelled {
                partial_report: self.build_report(
                    doc_name,
                    chunks.len(),
                    0,
                    vec![],
                    start.elapsed(),
                ),
            });
        }

        // ---- Phase 1: Decompose——原子声明提取（REQ-AUDIT-002）----
        let claims = self.decompose(&chunks).await.unwrap_or_default();

        // 取消检查：Decompose 后、Verify 前
        if is_cancelled(&cancel) {
            return Ok(AuditOutcome::Cancelled {
                partial_report: self.build_report(
                    doc_name,
                    chunks.len(),
                    claims.len(),
                    vec![],
                    start.elapsed(),
                ),
            });
        }

        // ---- Phase 2: Verify——跨声明矛盾检测（REQ-AUDIT-003）----
        let contradictions = self.verify(&claims, &cancel).await.unwrap_or_default();

        // ---- Phase 3: Report——聚合审计报告（REQ-AUDIT-004）----
        // 取消检查：Verify 后、Report 前
        if is_cancelled(&cancel) {
            return Ok(AuditOutcome::Cancelled {
                partial_report: self.build_report(
                    doc_name,
                    chunks.len(),
                    claims.len(),
                    contradictions,
                    start.elapsed(),
                ),
            });
        }

        Ok(AuditOutcome::Completed {
            report: self.build_report(
                doc_name,
                chunks.len(),
                claims.len(),
                contradictions,
                start.elapsed(),
            ),
        })
    }

    /// Decompose 阶段：从全部 chunk 中提取原子声明（REQ-AUDIT-002）。
    ///
    /// 批量模式：将所有 chunk 拼入单个 prompt，一次 LLM 调用提取全部声明。
    /// LLM 返回 JSON 数组，每元素含 {claim_id, chunk_id, sequence, text, claim_type, entities}。
    /// JSON 解析失败时优雅降级，返回空 Vec（REQ-AUDIT-002-AC-4）。
    async fn decompose(&self, chunks: &[echomind_models::Chunk]) -> anyhow::Result<Vec<Claim>> {
        let system_prompt = build_decompose_prompt(chunks);
        let stream = self.llm.chat_stream(&system_prompt, &[], "").await?;
        let raw = collect_stream(stream).await?;

        // JSON 解析：失败时优雅降级（REQ-AUDIT-002-AC-4）
        let claims: Vec<Claim> = serde_json::from_str(&raw)
            .map_err(|e| {
                eprintln!("声明提取 JSON 解析失败，降级为空列表: {e}");
                e
            })
            .unwrap_or_default();

        Ok(claims)
    }

    /// Verify 阶段：embedding 预筛 + LLM 精判（REQ-AUDIT-003）。
    ///
    /// 1. 批量嵌入所有声明文本
    /// 2. 遍历声明对 (i, j)，仅保留来自不同 chunk 且 cosine > 阈值的对（SIFiD 预筛）
    /// 3. 对预筛通过的对调用 LLM 做矛盾精判
    /// 4. 所有精判结果（含 consistent/unverifiable）纳入 contradictions 列表
    async fn verify(
        &self,
        claims: &[Claim],
        cancel: &AuditCancelFlag,
    ) -> anyhow::Result<Vec<ContradictionPair>> {
        if claims.len() < 2 {
            return Ok(vec![]);
        }

        // 批量嵌入声明文本
        let texts: Vec<String> = claims.iter().map(|c| c.text.clone()).collect();
        let embeddings = self.embedder.embed_batch(&texts).await?;

        let mut pairs = Vec::new();

        for i in 0..claims.len() {
            for j in (i + 1)..claims.len() {
                // 取消检查：在每对比对前检查
                if is_cancelled(cancel) {
                    break;
                }

                let claim_a = &claims[i];
                let claim_b = &claims[j];

                // SIFiD 预筛：仅比对来自不同 chunk 的声明（REQ-AUDIT-003-AC-2）
                if claim_a.chunk_id == claim_b.chunk_id {
                    continue;
                }

                // cosine 相似度预筛：低于阈值的声明对（语义无关）直接跳过
                let sim = cosine_similarity(&embeddings[i], &embeddings[j]);
                if sim < PREFILTER_THRESHOLD {
                    continue;
                }

                // LLM 精判：将预筛通过的声明对交给 LLM 判定
                let system_prompt = build_verify_prompt(claim_a, claim_b);
                let stream = self.llm.chat_stream(&system_prompt, &[], "").await?;
                let raw = collect_stream(stream).await?;

                // JSON 解析：失败时跳过此对（优雅降级）
                let Ok(verdict_resp) = serde_json::from_str::<VerdictResponse>(&raw) else {
                    eprintln!("矛盾判定 JSON 解析失败，跳过此对: {raw}");
                    continue;
                };

                pairs.push(ContradictionPair {
                    pair_id: verdict_resp.pair_id,
                    claim_a: claim_a.clone(),
                    claim_b: claim_b.clone(),
                    verdict: verdict_resp.verdict,
                    explanation: verdict_resp.explanation,
                    severity: verdict_resp.severity,
                });
            }
        }

        Ok(pairs)
    }

    /// 构建审计报告（REQ-AUDIT-004）。
    fn build_report(
        &self,
        doc_name: &str,
        total_chunks: usize,
        total_claims: usize,
        contradictions: Vec<ContradictionPair>,
        elapsed: std::time::Duration,
    ) -> AuditReport {
        AuditReport {
            doc_name: doc_name.to_string(),
            total_chunks,
            total_claims,
            contradictions,
            elapsed_ms: elapsed.as_millis() as u64,
        }
    }
}

// ================== 辅助函数 ==================

/// 检查取消标志是否已触发。
fn is_cancelled(cancel: &AuditCancelFlag) -> bool {
    cancel.load(Ordering::SeqCst)
}

/// 将 LLM token 流收集为完整字符串（用于 JSON 解析）。
async fn collect_stream(
    mut stream: BoxStream<'static, anyhow::Result<String>>,
) -> anyhow::Result<String> {
    let mut result = String::new();
    while let Some(item) = stream.next().await {
        match item {
            Ok(token) => result.push_str(&token),
            Err(e) => return Err(e),
        }
    }
    Ok(result)
}

/// 计算 cosine 相似度（core 层独立实现，不依赖 infra 层）。
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 { 0.0 } else { dot / denom }
}

/// LLM 矛盾判定响应（Verify Phase 内部反序列化用）。
#[derive(Debug, Deserialize)]
struct VerdictResponse {
    pair_id: String,
    verdict: Verdict,
    explanation: String,
    severity: Severity,
}

/// 构建 Decompose 阶段系统提示词（REQ-AUDIT-002）。
///
/// 引导 LLM 逐 chunk 扫描并输出 JSON 数组格式的原子声明。
/// prompt 中包含 "提取" 关键词，使 Mock LLM 能区分 Decompose 阶段。
fn build_decompose_prompt(chunks: &[echomind_models::Chunk]) -> String {
    let mut prompt = String::from(
        "你是文档审计助手。请从以下文本片段中提取可验证的原子声明（Atomic Claims）。\n\
         声明是文档中做出的可被事实检验的断言，包括但不限于：\n\
         - 数值参数（如\"温度为 25°C\"、\"误差小于 5%\"）\n\
         - 定义声明（如\"X 定义为 Y 的函数\"）\n\
         - 因果声明（如\"A 导致 B\"）\n\
         - 结论声明（如\"结果表明算法收敛\"）\n\n\
         请逐片段提取声明，每个声明结构化为 JSON 对象，包含以下字段：\n\
         - claim_id: 声明唯一标识（如 \"c1\"）\n\
         - chunk_id: 来源片段的 chunk_id（使用下方提供的值）\n\
         - sequence: 来源片段的序号\n\
         - text: 声明文本\n\
         - claim_type: 声明类型（numeric_parameter / definition / causal / conclusion / other）\n\
         - entities: 涉及的实体关键词列表\n\n\
         输出格式：JSON 数组，仅输出 JSON，不要添加 Markdown 代码块标记。\n\n",
    );

    for (i, chunk) in chunks.iter().enumerate() {
        prompt.push_str(&format!(
            "--- 片段 {} (chunk_id: {}, sequence: {}) ---\n{}\n\n",
            i + 1,
            chunk.id,
            chunk.sequence,
            chunk.content,
        ));
    }

    prompt
}

/// 构建 Verify 阶段系统提示词（REQ-AUDIT-003）。
///
/// 引导 LLM 判断两个声明是否矛盾。prompt 中避免使用 "提取" 和 "claim" 关键词，
/// 使 Mock LLM 能区分 Verify 阶段。
fn build_verify_prompt(claim_a: &Claim, claim_b: &Claim) -> String {
    format!(
        "你是文档审计助手。请判断以下两个声明是否互相矛盾。\n\n\
         声明 A（来源片段 {a_seq}）：{a_text}\n\
         声明 B（来源片段 {b_seq}）：{b_text}\n\n\
         判定标准：\n\
         - contradiction: 两声明互相冲突，不能同时为真\n\
         - consistent: 两声明不矛盾，可以同时为真\n\
         - unverifiable: 信息不足或语义模糊，无法判定\n\n\
         输出格式：JSON 对象，包含以下字段：\n\
         - pair_id: 矛盾对标识（如 \"p1\"）\n\
         - verdict: 判定结果（contradiction / consistent / unverifiable）\n\
         - explanation: 判定理由说明\n\
         - severity: 严重等级（high / medium / low）\n\n\
         仅输出 JSON，不要添加 Markdown 代码块标记。",
        a_seq = claim_a.sequence,
        a_text = claim_a.text,
        b_seq = claim_b.sequence,
        b_text = claim_b.text,
    )
}

// ================== 论文审稿扩展（A1-1）==================

/// 审计模式（A1-1 论文审稿扩展）。
///
/// 一致性检测（现有）vs 论文审稿（新扩展）：
/// - `ConsistencyCheck`：跨 chunk 矛盾检测（Decompose-Then-Verify 流水线）
/// - `PaperReview`：论文级审稿（方法学/统计/逻辑自洽性评估）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AuditMode {
    /// 一致性检测（REQ-AUDIT-001~005，默认模式）
    #[default]
    ConsistencyCheck,
    /// 论文审稿（A1-1 扩展，结构化审稿维度评估）
    PaperReview,
}

/// 论文审稿维度（A1-1）。
///
/// 参考 Google PAT 框架（arXiv:2606.28277）和 FIND 基准（arXiv:2512.18601），
/// 将论文审稿分解为多个独立评估维度。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDimension {
    /// 方法学审查：研究设计、实验方法、对照组设置是否合理
    Methodology,
    /// 统计分析验证：统计方法选择、样本量、p 值报告、效应量计算
    Statistics,
    /// 逻辑自洽性：论证链条完整性、假设-推理-结论一致性
    Logic,
    /// 结果可重复性：实验条件描述、参数公开、数据可得性
    Reproducibility,
}

impl ReviewDimension {
    /// 返回维度的中文标签（用于报告渲染）
    pub fn label(&self) -> &'static str {
        match self {
            Self::Methodology => "方法学审查",
            Self::Statistics => "统计分析验证",
            Self::Logic => "逻辑自洽性",
            Self::Reproducibility => "结果可重复性",
        }
    }
}

/// 论文审稿发现（A1-1）。
///
/// 单个维度的审稿结果，包含发现的问题和严重等级。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewFinding {
    /// 审稿维度
    pub dimension: ReviewDimension,
    /// 发现的问题描述
    pub issue: String,
    /// 具体位置引用（chunk sequence）
    pub chunk_sequence: Option<usize>,
    /// 建议改进措施
    pub recommendation: String,
    /// 严重等级
    pub severity: Severity,
}

/// 论文审稿报告（A1-1）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperReviewReport {
    /// 审稿文档名
    pub doc_name: String,
    /// 文档 chunk 总数
    pub total_chunks: usize,
    /// 各维度发现列表
    pub findings: Vec<ReviewFinding>,
    /// 总体评价
    pub overall_assessment: String,
    /// 审稿耗时（毫秒）
    pub elapsed_ms: u64,
}

/// 构建论文审稿系统提示词（A1-1）。
///
/// 引导 LLM 从 4 个维度对论文进行结构化审稿：
/// 1. 方法学审查：研究设计是否合理，对照组设置是否充分
/// 2. 统计分析验证：统计方法选择是否正确，样本量是否充分
/// 3. 逻辑自洽性：假设→推理→结论链条是否完整一致
/// 4. 结果可重复性：实验条件描述是否充分，参数是否公开
///
/// 输出 JSON 数组格式，每个发现包含 dimension/issue/chunk_sequence/recommendation/severity。
pub fn build_paper_review_prompt(chunks: &[echomind_models::Chunk]) -> String {
    let mut prompt = String::from(
        "你是学术论文审稿助手。请从以下 4 个维度对论文内容进行结构化审稿：\n\n\
         1. methodology（方法学审查）：研究设计是否合理？实验方法是否恰当？对照组设置是否充分？\n\
         2. statistics（统计分析验证）：统计方法选择是否正确？样本量是否充分？p 值和效应量是否正确报告？\n\
         3. logic（逻辑自洽性）：假设→推理→结论链条是否完整？是否存在逻辑跳跃或循环论证？\n\
         4. reproducibility（结果可重复性）：实验条件描述是否充分？关键参数是否公开？数据可得性如何？\n\n\
         请逐片段扫描，对每个发现的问题输出 JSON 对象，包含以下字段：\n\
         - dimension: 审稿维度（methodology / statistics / logic / reproducibility）\n\
         - issue: 问题描述（具体指出论文中的不足）\n\
         - chunk_sequence: 来源片段序号（整数或 null）\n\
         - recommendation: 建议改进措施\n\
         - severity: 严重等级（high / medium / low）\n\n\
         如果某维度未发现问题，不输出该维度的条目。\n\
         输出格式：JSON 数组，仅输出 JSON，不要添加 Markdown 代码块标记。\n\n",
    );

    for (i, chunk) in chunks.iter().enumerate() {
        prompt.push_str(&format!(
            "--- 片段 {} (chunk_id: {}, sequence: {}) ---\n{}\n\n",
            i + 1,
            chunk.id,
            chunk.sequence,
            chunk.content,
        ));
    }

    prompt
}

// ================== 论文审稿扩展测试 ==================

#[cfg(test)]
mod paper_review_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    /// TC-AUDIT-006：`AuditMode` 枚举默认值为 ConsistencyCheck（A1-1）。
    #[test]
    fn tc_audit_006_default_mode() {
        let mode = AuditMode::default();
        assert_eq!(mode, AuditMode::ConsistencyCheck);
    }

    /// TC-AUDIT-007：`ReviewDimension` 标签正确（A1-1）。
    #[test]
    fn tc_audit_007_review_dimension_labels() {
        assert_eq!(ReviewDimension::Methodology.label(), "方法学审查");
        assert_eq!(ReviewDimension::Statistics.label(), "统计分析验证");
        assert_eq!(ReviewDimension::Logic.label(), "逻辑自洽性");
        assert_eq!(ReviewDimension::Reproducibility.label(), "结果可重复性");
    }

    /// TC-AUDIT-008：`build_paper_review_prompt` 包含 4 个维度关键词（A1-1）。
    #[test]
    fn tc_audit_008_paper_review_prompt_contains_dimensions() {
        let chunks = vec![echomind_models::Chunk {
            id: "test-1".to_string(),
            doc_id: "doc-1".to_string(),
            content: "We used a sample size of 30 and applied t-test.".to_string(),
            token_count: 20,
            sequence: 0,
        }];
        let prompt = build_paper_review_prompt(&chunks);

        // 验证 prompt 包含 4 个维度关键词
        assert!(
            prompt.contains("methodology"),
            "prompt 应包含 methodology 维度"
        );
        assert!(
            prompt.contains("statistics"),
            "prompt 应包含 statistics 维度"
        );
        assert!(prompt.contains("logic"), "prompt 应包含 logic 维度");
        assert!(
            prompt.contains("reproducibility"),
            "prompt 应包含 reproducibility 维度"
        );

        // 验证 prompt 包含 chunk 内容
        assert!(prompt.contains("sample size of 30"));
    }

    /// TC-AUDIT-009：`PaperReviewReport` 序列化/反序列化往返（A1-1）。
    #[test]
    fn tc_audit_009_paper_review_report_serde() {
        let report = PaperReviewReport {
            doc_name: "test-paper.pdf".to_string(),
            total_chunks: 10,
            findings: vec![ReviewFinding {
                dimension: ReviewDimension::Statistics,
                issue: "样本量不足，30 样本可能无法检测中等效应".to_string(),
                chunk_sequence: Some(3),
                recommendation: "建议进行功效分析，确定最小样本量".to_string(),
                severity: Severity::Medium,
            }],
            overall_assessment: "论文方法学基本合理，但统计分析需加强".to_string(),
            elapsed_ms: 5000,
        };

        let json = serde_json::to_string(&report).expect("序列化失败");
        let deserialized: PaperReviewReport = serde_json::from_str(&json).expect("反序列化失败");

        assert_eq!(deserialized.doc_name, "test-paper.pdf");
        assert_eq!(deserialized.findings.len(), 1);
        assert_eq!(
            deserialized.findings[0].dimension,
            ReviewDimension::Statistics
        );
        assert_eq!(deserialized.findings[0].severity, Severity::Medium);
    }

    /// TC-AUDIT-010：空 chunk 列表的 prompt 仍正确构建（A1-1 边界测试）。
    #[test]
    fn tc_audit_010_empty_chunks_prompt() {
        let prompt = build_paper_review_prompt(&[]);
        assert!(
            prompt.contains("审稿助手"),
            "空 chunk 列表 prompt 仍应包含系统指令"
        );
    }
}
