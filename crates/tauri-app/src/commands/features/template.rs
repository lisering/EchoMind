//! 自定义快捷指令模板管理（S56）。
use super::super::*;

/// 创建或更新自定义快捷指令模板（S56）。
///
/// # 参数
/// - `name` — 指令名（不含 `/`，仅小写字母/数字/下划线，1-32 字符）
/// - `label` — 显示标签
/// - `description` — 描述说明
/// - `icon` — 图标 emoji
/// - `prompt_template` — Prompt 模板内容（必须包含 `{query}` 占位符）
///
/// # 返回
/// 模板 ID（UUID v4）
///
/// # 错误
/// - 名称不合法（空/含非法字符/超长）
/// - 模板内容缺少 `{query}` 占位符
/// - 名称与系统内置指令冲突（summary/compare/extract/translate/timeline/mindmap）
/// - settings 表写入失败
#[tauri::command]
pub async fn save_prompt_template(
    name: String,
    label: String,
    description: String,
    icon: String,
    prompt_template: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    save_prompt_template_inner(
        &name,
        &label,
        &description,
        &icon,
        &prompt_template,
        state.inner(),
    )
    .await
}

/// 模板保存逻辑（命令与集成测试复用）。
pub async fn save_prompt_template_inner(
    name: &str,
    label: &str,
    description: &str,
    icon: &str,
    prompt_template: &str,
    state: &AppState,
) -> Result<String, String> {
    // 验证名称合法性
    if !PromptTemplate::is_valid_name(name) {
        return Err(prefix_error(
            ERR_VALIDATION,
            "指令名称不合法（仅小写字母/数字/下划线，1-32 字符）",
        ));
    }

    // 验证不与系统内置指令冲突
    const SYSTEM_COMMANDS: &[&str] = &[
        "summary",
        "compare",
        "extract",
        "translate",
        "timeline",
        "mindmap",
    ];
    if SYSTEM_COMMANDS.contains(&name) {
        return Err(prefix_error(
            ERR_VALIDATION,
            &format!("指令名称 '{name}' 与系统内置指令冲突"),
        ));
    }

    // 验证模板内容包含 {query} 占位符
    if !PromptTemplate::has_query_placeholder(prompt_template) {
        return Err(prefix_error(
            ERR_VALIDATION,
            "模板内容必须包含 {query} 占位符",
        ));
    }

    // 检查名称是否已被使用（更新已有模板时允许同名）
    let existing_templates = list_prompt_templates_inner(state).await.unwrap_or_default();
    let existing = existing_templates.iter().find(|t| t.name == name);

    let template = if let Some(existing) = existing {
        // 更新已有模板（保留 ID 和 created_at）
        let mut updated = existing.clone();
        updated.label = label.to_string();
        updated.description = description.to_string();
        updated.icon = icon.to_string();
        updated.prompt_template = prompt_template.to_string();
        updated.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        updated
    } else {
        // 创建新模板
        PromptTemplate::new(
            name.to_string(),
            label.to_string(),
            description.to_string(),
            icon.to_string(),
            prompt_template.to_string(),
        )
    };

    let template_id = template.id.clone();

    // 序列化为 JSON 并存储
    let json = serde_json::to_string(&template)
        .map_err(|e| prefix_error(ERR_PARSE, &format!("模板序列化失败: {e}")))?;

    state
        .storage
        .set_setting(&format!("{PROMPT_TEMPLATE_KEY_PREFIX}{template_id}"), &json)
        .await
        .map_err(|e| prefix_error(ERR_STORAGE, &format!("{e:#}")))?;

    // 更新索引（新模板才需要添加到索引）
    if existing.is_none() {
        update_prompt_template_index(state, &template_id, true)
            .await
            .map_err(|e| prefix_error(ERR_STORAGE, &format!("模板索引更新失败: {e:#}")))?;
    }

    Ok(template_id)
}

/// 列出所有自定义快捷指令模板（S56）。
///
/// 返回 `PromptTemplate` 列表，按 `name` 字母序排列。
#[tauri::command]
pub async fn list_prompt_templates(
    state: State<'_, AppState>,
) -> Result<Vec<PromptTemplate>, String> {
    list_prompt_templates_inner(state.inner()).await
}

/// 模板列表查询逻辑（命令与集成测试复用）。
pub async fn list_prompt_templates_inner(state: &AppState) -> Result<Vec<PromptTemplate>, String> {
    // 读取索引
    let index_json = state
        .storage
        .get_setting(PROMPT_TEMPLATE_INDEX_KEY)
        .await
        .map_err(|e| prefix_error(ERR_STORAGE, &format!("{e:#}")))?;

    let template_ids: Vec<String> = match index_json {
        Some(json) => serde_json::from_str(&json)
            .map_err(|e| prefix_error(ERR_PARSE, &format!("模板索引解析失败: {e}")))?,
        None => Vec::new(),
    };

    // 逐个读取模板
    let mut templates = Vec::new();
    for id in &template_ids {
        if let Ok(Some(json)) = state
            .storage
            .get_setting(&format!("{PROMPT_TEMPLATE_KEY_PREFIX}{id}"))
            .await
            && let Ok(tmpl) = serde_json::from_str::<PromptTemplate>(&json)
        {
            templates.push(tmpl);
        }
    }

    // 按名称排序
    templates.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(templates)
}

/// 删除自定义快捷指令模板（S56）。
///
/// 从 settings 表删除 `prompt_template.{id}` 键，并从索引中移除。
///
/// # 参数
/// - `template_id` — 要删除的模板 ID
///
/// # 错误
/// - settings 表操作失败
#[tauri::command]
pub async fn delete_prompt_template(
    template_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    delete_prompt_template_inner(&template_id, state.inner()).await
}

/// 模板删除逻辑（命令与集成测试复用）。
pub async fn delete_prompt_template_inner(
    template_id: &str,
    state: &AppState,
) -> Result<(), String> {
    // 删除模板定义
    state
        .storage
        .set_setting(&format!("{PROMPT_TEMPLATE_KEY_PREFIX}{template_id}"), "")
        .await
        .map_err(|e| prefix_error(ERR_STORAGE, &format!("{e:#}")))?;

    // 从索引中移除
    update_prompt_template_index(state, template_id, false)
        .await
        .map_err(|e| prefix_error(ERR_STORAGE, &format!("模板索引更新失败: {e:#}")))?;

    Ok(())
}

/// 更新模板索引（添加或移除模板 ID）。
///
/// `add = true` → 添加 ID 到索引；`add = false` → 从索引移除 ID。
async fn update_prompt_template_index(
    state: &AppState,
    template_id: &str,
    add: bool,
) -> anyhow::Result<()> {
    let index_json = state.storage.get_setting(PROMPT_TEMPLATE_INDEX_KEY).await?;

    let mut ids: Vec<String> = match index_json {
        Some(json) => serde_json::from_str(&json).unwrap_or_default(),
        None => Vec::new(),
    };

    if add {
        if !ids.contains(&template_id.to_string()) {
            ids.push(template_id.to_string());
        }
    } else {
        ids.retain(|id| id != template_id);
    }

    let new_index = serde_json::to_string(&ids)?;

    state
        .storage
        .set_setting(PROMPT_TEMPLATE_INDEX_KEY, &new_index)
        .await?;

    Ok(())
}
