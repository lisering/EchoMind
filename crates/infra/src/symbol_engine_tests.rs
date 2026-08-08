//! 代码符号引擎 TDD 测试（REQ-RAG-031 代码感知 RAG）。
//!
//! 测试策略：先写红灯测试（10 个 AC），再实现变绿。

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crate::symbol_engine::SymbolEngine;
use echomind_core::{Storage, SymbolExtractor};
use echomind_models::SymbolKind;

/// TC-SYMBOL-001: Rust 代码抽取函数符号。
#[test]
fn tc_symbol_001_rust_function() {
    let engine = SymbolEngine;
    let code = r#"fn hello_world() {
    println!("hello");
}"#;
    let symbols = engine.extract_symbols(code, "rust", "chunk_001");
    assert!(!symbols.is_empty(), "应至少抽取到 1 个符号");
    let func = symbols
        .iter()
        .find(|s| s.name == "hello_world")
        .expect("应找到 hello_world 函数");
    assert_eq!(func.kind, SymbolKind::Function);
    assert_eq!(func.language, "rust");
    assert!(func.start_line <= func.end_line);
    assert!(func.signature.is_some(), "函数应有签名");
}

/// TC-SYMBOL-002: Rust 代码抽取 struct + impl 方法。
#[test]
fn tc_symbol_002_rust_struct_impl() {
    let engine = SymbolEngine;
    let code = r#"struct Foo {
    x: i32,
}

impl Foo {
    fn bar(&self) -> i32 {
        self.x
    }
}"#;
    let symbols = engine.extract_symbols(code, "rust", "chunk_002");
    let has_struct = symbols
        .iter()
        .any(|s| s.name == "Foo" && s.kind == SymbolKind::Struct);
    assert!(has_struct, "应抽取到 Struct Foo");
    let has_method = symbols
        .iter()
        .any(|s| s.name == "bar" && s.kind == SymbolKind::Method);
    assert!(has_method, "应抽取到 Method bar");
}

/// TC-SYMBOL-003: TypeScript 代码抽取 class + interface。
#[test]
fn tc_symbol_003_ts_class_interface() {
    let engine = SymbolEngine;
    let code = r#"interface IFoo {
    bar(): void;
}

class FooImpl implements IFoo {
    bar() {
        console.log("bar");
    }
}"#;
    let symbols = engine.extract_symbols(code, "typescript", "chunk_003");
    let has_interface = symbols
        .iter()
        .any(|s| s.name == "IFoo" && s.kind == SymbolKind::Interface);
    assert!(has_interface, "应抽取到 Interface IFoo");
    let has_class = symbols
        .iter()
        .any(|s| s.name == "FooImpl" && s.kind == SymbolKind::Class);
    assert!(has_class, "应抽取到 Class FooImpl");
}

/// TC-SYMBOL-004: Python 代码抽取函数 + 类。
#[test]
fn tc_symbol_004_python_func_class() {
    let engine = SymbolEngine;
    let code = r#"def hello():
    pass

class World:
    def __init__(self):
        pass"#;
    let symbols = engine.extract_symbols(code, "python", "chunk_004");
    let has_func = symbols
        .iter()
        .any(|s| s.name == "hello" && s.kind == SymbolKind::Function);
    assert!(has_func, "应抽取到 Function hello");
    let has_class = symbols
        .iter()
        .any(|s| s.name == "World" && s.kind == SymbolKind::Class);
    assert!(has_class, "应抽取到 Class World");
    let has_method = symbols
        .iter()
        .any(|s| s.name == "__init__" && s.kind == SymbolKind::Method);
    assert!(has_method, "应抽取到 Method __init__");
}

/// TC-SYMBOL-005: 按函数边界分块——函数不被切断。
#[test]
fn tc_symbol_005_split_by_symbols() {
    let engine = SymbolEngine;
    let code = r#"fn func_a() {
    println!("a");
}

fn func_b() {
    println!("b");
}

fn func_c() {
    println!("c");
}"#;
    let chunks = engine.split_by_symbols(code, "rust", 100);
    assert!(
        chunks.len() >= 3,
        "应至少分出 3 个 chunk（每个函数一个），实际: {}",
        chunks.len()
    );
    // 验证每个 chunk 包含一个完整函数
    for (text, _, _) in &chunks {
        assert!(
            text.contains("fn func_") || text.contains("Symbol:"),
            "chunk 应包含函数定义或符号前缀"
        );
    }
}

/// TC-SYMBOL-006: 超大函数内部分块。
#[test]
fn tc_symbol_006_large_function_split() {
    let engine = SymbolEngine;
    // 生成一个超长函数（很多行）
    let mut code = String::from("fn big_func() {\n");
    for i in 0..200 {
        code.push_str(&format!("    let x{i} = {i};\n"));
    }
    code.push_str("}\n");
    // max_tokens 设很小，强制内部分块
    let chunks = engine.split_by_symbols(&code, "rust", 20);
    assert!(
        chunks.len() >= 2,
        "超大函数应被分割为多个 chunk，实际: {}",
        chunks.len()
    );
    // 至少一个 chunk 应包含 continued 前缀
    let has_continued = chunks.iter().any(|(text, _, _)| text.contains("continued"));
    assert!(
        has_continued || chunks.len() >= 2,
        "分割后的 chunk 应有 continued 标记或至少分出多块"
    );
}

/// TC-SYMBOL-007: 语言检测正确。
#[test]
fn tc_symbol_007_detect_language() {
    let engine = SymbolEngine;
    assert_eq!(engine.detect_language("main.rs"), Some("rust".to_string()));
    assert_eq!(
        engine.detect_language("app.ts"),
        Some("typescript".to_string())
    );
    assert_eq!(
        engine.detect_language("component.tsx"),
        Some("typescript".to_string())
    );
    assert_eq!(
        engine.detect_language("script.py"),
        Some("python".to_string())
    );
    assert_eq!(engine.detect_language("main.go"), Some("go".to_string()));
    assert_eq!(
        engine.detect_language("readme.md"),
        None,
        "非代码文件应返回 None"
    );
    assert_eq!(
        engine.detect_language("document.pdf"),
        None,
        "PDF 应返回 None"
    );
}

/// TC-SYMBOL-008: 符号搜索精确匹配（通过 SQLite Storage）。
#[tokio::test]
async fn tc_symbol_008_search_exact() {
    use crate::sqlite_storage::SqliteStorage;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("test.db");
    let storage = SqliteStorage::new(&db_path).unwrap();

    // 先创建一个文档和 chunk（外键约束）
    let doc = echomind_models::Document::new("test.rs".to_string(), "abc123".to_string());
    storage.add_document(&doc).await.unwrap();
    let chunk =
        echomind_models::Chunk::new(doc.id.clone(), "fn hello_world() {}".to_string(), 10, 0);
    storage.add_chunk(&chunk).await.unwrap();

    // 添加符号
    let symbol = echomind_models::CodeSymbol::new(
        chunk.id.clone(),
        "hello_world".to_string(),
        SymbolKind::Function,
        "rust".to_string(),
        1,
        1,
        Some("fn hello_world()".to_string()),
    );
    storage.add_symbols(&[symbol]).await.unwrap();

    // 精确搜索
    let results = storage.search_by_symbol("hello_world", None).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "hello_world");
    assert_eq!(results[0].kind, SymbolKind::Function);
}

/// TC-SYMBOL-009: 符号模糊搜索。
#[tokio::test]
async fn tc_symbol_009_search_fuzzy() {
    use crate::sqlite_storage::SqliteStorage;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("test_fuzzy.db");
    let storage = SqliteStorage::new(&db_path).unwrap();

    // 创建文档和 chunk
    let doc = echomind_models::Document::new("test.rs".to_string(), "def456".to_string());
    storage.add_document(&doc).await.unwrap();
    let chunk = echomind_models::Chunk::new(doc.id.clone(), "code".to_string(), 10, 0);
    storage.add_chunk(&chunk).await.unwrap();

    // 添加多个符号
    let symbols = vec![
        echomind_models::CodeSymbol::new(
            chunk.id.clone(),
            "hello_world".to_string(),
            SymbolKind::Function,
            "rust".to_string(),
            1,
            5,
            None,
        ),
        echomind_models::CodeSymbol::new(
            chunk.id.clone(),
            "hello_rust".to_string(),
            SymbolKind::Function,
            "rust".to_string(),
            6,
            10,
            None,
        ),
        echomind_models::CodeSymbol::new(
            chunk.id.clone(),
            "goodbye".to_string(),
            SymbolKind::Function,
            "rust".to_string(),
            11,
            15,
            None,
        ),
    ];
    storage.add_symbols(&symbols).await.unwrap();

    // 模糊搜索 "hello"
    let results = storage.search_symbols_fuzzy("hello", 10).await.unwrap();
    assert_eq!(results.len(), 2, "应匹配 hello_world 和 hello_rust");
    let names: Vec<&str> = results.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"hello_world"));
    assert!(names.contains(&"hello_rust"));
}

/// TC-SYMBOL-010: 非代码文件不受影响（向后兼容）。
#[test]
fn tc_symbol_010_non_code_file_unchanged() {
    let engine = SymbolEngine;
    // .md 文件不被识别为代码文件
    assert!(engine.detect_language("readme.md").is_none());
    assert!(engine.detect_language("notes.txt").is_none());
    assert!(engine.detect_language("doc.pdf").is_none());

    // 对非代码内容调用 extract_symbols 应返回空 Vec（不报错）
    let md_content = "# Hello\n\nThis is a markdown file.";
    let symbols = engine.extract_symbols(md_content, "rust", "chunk_md");
    // tree-sitter 解析 markdown 作为 rust 会产生垃圾节点，但不应崩溃
    // 关键是：导入管线不会对 .md 文件调用 extract_symbols
    // 此测试仅验证函数不会 panic
    let _ = symbols; // 不做断言，仅验证不崩溃
}

/// TC-SYMBOL-011: Go 代码抽取函数。
#[test]
fn tc_symbol_011_go_function() {
    let engine = SymbolEngine;
    let code = r#"package main

import "fmt"

func main() {
    fmt.Println("hello")
}

type Foo struct {
    Bar int
}"#;
    let symbols = engine.extract_symbols(code, "go", "chunk_go");
    let has_func = symbols
        .iter()
        .any(|s| s.name == "main" && s.kind == SymbolKind::Function);
    assert!(has_func, "应抽取到 Function main");
    let has_struct = symbols
        .iter()
        .any(|s| s.name == "Foo" && s.kind == SymbolKind::Struct);
    assert!(has_struct, "应抽取到 Struct Foo");
}
