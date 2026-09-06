//! 实体/关系/摘要/命题/wiki链接/代码符号 CRUD 操作（从 `sqlite_storage.rs` 拆分）。
//!
//! v2.0 S03 拆分：实体索引、实体检索、关系图谱、RAPTOR 摘要树、
//! Wiki 双向链接、代码符号索引。
//!
//! 所有函数接收 `&Pool` 参数，不持有 `&self`，可见性 `pub(crate)`。

pub(crate) mod index;
pub(crate) mod relations;
pub(crate) mod search;
pub(crate) mod symbols;
pub(crate) mod wiki_links;

// 重导出所有公共函数
#[allow(unused_imports)]
pub(crate) use index::{add_entities, entity_search, get_entity_graph, get_entity_types};
#[allow(unused_imports)]
pub(crate) use relations::{
    add_relation, add_relations_batch, count_relations, get_relations_for_chunk,
    get_relations_for_entity, list_all_relations, search_by_relation,
};
#[allow(unused_imports)]
pub(crate) use search::search_entities;
#[allow(unused_imports)]
pub(crate) use symbols::{
    add_symbols, get_symbols_for_chunk, search_by_symbol, search_symbols_fuzzy,
};
#[allow(unused_imports)]
pub(crate) use wiki_links::{add_wiki_links, get_backlinks, get_forward_links};
