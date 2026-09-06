//! 对话模板/快捷指令系统数据契约。
//!
//! 对应 S56 自定义快捷指令模板。

use serde::{Deserialize, Serialize};

/// 自定义快捷指令模板（S56）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromptTemplate {
    pub id: String,
    pub name: String,
    pub label: String,
    pub description: String,
    pub icon: String,
    pub prompt_template: String,
    pub created_at: i64,
    pub updated_at: i64,
}

impl PromptTemplate {
    pub fn new(
        name: String,
        label: String,
        description: String,
        icon: String,
        prompt_template: String,
    ) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            label,
            description,
            icon,
            prompt_template,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn is_valid_name(name: &str) -> bool {
        let char_count = name.chars().count();
        !name.is_empty()
            && char_count <= 32
            && name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    }

    pub fn has_query_placeholder(template: &str) -> bool {
        template.contains("{query}")
    }
}
