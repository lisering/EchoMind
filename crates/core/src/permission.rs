//! Permission 细粒度控制（B11 Permission Rule Engine，REQ-ARCH-011）。
//!
//! 借鉴 OpenCode `permission.ts`：Wildcard 匹配的 RBAC 权限规则引擎。
//!
//! ## 设计
//!
//! - **纯函数**：`evaluate()` 和 `wildcard_match()` 是纯计算函数，零 I/O、零 async
//! - **三种效果**：`Allow`（允许）、`Deny`（拒绝）、`Ask`（询问用户）
//! - **通配符匹配**：`*` 匹配所有，`prefix.*` 匹配前缀
//! - **Last-writer-wins**：多条规则匹配时，最后一条规则的效果生效
//! - **默认拒绝**：无匹配规则时返回 `Deny`

use serde::{Deserialize, Serialize};

/// 权限效果（允许 / 拒绝 / 询问）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PermissionEffect {
    /// 允许执行
    Allow,
    /// 拒绝执行
    Deny,
    /// 询问用户确认（前端弹出确认对话框）
    Ask,
}

/// 权限规则（一条 RBAC 规则）。
///
/// 每条规则包含 action（如 `agent.search_kb`）、resource（如 `*` 或 `python`）、
/// 以及效果（Allow / Deny / Ask）。规则集按从后向前顺序评估，
/// 第一个匹配的规则效果生效（last-writer-wins）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PermissionRule {
    /// 操作标识（如 `agent.search_kb` / `agent.execute_code` / `*`）
    pub action: String,
    /// 资源标识（如 `*` / `python` / `bash`）
    pub resource: String,
    /// 规则效果（Allow / Deny / Ask）
    pub effect: PermissionEffect,
}

/// 权限规则集（`Vec<PermissionRule>` 的类型别名）。
pub type Ruleset = Vec<PermissionRule>;

/// 评估权限：在规则集中查找匹配的规则，返回效果。
///
/// 评估顺序为**从后向前**（last-writer-wins）：规则集中最后一条匹配的规则生效。
/// 如果没有匹配的规则，默认返回 `Deny`。
///
/// # 参数
/// - `action`: 要评估的操作（如 `agent.search_kb`）
/// - `resource`: 要评估的资源（如 `python`）
/// - `ruleset`: 权限规则集
///
/// # 返回
/// 匹配规则的效果，或 `Deny`（无匹配时）
///
/// # 示例
///
/// ```
/// use echomind_core::permission::{evaluate, PermissionEffect, PermissionRule, Ruleset};
///
/// let ruleset: Ruleset = vec![
///     PermissionRule {
///         action: "*".to_string(),
///         resource: "*".to_string(),
///         effect: PermissionEffect::Deny,
///     },
///     PermissionRule {
///         action: "agent.*".to_string(),
///         resource: "*".to_string(),
///         effect: PermissionEffect::Allow,
///     },
/// ];
///
/// // agent.* 规则在后面，生效
/// assert_eq!(evaluate("agent.search_kb", "kb", &ruleset), PermissionEffect::Allow);
/// // 无匹配规则
/// assert_eq!(evaluate("unknown.action", "kb", &ruleset), PermissionEffect::Deny);
/// ```
pub fn evaluate(action: &str, resource: &str, ruleset: &Ruleset) -> PermissionEffect {
    // 从后向前查找第一个匹配的规则（last-writer-wins）
    for rule in ruleset.iter().rev() {
        if wildcard_match(&rule.action, action) && wildcard_match(&rule.resource, resource) {
            return rule.effect.clone();
        }
    }
    // 默认拒绝
    PermissionEffect::Deny
}

/// 通配符匹配：`*` 匹配所有，`prefix.*` 匹配前缀。
///
/// 支持两种通配符模式：
/// - `*`：匹配任意字符串
/// - `prefix.*`：匹配以 `prefix.` 开头的任意字符串
///
/// # 参数
/// - `pattern`: 通配符模式（如 `*` / `agent.*` / `agent.search_kb`）
/// - `value`: 要匹配的值（如 `agent.search_kb`）
///
/// # 返回
/// `true` 表示匹配成功
///
/// # 示例
///
/// ```
/// use echomind_core::permission::wildcard_match;
///
/// assert!(wildcard_match("*", "anything"));
/// assert!(wildcard_match("agent.*", "agent.search_kb"));
/// assert!(wildcard_match("agent.*", "agent.execute_code"));
/// assert!(!wildcard_match("agent.*", "chat.send"));
/// assert!(wildcard_match("agent.search_kb", "agent.search_kb"));
/// assert!(!wildcard_match("agent.search_kb", "agent.execute_code"));
/// ```
pub fn wildcard_match(pattern: &str, value: &str) -> bool {
    // `*` 匹配所有
    if pattern == "*" {
        return true;
    }
    // `prefix.*` 前缀通配符匹配
    if let Some(prefix) = pattern.strip_suffix(".*") {
        return value.starts_with(prefix) && value[prefix.len()..].starts_with('.');
    }
    // 精确匹配
    pattern == value
}

// ============================================================================
// TDD 测试（TC-PERM-001~005，对应 REQ-ARCH-011 AC-1~AC-5）
// ============================================================================

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    /// TC-PERM-001：通配符 `*` 匹配所有 action 和 resource（AC-1）。
    #[test]
    fn tc_perm_001_wildcard_matches_all() {
        let ruleset: Ruleset = vec![PermissionRule {
            action: "*".to_string(),
            resource: "*".to_string(),
            effect: PermissionEffect::Allow,
        }];

        assert_eq!(
            evaluate("agent.search_kb", "kb", &ruleset),
            PermissionEffect::Allow
        );
        assert_eq!(
            evaluate("chat.send", "text", &ruleset),
            PermissionEffect::Allow
        );
        assert_eq!(
            evaluate("any.action", "any.resource", &ruleset),
            PermissionEffect::Allow
        );
    }

    /// TC-PERM-002：前缀通配符 `agent.*` 匹配 `agent.search_kb` / `agent.execute_code`（AC-2）。
    #[test]
    fn tc_perm_002_prefix_wildcard_matches() {
        let ruleset: Ruleset = vec![PermissionRule {
            action: "agent.*".to_string(),
            resource: "*".to_string(),
            effect: PermissionEffect::Allow,
        }];

        assert_eq!(
            evaluate("agent.search_kb", "kb", &ruleset),
            PermissionEffect::Allow
        );
        assert_eq!(
            evaluate("agent.execute_code", "python", &ruleset),
            PermissionEffect::Allow
        );
        // 不匹配非 agent 前缀的操作
        assert_eq!(
            evaluate("chat.send", "text", &ruleset),
            PermissionEffect::Deny
        );
    }

    /// TC-PERM-003：last-writer-wins — 多条规则匹配时，最后一条规则的效果生效（AC-3）。
    #[test]
    fn tc_perm_003_last_writer_wins() {
        let ruleset: Ruleset = vec![
            // 第一条：全局允许
            PermissionRule {
                action: "*".to_string(),
                resource: "*".to_string(),
                effect: PermissionEffect::Allow,
            },
            // 第二条：agent.execute_code 拒绝
            PermissionRule {
                action: "agent.*".to_string(),
                resource: "*".to_string(),
                effect: PermissionEffect::Deny,
            },
            // 第三条：agent.search_kb 允许（覆盖第二条）
            PermissionRule {
                action: "agent.search_kb".to_string(),
                resource: "*".to_string(),
                effect: PermissionEffect::Allow,
            },
        ];

        // agent.search_kb 匹配第三条（最后一条匹配的）→ Allow
        assert_eq!(
            evaluate("agent.search_kb", "kb", &ruleset),
            PermissionEffect::Allow
        );
        // agent.execute_code 匹配第二条（第三条不匹配）→ Deny
        assert_eq!(
            evaluate("agent.execute_code", "python", &ruleset),
            PermissionEffect::Deny
        );
        // chat.send 只匹配第一条 → Allow
        assert_eq!(
            evaluate("chat.send", "text", &ruleset),
            PermissionEffect::Allow
        );
    }

    /// TC-PERM-004：无匹配规则时默认返回 `Deny`（AC-4）。
    #[test]
    fn tc_perm_004_default_deny() {
        let ruleset: Ruleset = vec![PermissionRule {
            action: "agent.*".to_string(),
            resource: "python".to_string(),
            effect: PermissionEffect::Allow,
        }];

        // 不匹配的 action
        assert_eq!(
            evaluate("chat.send", "python", &ruleset),
            PermissionEffect::Deny
        );
        // 不匹配的 resource
        assert_eq!(
            evaluate("agent.search_kb", "bash", &ruleset),
            PermissionEffect::Deny
        );
        // 空规则集
        assert_eq!(
            evaluate("any.action", "any.resource", &vec![]),
            PermissionEffect::Deny
        );
    }

    /// TC-PERM-005：`Ask` 效果正确返回，供前端弹出用户确认对话框（AC-5）。
    #[test]
    fn tc_perm_005_ask_effect() {
        let ruleset: Ruleset = vec![PermissionRule {
            action: "agent.execute_code".to_string(),
            resource: "*".to_string(),
            effect: PermissionEffect::Ask,
        }];

        assert_eq!(
            evaluate("agent.execute_code", "python", &ruleset),
            PermissionEffect::Ask
        );
        assert_eq!(
            evaluate("agent.execute_code", "bash", &ruleset),
            PermissionEffect::Ask
        );
        // 其他操作不匹配 → Deny
        assert_eq!(
            evaluate("agent.search_kb", "kb", &ruleset),
            PermissionEffect::Deny
        );
    }
}
