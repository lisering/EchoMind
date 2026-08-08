//! 工具输出有界截断（B07 Tool Output Bounding，REQ-RAG-043）。
//!
//! 借鉴 OpenCode `tool-output-store.ts`：超大工具输出进行有界截断，
//! 保留开头和结尾，确保 LLM 上下文不被冗长输出淹没。
//!
//! ## 设计
//!
//! - **双限制**：行数 500 行 + 字节数 64KB，任一超标即截断
//! - **首尾保留**：前 250 行 + 后 250 行，中间插入截断标记
//! - **截断标记格式**：`[... 输出已截断，共 N 行 M 字节 ...]`
//! - **纯函数**：零 I/O、零 async、零依赖，可独立单元测试
//!
//! ## 集成点
//!
//! - `AgentEngine::execute_tool_single()` — `search_kb` 观察结果截断
//! - `AgentEngine::execute_code_tool()` — 代码执行结果截断

/// 工具输出行数上限（超过此值触发截断）。
pub const MAX_TOOL_OUTPUT_LINES: usize = 500;

/// 工具输出字节数上限（超过此值触发截断）。
pub const MAX_TOOL_OUTPUT_BYTES: usize = 64 * 1024; // 64KB

/// 有界截断后的工具输出。
///
/// 当原始输出未超限时，`truncated` 为 `false`，`text` 为原始内容。
/// 当原始输出超限时，`truncated` 为 `true`，`text` 为保留首尾 + 截断标记的内容。
/// `managed_path` 预留用于未来将完整输出存入临时文件（当前为 `None`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedOutput {
    /// 截断后的文本（或原始文本，如果未截断）
    pub text: String,
    /// 是否发生了截断
    pub truncated: bool,
    /// 完整输出的临时文件路径（预留，当前始终为 None）
    pub managed_path: Option<String>,
}

impl BoundedOutput {
    /// 创建未截断的输出。
    fn not_truncated(text: String) -> Self {
        Self {
            text,
            truncated: false,
            managed_path: None,
        }
    }

    /// 创建已截断的输出。
    fn truncated(text: String) -> Self {
        Self {
            text,
            truncated: true,
            managed_path: None,
        }
    }
}

/// 对工具输出进行有界截断。
///
/// 当输出行数 ≤ `MAX_TOOL_OUTPUT_LINES` 且字节数 ≤ `MAX_TOOL_OUTPUT_BYTES` 时，
/// 原样返回（`truncated = false`）。
///
/// 当任一限制超标时，保留前 `MAX_TOOL_OUTPUT_LINES / 2` 行和后 `MAX_TOOL_OUTPUT_LINES / 2` 行，
/// 中间插入截断标记 `[... 输出已截断，共 N 行 M 字节 ...]`。
///
/// # 空输出安全
///
/// 空字符串返回 `BoundedOutput { text: "", truncated: false, managed_path: None }`。
///
/// # 示例
///
/// ```
/// use echomind_core::tool_output::{bound_tool_output, MAX_TOOL_OUTPUT_LINES};
///
/// // 短输出不截断
/// let result = bound_tool_output("hello world");
/// assert!(!result.truncated);
///
/// // 超行数限制截断
/// let long: String = (0..MAX_TOOL_OUTPUT_LINES + 100)
///     .map(|i| format!("line {i}"))
///     .collect::<Vec<_>>()
///     .join("\n");
/// let result = bound_tool_output(&long);
/// assert!(result.truncated);
/// ```
pub fn bound_tool_output(output: &str) -> BoundedOutput {
    let total_bytes = output.len();
    let lines: Vec<&str> = output.lines().collect();
    let total_lines = lines.len();

    // 未超限：原样返回
    if total_lines <= MAX_TOOL_OUTPUT_LINES && total_bytes <= MAX_TOOL_OUTPUT_BYTES {
        return BoundedOutput::not_truncated(output.to_string());
    }

    // 空输出安全（理论上不会到这里，但防御性处理）
    if lines.is_empty() {
        return BoundedOutput::not_truncated(String::new());
    }

    // 保留前 N/2 行 + 后 N/2 行
    let head_lines = MAX_TOOL_OUTPUT_LINES / 2;
    let tail_lines = MAX_TOOL_OUTPUT_LINES / 2;

    let head_end = head_lines.min(total_lines);
    let tail_start = total_lines.saturating_sub(tail_lines);

    // 确保 head 和 tail 不重叠（行数刚好在临界值附近时全部保留）
    if head_end >= tail_start {
        // 行数不足以按行截断，但如果字节数超限，按字节截断
        if total_bytes > MAX_TOOL_OUTPUT_BYTES {
            let half_bytes = MAX_TOOL_OUTPUT_BYTES / 2;
            // UTF-8 安全：找到 <= half_bytes 的最近字符边界
            let head_end_byte = floor_char_boundary(output, half_bytes);
            let tail_start_byte =
                ceil_char_boundary(output, total_bytes.saturating_sub(half_bytes));
            let head_bytes = &output[..head_end_byte];
            let tail_bytes = &output[tail_start_byte..];
            let truncated_text = format!(
                "{}\n\n[... 输出已截断，共 {} 行 {} 字节 ...]\n\n{}",
                head_bytes, total_lines, total_bytes, tail_bytes,
            );
            return BoundedOutput::truncated(truncated_text);
        }
        return BoundedOutput::not_truncated(output.to_string());
    }

    let head: Vec<&str> = lines[..head_end].to_vec();
    let tail: Vec<&str> = lines[tail_start..].to_vec();

    let truncated_text = format!(
        "{}\n\n[... 输出已截断，共 {} 行 {} 字节 ...]\n\n{}",
        head.join("\n"),
        total_lines,
        total_bytes,
        tail.join("\n"),
    );

    BoundedOutput::truncated(truncated_text)
}

/// 找到 ≤ `index` 的最大 UTF-8 字符边界（防止切片切到多字节字符中间）。
///
/// 从 `index` 开始向前回退，直到找到字符边界（即该字节不是 UTF-8 续字节 `10xxxxxx`）。
fn floor_char_boundary(s: &str, mut index: usize) -> usize {
    if index >= s.len() {
        return s.len();
    }
    while !s.is_char_boundary(index) {
        index -= 1;
    }
    index
}

/// 找到 ≥ `index` 的最小 UTF-8 字符边界。
///
/// 从 `index` 开始向后前进，直到找到字符边界。
fn ceil_char_boundary(s: &str, mut index: usize) -> usize {
    if index >= s.len() {
        return s.len();
    }
    while !s.is_char_boundary(index) {
        index += 1;
    }
    index
}

// ============================================================================
// TDD 测试（TC-BOUND-001~005，对应 REQ-RAG-043 AC-1~AC-5）
// ============================================================================

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    /// TC-BOUND-001：短输出不截断（AC-1）。
    ///
    /// 验证行数和字节数均在限制内的输出原样返回，`truncated = false`。
    #[test]
    fn tc_bound_001_short_output_not_truncated() {
        let output = "Hello, world!\nThis is a short output.";
        let result = bound_tool_output(output);
        assert!(!result.truncated, "短输出不应截断");
        assert_eq!(result.text, output, "文本应与原始输出一致");
        assert!(result.managed_path.is_none(), "managed_path 应为 None");
    }

    /// TC-BOUND-002：超行数限制截断，保留首尾（AC-2）。
    ///
    /// 构造超过 MAX_TOOL_OUTPUT_LINES 行的输出，验证截断后包含首行和末行。
    #[test]
    fn tc_bound_002_exceeds_line_limit_truncated() {
        let total = MAX_TOOL_OUTPUT_LINES + 100;
        let lines: Vec<String> = (0..total).map(|i| format!("line_{i}")).collect();
        let input = lines.join("\n");

        let result = bound_tool_output(&input);
        assert!(result.truncated, "超行数限制应截断");

        // 首行存在
        assert!(result.text.contains("line_0"), "截断后应保留首行 line_0");
        // 末行存在
        assert!(
            result.text.contains(&format!("line_{}", total - 1)),
            "截断后应保留末行 line_{}",
            total - 1
        );
        // 中间行被截断
        assert!(
            !result.text.contains(&format!("line_{}", total / 2)),
            "中间行 line_{} 应被截断",
            total / 2
        );
    }

    /// TC-BOUND-003：超字节限制截断（AC-3）。
    ///
    /// 构造字节数超过 MAX_TOOL_OUTPUT_BYTES 但行数不超限的输出，
    /// 验证截断后输出字节数小于原始字节数。
    #[test]
    fn tc_bound_003_exceeds_byte_limit_truncated() {
        // 构造单行超长输出：64KB + 1KB，行数 < 500
        let big_line = "A".repeat(MAX_TOOL_OUTPUT_BYTES + 1024);
        let input = &big_line;

        let result = bound_tool_output(input);
        assert!(result.truncated, "超字节限制应截断");
        assert!(
            result.text.len() < input.len(),
            "截断后字节数应小于原始字节数"
        );
    }

    /// TC-BOUND-004：截断标记包含原始行数和字节数（AC-4）。
    ///
    /// 验证截断标记格式为 `[... 输出已截断，共 N 行 M 字节 ...]`，
    /// 其中 N 为原始行数，M 为原始字节数。
    #[test]
    fn tc_bound_004_truncation_marker_contains_stats() {
        let total = MAX_TOOL_OUTPUT_LINES + 10;
        let lines: Vec<String> = (0..total).map(|i| format!("line_{i}")).collect();
        let input = lines.join("\n");
        let total_bytes = input.len();

        let result = bound_tool_output(&input);
        assert!(result.truncated, "应触发截断");

        // 验证截断标记格式
        let expected_marker = format!("[... 输出已截断，共 {} 行 {} 字节 ...]", total, total_bytes);
        assert!(
            result.text.contains(&expected_marker),
            "截断标记应包含原始行数({})和字节数({})，实际文本: {}",
            total,
            total_bytes,
            &result.text[..200.min(result.text.len())]
        );
    }

    /// TC-BOUND-005：空输出安全处理（AC-5）。
    ///
    /// 验证空字符串返回 `truncated = false`，`text = ""`。
    #[test]
    fn tc_bound_005_empty_output_safe() {
        let result = bound_tool_output("");
        assert!(!result.truncated, "空输出不应截断");
        assert_eq!(result.text, "", "空输出文本应为空字符串");
        assert!(result.managed_path.is_none(), "managed_path 应为 None");
    }
}
