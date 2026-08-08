//! 代码符号引擎（REQ-RAG-031 代码感知 RAG，Pro feature）。
//!
//! 借鉴 IfAI SymbolEngine：使用 tree-sitter 对代码文件进行 AST 级符号分析，
//! 建立「符号 → 位置 → chunk」三级映射，使代码查询能精确定位到函数定义而非模糊匹配整个文件。
//!
//! 支持 Rust / TypeScript / Python / Go 四种语言，纯 CPU 解析（零 LLM 调用），典型文件 < 5ms。
//!
//! # 架构
//!
//! - 端口（trait）：`SymbolExtractor` 定义在 `crates/core/src/lib.rs`
//! - 适配器（impl）：`SymbolEngine` 定义在本文件（infra 层，Pro 门控）
//! - 注入方式：泛型参数 `E: SymbolExtractor`（同 `PageRenderer` / `OcrEngine` 模式）

#![cfg(feature = "pro")]

use std::path::Path;

use echomind_core::SymbolExtractor;
use echomind_models::{CodeSymbol, SymbolKind};
use tree_sitter::{Node, Parser, Tree};

/// 代码符号引擎（tree-sitter AST 抽取）。
///
/// 零大小类型（ZST），无内部状态，所有方法无副作用。
pub struct SymbolEngine;

impl SymbolEngine {
    /// 根据语言标识创建 tree-sitter Parser。
    ///
    /// 返回 `None` 表示不支持的语言（调用方应优雅降级）。
    fn create_parser(language: &str) -> Option<Parser> {
        let mut parser = Parser::new();
        let lang: tree_sitter::Language = match language {
            "rust" => tree_sitter_rust::LANGUAGE.into(),
            "typescript" | "tsx" => tree_sitter_typescript::LANGUAGE_TSX.into(),
            "python" => tree_sitter_python::LANGUAGE.into(),
            "go" => tree_sitter_go::LANGUAGE.into(),
            _ => return None,
        };
        parser.set_language(&lang).ok()?;
        Some(parser)
    }

    /// 从 AST 节点提取文本（安全字节切片）。
    fn node_text<'a>(content: &'a str, node: &Node) -> &'a str {
        content
            .get(node.start_byte()..node.end_byte())
            .unwrap_or("")
    }

    /// 判断节点是否为方法（位于 impl / class 内部）。
    fn is_method(node: &Node, language: &str) -> bool {
        let impl_kinds: &[&str] = match language {
            "rust" => &["impl_item"],
            "python" => &["class_definition"],
            "typescript" | "tsx" => &["class_declaration", "class_body"],
            "go" => &[],
            _ => &[],
        };
        if impl_kinds.is_empty() {
            return false;
        }
        let mut current = node.parent();
        while let Some(parent) = current {
            if impl_kinds.contains(&parent.kind()) {
                return true;
            }
            current = parent.parent();
        }
        false
    }

    /// 分类节点类型为 SymbolKind。
    ///
    /// 返回 `None` 表示该节点不是符号定义。
    fn classify_node(node: &Node, language: &str) -> Option<SymbolKind> {
        match (language, node.kind()) {
            // Rust
            ("rust", "function_item") => {
                if Self::is_method(node, "rust") {
                    Some(SymbolKind::Method)
                } else {
                    Some(SymbolKind::Function)
                }
            }
            ("rust", "struct_item") => Some(SymbolKind::Struct),
            ("rust", "enum_item") => Some(SymbolKind::Enum),
            ("rust", "trait_item") => Some(SymbolKind::Interface),
            ("rust", "const_item") => Some(SymbolKind::Constant),
            ("rust", "mod_item") => Some(SymbolKind::Module),

            // TypeScript
            ("typescript", "function_declaration") => Some(SymbolKind::Function),
            ("typescript", "class_declaration") => Some(SymbolKind::Class),
            ("typescript", "interface_declaration") => Some(SymbolKind::Interface),
            ("typescript", "enum_declaration") => Some(SymbolKind::Enum),
            ("typescript", "type_alias_declaration") => Some(SymbolKind::Constant),
            ("typescript", "method_definition") => Some(SymbolKind::Method),

            // Python
            ("python", "function_definition") => {
                if Self::is_method(node, "python") {
                    Some(SymbolKind::Method)
                } else {
                    Some(SymbolKind::Function)
                }
            }
            ("python", "class_definition") => Some(SymbolKind::Class),

            // Go
            ("go", "function_declaration") => Some(SymbolKind::Function),
            ("go", "method_declaration") => Some(SymbolKind::Method),
            ("go", "type_spec") => {
                // 根据 type 字段判断 Struct 或 Interface
                if let Some(type_node) = node.child_by_field_name("type") {
                    match type_node.kind() {
                        "struct_type" => Some(SymbolKind::Struct),
                        "interface_type" => Some(SymbolKind::Interface),
                        _ => Some(SymbolKind::Struct),
                    }
                } else {
                    Some(SymbolKind::Struct)
                }
            }

            _ => None,
        }
    }

    /// 提取函数/类签名（声明部分，不含函数体）。
    fn extract_signature(node: &Node, content: &str, language: &str) -> Option<String> {
        if language == "python" {
            // Python：取第一行（含 def/class ... : ）
            let text = Self::node_text(content, node);
            let first_line = text.lines().next()?;
            return Some(first_line.trim().to_string());
        }

        // 其他语言：从节点开始到第一个 { 或 ;
        let start = node.start_byte();
        let end = node.end_byte();
        let text = content.get(start..end)?;
        let sig_end = text.find(['{', ';'])?;
        let sig = &text[..sig_end];
        Some(sig.trim().to_string())
    }

    /// 获取 impl 块的类型名（Rust）。
    fn get_impl_type_name(node: &Node, content: &str) -> Option<String> {
        // impl_item 的 type 字段包含类型名
        let type_node = node.child_by_field_name("type")?;
        Some(Self::node_text(content, &type_node).to_string())
    }

    /// 粗略估算 token 数（4 字节 ≈ 1 token）。
    fn estimate_tokens(text: &str) -> usize {
        (text.len() / 4).max(1)
    }

    /// 按行分割大文本（每个子块不超过 max_tokens）。
    fn split_text_by_lines(text: &str, max_tokens: usize) -> Vec<String> {
        let lines: Vec<&str> = text.lines().collect();
        let mut chunks = Vec::new();
        let mut current = String::new();
        let mut current_tokens = 0usize;

        for line in lines {
            let line_tokens = Self::estimate_tokens(line);
            if current_tokens + line_tokens > max_tokens && !current.is_empty() {
                chunks.push(std::mem::take(&mut current));
                current_tokens = 0;
            }
            current.push_str(line);
            current.push('\n');
            current_tokens += line_tokens;
        }

        if !current.is_empty() {
            chunks.push(current);
        }

        if chunks.is_empty() {
            chunks.push(text.to_string());
        }
        chunks
    }

    /// 递归遍历 AST 节点，抽取所有符号。
    fn walk_and_extract(
        node: &Node,
        content: &str,
        language: &str,
        chunk_id: &str,
        symbols: &mut Vec<CodeSymbol>,
    ) {
        // 尝试从此节点提取符号
        if let Some(kind) = Self::classify_node(node, language)
            && let Some(name_node) = node.child_by_field_name("name")
        {
            let name = Self::node_text(content, &name_node).to_string();
            if !name.is_empty() {
                let signature = Self::extract_signature(node, content, language);
                symbols.push(CodeSymbol::new(
                    chunk_id.to_string(),
                    name,
                    kind,
                    language.to_string(),
                    node.start_position().row + 1, // 1-based
                    node.end_position().row + 1,
                    signature,
                ));
            }
        }

        // 递归遍历子节点
        for i in 0..node.named_child_count() {
            if let Some(child) = node.named_child(i) {
                Self::walk_and_extract(&child, content, language, chunk_id, symbols);
            }
        }
    }

    /// 判断顶级节点是否为「符号容器」（impl / type_declaration 等）。
    ///
    /// 符号容器本身不是符号，但包含子符号（方法 / 类型定义）。
    /// 在 `split_by_symbols` 中，符号容器作为一个整体 chunk。
    fn is_symbol_container(node: &Node, language: &str) -> bool {
        matches!(
            (language, node.kind()),
            ("rust", "impl_item") | ("go", "type_declaration")
        )
    }

    /// 判断顶级节点是否为符号定义或符号容器。
    fn is_symbol_or_container(node: &Node, language: &str) -> bool {
        Self::classify_node(node, language).is_some() || Self::is_symbol_container(node, language)
    }
}

impl SymbolExtractor for SymbolEngine {
    fn detect_language(&self, file_path: &str) -> Option<String> {
        let ext = Path::new(file_path).extension()?.to_str()?.to_lowercase();
        let lang = match ext.as_str() {
            "rs" => "rust",
            "ts" | "tsx" => "typescript",
            "py" => "python",
            "go" => "go",
            _ => return None,
        };
        Some(lang.to_string())
    }

    fn extract_symbols(&self, content: &str, language: &str, chunk_id: &str) -> Vec<CodeSymbol> {
        let mut parser = match Self::create_parser(language) {
            Some(p) => p,
            None => return Vec::new(),
        };

        let tree: Tree = match parser.parse(content, None) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let root = tree.root_node();
        let mut symbols = Vec::new();
        Self::walk_and_extract(&root, content, language, chunk_id, &mut symbols);
        symbols
    }

    fn split_by_symbols(
        &self,
        content: &str,
        language: &str,
        max_tokens: usize,
    ) -> Vec<(String, usize, usize)> {
        let mut parser = match Self::create_parser(language) {
            Some(p) => p,
            None => {
                // 解析失败：回退为整文本一个 chunk
                return vec![(content.to_string(), 1, content.lines().count())];
            }
        };

        let tree: Tree = match parser.parse(content, None) {
            Some(t) => t,
            None => {
                return vec![(content.to_string(), 1, content.lines().count())];
            }
        };

        let root = tree.root_node();
        let mut chunks = Vec::new();
        let mut header_buffer = String::new();
        let mut header_start_line = 1usize;

        for i in 0..root.named_child_count() {
            if let Some(child) = root.named_child(i) {
                let start_line = child.start_position().row + 1;
                let end_line = child.end_position().row + 1;
                let node_text = Self::node_text(content, &child);

                if Self::is_symbol_or_container(&child, language) {
                    // Flush header buffer
                    if !header_buffer.trim().is_empty() {
                        chunks.push((
                            std::mem::take(&mut header_buffer),
                            header_start_line,
                            start_line.saturating_sub(1),
                        ));
                    }

                    // Determine symbol name and kind for prefix
                    let (name, kind_str) = if Self::is_symbol_container(&child, language) {
                        // For impl blocks, use the type name
                        if language == "rust" {
                            let type_name = Self::get_impl_type_name(&child, content)
                                .unwrap_or_else(|| "Unknown".to_string());
                            (type_name, "impl")
                        } else {
                            ("types".to_string(), "declarations")
                        }
                    } else if let Some(kind) = Self::classify_node(&child, language) {
                        let name = child
                            .child_by_field_name("name")
                            .map(|n| Self::node_text(content, &n).to_string())
                            .unwrap_or_else(|| "anonymous".to_string());
                        (name, kind.as_str())
                    } else {
                        ("unknown".to_string(), "symbol")
                    };

                    // Build context prefix
                    let mut prefix = String::new();
                    if language == "rust" && child.kind() == "impl_item" {
                        prefix.push_str(&format!("// impl {}\n", name));
                    }
                    prefix.push_str(&format!("// Symbol: {} ({})\n", name, kind_str));

                    // Check if the symbol is too large
                    let token_count = Self::estimate_tokens(node_text);
                    if token_count > max_tokens {
                        // Split large symbol by lines
                        let sub_chunks = Self::split_text_by_lines(node_text, max_tokens);
                        for (idx, sub) in sub_chunks.iter().enumerate() {
                            let chunk_prefix = if idx == 0 {
                                prefix.clone()
                            } else {
                                format!("// Symbol: {} (continued)\n", name)
                            };
                            let sub_end_line = if idx + 1 == sub_chunks.len() {
                                end_line
                            } else {
                                start_line + sub.lines().count()
                            };
                            chunks.push((
                                format!("{}{}", chunk_prefix, sub),
                                if idx == 0 {
                                    start_line
                                } else {
                                    sub_end_line.saturating_sub(sub.lines().count())
                                },
                                sub_end_line,
                            ));
                        }
                    } else {
                        chunks.push((format!("{}{}", prefix, node_text), start_line, end_line));
                    }
                } else {
                    // Non-symbol item: accumulate in header buffer
                    if header_buffer.is_empty() {
                        header_start_line = start_line;
                    }
                    header_buffer.push_str(node_text);
                    header_buffer.push('\n');
                }
            }
        }

        // Flush remaining header buffer
        if !header_buffer.trim().is_empty() {
            chunks.push((header_buffer, header_start_line, content.lines().count()));
        }

        if chunks.is_empty() {
            chunks.push((content.to_string(), 1, content.lines().count()));
        }

        chunks
    }
}
