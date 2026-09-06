//! 向量与分块表 CRUD 操作（从 `sqlite_storage.rs` 拆分）。
//!
//! v2.0 S03 拆分：分块写入、向量写入、关键词检索（FTS5 BM25）、
//! 嵌入缓存、BM25 索引重建、分块查询/删除、路径查找、FTS5 查询构建。
//!
//! 所有函数接收 `&Pool` 参数，不持有 `&self`，可见性 `pub(crate)`。

pub(crate) mod bm25;
pub(crate) mod cache;
pub(crate) mod chunks;
pub(crate) mod embeddings;
pub(crate) mod fts5;
pub(crate) mod keyword;

// 重导出所有公共函数
#[allow(unused_imports)]
pub(crate) use bm25::rebuild_bm25_index;
#[allow(unused_imports)]
pub(crate) use cache::{
    cache_embedding_sync, find_cached_embedding_sync, lookup_embedding_cache,
    lookup_embedding_cache_batch, put_embedding_cache, put_embedding_cache_batch,
};
#[allow(unused_imports)]
pub(crate) use chunks::{
    add_chunk, add_chunks_batch, delete_chunks_by_doc, get_chunk_by_id, list_chunks,
};
#[allow(unused_imports)]
pub(crate) use embeddings::{add_embedding, add_embeddings_batch};
#[allow(unused_imports)]
pub(crate) use fts5::build_fts5_or_query;
#[allow(unused_imports)]
pub(crate) use keyword::keyword_search;
