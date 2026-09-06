//! BM25 索引重建

use anyhow::Context;

use super::super::migration::Pool;
use crate::sqlite_storage::{run_db, with_transaction};

/// 重建 BM25 全文索引（REQ-PERF-005 Contextual BM25）。
pub(crate) async fn rebuild_bm25_index(pool: &Pool) -> anyhow::Result<()> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        with_transaction(&conn, |conn| {
            conn.execute_batch("DELETE FROM chunks_fts;")
                .context("清空 FTS5 索引失败")?;
            conn.execute_batch(
                "INSERT INTO chunks_fts (chunk_id, doc_id, content)
                 SELECT c.id, c.doc_id,
                   '文档《' ||
                   COALESCE(substr(d.file_path, instr(replace(d.file_path, '\\', '/'), '/') + 1), d.file_path)
                   || '》：\n' || c.content
                 FROM chunks c
                 JOIN documents d ON d.id = c.doc_id;",
            )
            .context("重建 FTS5 索引失败")?;
            Ok(())
        })
    })
    .await
}
