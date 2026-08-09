//! 向量检索器（REQ-RAG-003）：Embedder + Storage 编排与相关度阈值过滤。
//! 低于阈值的检索结果直接丢弃，防止低质量上下文污染 LLM 提示词。
//!
//! Phase 4 Chunk Expansion：检索命中后扩展前后各 1 个相邻 chunk，
//! 补全跨 chunk 上下文（如被分块器切断的段落、代码块后续行），
//! 显著减少「回答缺少关键上下文」的检索质量问题。

use echomind_models::RetrievalResult;

use crate::{Embedder, Retriever, Storage};

/// 默认相关度阈值：低于该分数的检索结果直接丢弃（REQ-RAG-003）
/// 0.35：all-MiniLM-L6-v2 对中文查询与 256-token 分块的余弦相似度通常在 0.3-0.5 区间，
/// 0.5 阈值会误杀大量有效结果导致空检索（用户反馈：上传文档后提问"没有任何反应"）。
pub const DEFAULT_SCORE_THRESHOLD: f32 = 0.35;

/// Phase 3 ContextualEmbedding：为 chunk 内容拼接文档名上下文前缀（低成本规则方案）。
///
/// 嵌入时使用此文本而非纯 chunk.content，使向量包含文档上下文信息，
/// 提升检索质量（Anthropic Contextual Retrieval 调研：检索失败率 ↓49%）。
///
/// EchoMind 采用低成本规则方案（零 LLM 调用）：仅拼接文档名前缀，
/// 不使用 LLM 生成上下文摘要（Anthropic 高成本方案，需每 chunk 调用 LLM）。
/// 后续可扩展为拼接 Markdown 标题层级（需 SemanticSplitter 记录标题元数据）。
pub fn build_contextual_text(doc_name: &str, chunk_content: &str) -> String {
    format!("文档《{doc_name}》：\n{chunk_content}")
}

/// Phase 4 Chunk Expansion：扩展命中 chunk 的相邻 chunk（前后各 1 个），合并去重。
///
/// 检索命中后，对每命中 chunk 调 `list_chunks` 获取同文档全部 chunk，
/// 找到命中 chunk 的位置，扩展前后各 1 个相邻 chunk，补全跨 chunk 上下文
///（如被分块器切断的段落、代码块后续行）。扩展的 chunk 使用命中 chunk 的
/// score 作为近似（同文档同主题假设）。结果按 (doc_id, sequence) 排序，
/// 使 prompt 上下文按文档顺序排列，提升 LLM 理解连贯性。
///
/// 共享函数：VectorRetriever 和 HybridRetriever 均使用此函数进行 Chunk Expansion。
pub async fn expand_neighbors<S: Storage>(
    storage: &S,
    hits: &[RetrievalResult],
) -> anyhow::Result<Vec<RetrievalResult>> {
    use std::collections::{HashMap, HashSet};

    let mut expanded: Vec<RetrievalResult> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // **性能优化**：原实现为每个 hit 调用一次 list_chunks（N+1 查询问题）。
    // 当 top_k=8 且 8 条命中来自 8 个不同文档时，产生 8 次独立的 spawn_blocking DB 查询。
    // 优化：按 doc_id 分组，每个唯一 doc_id 只查询一次 list_chunks，结果缓存复用。
    let unique_doc_ids: HashSet<&str> = hits.iter().map(|h| h.chunk.doc_id.as_str()).collect();
    let mut chunks_cache: HashMap<String, Vec<echomind_models::Chunk>> = HashMap::new();
    for doc_id in &unique_doc_ids {
        let all_chunks = storage.list_chunks(doc_id).await?;
        chunks_cache.insert(doc_id.to_string(), all_chunks);
    }

    for hit in hits {
        // 添加命中 chunk 本身
        if seen.insert(hit.chunk.id.clone()) {
            expanded.push(hit.clone());
        }

        // 从缓存中获取同文档全部 chunk
        let all_chunks = match chunks_cache.get(&hit.chunk.doc_id) {
            Some(c) if !c.is_empty() => c,
            _ => continue,
        };

        // 找到命中 chunk 的位置，扩展前后各 1 个
        if let Some(idx) = all_chunks.iter().position(|c| c.id == hit.chunk.id) {
            // 前一个 chunk
            if idx > 0 {
                let prev = &all_chunks[idx - 1];
                if seen.insert(prev.id.clone()) {
                    expanded.push(RetrievalResult {
                        chunk: prev.clone(),
                        score: hit.score,
                        doc_name: hit.doc_name.clone(),
                    });
                }
            }
            // 后一个 chunk
            if idx + 1 < all_chunks.len() {
                let next = &all_chunks[idx + 1];
                if seen.insert(next.id.clone()) {
                    expanded.push(RetrievalResult {
                        chunk: next.clone(),
                        score: hit.score,
                        doc_name: hit.doc_name.clone(),
                    });
                }
            }
        }
    }

    // 按 (doc_id, sequence) 排序，使 prompt 上下文按文档顺序排列
    expanded.sort_by(|a, b| {
        a.chunk
            .doc_id
            .cmp(&b.chunk.doc_id)
            .then(a.chunk.sequence.cmp(&b.chunk.sequence))
    });

    Ok(expanded)
}

/// Proposition 级检索 → chunk 扩展（REQ-PERF-007）。
///
/// 在 proposition 嵌入表上执行向量检索，命中后扩展到包含的 chunk，
/// 再通过 `expand_neighbors` 补全相邻 chunk 上下文。
///
/// 与 `VectorRetriever.retrieve()` 的区别：
/// - `retrieve()` 在 chunk 级嵌入上检索（粒度粗，一个 chunk 可能包含多个事实）
/// - `proposition_search_and_expand()` 在 proposition 级嵌入上检索（粒度细，
///   每个 proposition 是自包含的原子事实），命中率提升 30-50%
///   （Dense X Retrieval 论文 arXiv:2312.06648）
///
/// # 参数
/// - `storage`: 存储适配器（需实现 `proposition_search`）
/// - `query_embedding`: 查询的嵌入向量
/// - `top_k`: 返回结果数量上限
/// - `score_threshold`: 最低相似度阈值（低于此值丢弃）
///
/// # 返回
/// 扩展后的 RetrievalResult 列表（已去重 + 按 doc_id/sequence 排序）。
/// 如果 proposition 表为空或无命中，返回空 Vec（不报错）。
pub async fn proposition_search_and_expand<S: Storage>(
    storage: &S,
    query_embedding: &[f32],
    top_k: usize,
    score_threshold: f32,
) -> anyhow::Result<Vec<RetrievalResult>> {
    let mut hits = storage.proposition_search(query_embedding, top_k).await?;
    hits.retain(|h| h.score >= score_threshold);
    if hits.is_empty() {
        return Ok(vec![]);
    }
    expand_neighbors(storage, &hits).await
}

/// 符号检索（REQ-RAG-031 代码感知 RAG）：查询中包含函数/类名时优先精确匹配符号。
///
/// 检索策略：
/// 1. 从 query 中提取可能的符号名（camelCase / snake_case / PascalCase / `::` 路径）
/// 2. `search_by_symbol(name)` 精确匹配（score = 1.0）
/// 3. `search_symbols_fuzzy(query)` 模糊匹配（score = 0.8）
/// 4. 合并去重 → RetrievalResult
///
/// 不足 `top_k` 时由调用方补充向量检索。
///
/// # 参数
/// - `storage`: 存储适配器
/// - `query`: 用户查询文本
/// - `top_k`: 返回结果数量上限
pub async fn symbol_search<S: Storage>(
    storage: &S,
    query: &str,
    top_k: usize,
) -> anyhow::Result<Vec<RetrievalResult>> {
    use std::collections::HashSet;

    if top_k == 0 {
        return Ok(Vec::new());
    }

    // 1. 从 query 中提取可能的符号名
    let symbol_names = extract_symbol_names(query);

    let mut results = Vec::new();
    let mut seen_chunks: HashSet<String> = HashSet::new();

    // 2. 精确匹配每个符号名
    for name in &symbol_names {
        let symbols = storage.search_by_symbol(name, None).await?;
        for sym in symbols {
            if seen_chunks.insert(sym.chunk_id.clone())
                && let Some(chunk) = storage.get_chunk_by_id(&sym.chunk_id).await?
            {
                results.push(RetrievalResult {
                    chunk,
                    score: 1.0, // 精确匹配
                    doc_name: String::new(),
                });
            }
        }
    }

    // 3. 模糊匹配（若精确匹配不足）
    if results.len() < top_k {
        let fuzzy = storage.search_symbols_fuzzy(query, top_k * 2).await?;
        for sym in fuzzy {
            if seen_chunks.insert(sym.chunk_id.clone())
                && let Some(chunk) = storage.get_chunk_by_id(&sym.chunk_id).await?
            {
                results.push(RetrievalResult {
                    chunk,
                    score: 0.8, // 模糊匹配
                    doc_name: String::new(),
                });
            }
        }
    }

    // 4. 截断到 top_k
    results.truncate(top_k);

    Ok(results)
}

/// 从查询文本中提取可能的编程标识符（符号名）。
///
/// 识别模式：
/// - `PascalCase`：首字母大写（如 `HashMap`、`FooImpl`）
/// - `camelCase`：首字母小写含大写（如 `helloWorld`）
/// - `snake_case`：含下划线（如 `hello_world`、`__init__`）
/// - `::` 路径分隔：`HashMap::insert` → 提取 `HashMap` 和 `insert`
fn extract_symbol_names(query: &str) -> Vec<String> {
    let mut names = Vec::new();
    for word in query.split_whitespace() {
        // 按 :: 分割（Rust / C++ 路径分隔符）
        for part in word.split("::") {
            let cleaned: String = part
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if cleaned.len() >= 2 && is_identifier_like(&cleaned) {
                names.push(cleaned);
            }
        }
    }
    names
}

/// 判断字符串是否像编程标识符。
fn is_identifier_like(s: &str) -> bool {
    // 含下划线 → snake_case
    // 混合大小写 → camelCase
    // 首字母大写 → PascalCase
    s.contains('_')
        || (s.chars().any(|c| c.is_uppercase()) && s.chars().any(|c| c.is_lowercase()))
        || s.starts_with(char::is_uppercase)
}

/// 向量检索适配器：query → embedding → vector_search → 阈值过滤。
pub struct VectorRetriever<E: Embedder, S: Storage> {
    embedder: E,
    storage: S,
    score_threshold: f32,
}

impl<E: Embedder, S: Storage> VectorRetriever<E, S> {
    pub fn new(embedder: E, storage: S) -> Self {
        Self {
            embedder,
            storage,
            score_threshold: DEFAULT_SCORE_THRESHOLD,
        }
    }
}

impl<E: Embedder, S: Storage> Retriever for VectorRetriever<E, S> {
    async fn retrieve(&self, query: &str, top_k: usize) -> anyhow::Result<Vec<RetrievalResult>> {
        let embedding = self.embedder.embed(query).await?;
        self.retrieve_with_embedding(query, &embedding, top_k).await
    }

    /// 性能优化：使用预计算嵌入跳过冗余 ONNX 推理（省 ~50-100ms）。
    async fn retrieve_with_embedding(
        &self,
        _query: &str,
        query_embedding: &[f32],
        top_k: usize,
    ) -> anyhow::Result<Vec<RetrievalResult>> {
        let mut hits = self.storage.vector_search(query_embedding, top_k).await?;
        hits.retain(|h| h.score >= self.score_threshold);
        expand_neighbors(&self.storage, &hits).await
    }
}
