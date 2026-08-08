#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! QA 红灯测试：TC-MM-007/007b VLM 分级图表理解提示词（REQ-MM-005）。
//!
//! 验证 `VLM_TIERED_PROMPT` 常量包含 4 级精度提取策略的全部要素：
//! - Level 1 表格 → Markdown 表格
//! - Level 2 流程图/甘特图 → Mermaid
//! - Level 3 数据图表 → CSV 数据点 + 趋势 + 误差标注（AC-1, AC-3）
//! - Level 4 示意图/照片 → 简洁文字描述
//! - 数学公式 → LaTeX（AC-2）

use crate::vlm_prompt::VLM_TIERED_PROMPT;

// ================== TC-MM-007: 4 级分级图表理解策略（AC-1, AC-3） ==================

/// TC-MM-007：VLM 提示词包含 4 级分级策略框架（REQ-MM-005）。
///
/// 提示词必须明确区分四个精度等级，引导 VLM 按内容类型选择提取策略。
#[test]
fn tc_mm_007_prompt_contains_four_level_strategy() {
    assert!(
        !VLM_TIERED_PROMPT.is_empty(),
        "VLM_TIERED_PROMPT 不能为空（REQ-MM-005 未实现）"
    );

    // Level 1 — 表格 → Markdown 表格
    assert!(
        VLM_TIERED_PROMPT.contains("Level 1") || VLM_TIERED_PROMPT.contains("级别 1"),
        "AC: 提示词必须包含 Level 1 表格处理策略"
    );
    assert!(
        VLM_TIERED_PROMPT.contains("表格"),
        "AC: 提示词必须提及表格内容类型"
    );
    assert!(
        VLM_TIERED_PROMPT.contains("Markdown"),
        "AC: Level 1 表格必须转换为 Markdown 表格语法"
    );

    // Level 2 — 流程图/甘特图 → Mermaid
    assert!(
        VLM_TIERED_PROMPT.contains("Level 2") || VLM_TIERED_PROMPT.contains("级别 2"),
        "AC: 提示词必须包含 Level 2 流程图处理策略"
    );
    assert!(
        VLM_TIERED_PROMPT.contains("Mermaid"),
        "AC: Level 2 流程图/甘特图必须转换为 Mermaid 语法"
    );

    // Level 3 — 数据图表 → CSV（核心改进）
    assert!(
        VLM_TIERED_PROMPT.contains("Level 3") || VLM_TIERED_PROMPT.contains("级别 3"),
        "AC: 提示词必须包含 Level 3 数据图表处理策略"
    );
    assert!(
        VLM_TIERED_PROMPT.contains("数据图表"),
        "AC: 提示词必须明确数据图表内容类型"
    );

    // Level 4 — 示意图/照片 → 简洁文字描述
    assert!(
        VLM_TIERED_PROMPT.contains("Level 4") || VLM_TIERED_PROMPT.contains("级别 4"),
        "AC: 提示词必须包含 Level 4 示意图处理策略"
    );
}

/// TC-MM-007a：Level 3 数据图表 CSV 提取策略（AC-1）。
///
/// AC-1：含数据图表的图片经 VLM 处理后，chunk 中包含 CSV 格式的数据点
/// （坐标轴标签 + 至少 3 个数值点），而非仅文字趋势描述。
#[test]
fn tc_mm_007_level3_csv_extraction() {
    assert!(
        !VLM_TIERED_PROMPT.is_empty(),
        "VLM_TIERED_PROMPT 不能为空（REQ-MM-005 未实现）"
    );

    // 必须引导 VLM 提取坐标轴标签
    assert!(
        VLM_TIERED_PROMPT.contains("坐标轴"),
        "AC-1: 提示词必须引导提取坐标轴标签"
    );

    // 必须引导 VLM 提取数据系列名称
    assert!(
        VLM_TIERED_PROMPT.contains("数据系列"),
        "AC-1: 提示词必须引导提取数据系列名称"
    );

    // 必须以 CSV 格式输出数据点
    assert!(
        VLM_TIERED_PROMPT.contains("CSV"),
        "AC-1: 提示词必须要求以 CSV 格式输出数据点"
    );

    // 必须引导提取关键数值点（而非仅趋势描述）
    assert!(
        VLM_TIERED_PROMPT.contains("数值"),
        "AC-1: 提示词必须引导提取关键数值点"
    );

    // 必须附带趋势描述
    assert!(
        VLM_TIERED_PROMPT.contains("趋势"),
        "AC-1: 提示词必须要求附带趋势描述"
    );
}

/// TC-MM-007b：Level 3 数据图表误差标注（AC-3）。
///
/// AC-3：VLM 提取的数据图表结果末尾标注
/// 「以下数据由 AI 视觉提取，可能存在误差，请核对原文」提示。
#[test]
fn tc_mm_007_error_disclaimer() {
    assert!(
        !VLM_TIERED_PROMPT.is_empty(),
        "VLM_TIERED_PROMPT 不能为空（REQ-MM-005 未实现）"
    );

    assert!(
        VLM_TIERED_PROMPT.contains("AI 视觉提取"),
        "AC-3: 提示词必须包含「AI 视觉提取」误差提示"
    );
    assert!(
        VLM_TIERED_PROMPT.contains("可能存在误差"),
        "AC-3: 提示词必须包含「可能存在误差」提示"
    );
    assert!(
        VLM_TIERED_PROMPT.contains("请核对原文"),
        "AC-3: 提示词必须包含「请核对原文」提示"
    );
}

/// TC-MM-007c：数学公式 LaTeX 提取策略（AC-2）。
///
/// AC-2：含数学公式的图片经 VLM 处理后，chunk 中包含 LaTeX 格式的公式文本
/// （`$...$` 或 `$$...$$`），而非公式截图的 OCR 乱码。
#[test]
fn tc_mm_007_latex_formula_extraction() {
    assert!(
        !VLM_TIERED_PROMPT.is_empty(),
        "VLM_TIERED_PROMPT 不能为空（REQ-MM-005 未实现）"
    );

    assert!(
        VLM_TIERED_PROMPT.contains("公式"),
        "AC-2: 提示词必须提及数学公式内容类型"
    );
    assert!(
        VLM_TIERED_PROMPT.contains("LaTeX"),
        "AC-2: 提示词必须要求以 LaTeX 格式提取公式"
    );
    // 行内公式 $...$ 或块级公式 $$...$$ 至少出现一种引导
    assert!(
        VLM_TIERED_PROMPT.contains("$"),
        "AC-2: 提示词必须包含 LaTeX 定界符 $ 引导"
    );
}

/// TC-MM-007d：纯文字图片提取策略保留。
///
/// 原有功能不退化：纯文字图片仍直接提取文字内容。
#[test]
fn tc_mm_007_pure_text_image_extraction() {
    assert!(
        !VLM_TIERED_PROMPT.is_empty(),
        "VLM_TIERED_PROMPT 不能为空（REQ-MM-005 未实现）"
    );

    assert!(
        VLM_TIERED_PROMPT.contains("文字"),
        "提示词必须保留纯文字图片提取策略"
    );
}
