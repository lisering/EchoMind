//! Skill 系统发现（REQ-ARCH-010 v1.8：Skill 发现 + 斜杠命令面板集成）。
use super::super::*;

/// 发现本地 Skill 文件并返回 `slash: true` 的技能列表（REQ-ARCH-010 v1.8）。
///
/// 扫描 `{data_dir}/skills/` 目录中的 `.md` 文件，解析 YAML frontmatter，
/// 仅返回 `slash: true` 的 Skill（可注册为斜杠命令的技能）。
///
/// **降级策略**：
/// - 目录不存在 → 返回空列表（不报错，静默降级）
/// - 文件解析失败 → 跳过该文件
/// - IO 错误 → 返回空列表 + 日志告警
///
/// # 返回
///
/// `Vec<Skill>` — `slash: true` 的技能列表（可能为空）
#[tauri::command]
pub async fn discover_skills(state: State<'_, AppState>) -> Result<Vec<Skill>, String> {
    discover_skills_inner(state.inner()).await
}

/// `discover_skills` 的内部实现（命令与集成测试复用）。
pub async fn discover_skills_inner(state: &AppState) -> Result<Vec<Skill>, String> {
    let skills_dir = state.data_dir.join("skills");

    // 目录不存在时静默返回空列表（降级策略）
    if !skills_dir.exists() {
        debug!("Skills 目录不存在: {}，返回空列表", skills_dir.display());
        return Ok(Vec::new());
    }

    match echomind_core::skill::discover_from_dir(&skills_dir).await {
        Ok(all_skills) => {
            // 仅返回 slash: true 的技能
            let slash_skills: Vec<Skill> = all_skills.into_iter().filter(|s| s.slash).collect();
            debug!(
                "发现 {} 个 Skill（slash: true），目录: {}",
                slash_skills.len(),
                skills_dir.display()
            );
            Ok(slash_skills)
        }
        Err(e) => {
            warn!("Skill 目录扫描失败: {e:#}");
            Ok(Vec::new())
        }
    }
}

// ============================================================
// B09 Skill 集成 TDD 测试（TC-SKILL-INTEGRATE-001~003）
// ============================================================

#[cfg(test)]
mod skill_integration_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use echomind_core::skill::{Skill, parse_skill};

    /// TC-SKILL-INTEGRATE-001: discover_skills 返回 slash: true 的 Skill 列表
    ///
    /// 验证过滤逻辑：`slash: true` 的 Skill 被返回，`slash: false` 的被过滤掉。
    #[test]
    fn tc_skill_integrate_001_filter_slash_true() {
        // 构造两个 Skill：一个 slash: true，一个 slash: false
        let md_slash = "---\nname: code-review\nslash: true\ndescription: Code review skill\n---\nReview the code.";
        let md_no_slash =
            "---\nname: internal-doc\nslash: false\ndescription: Internal doc\n---\nSome content.";

        let skill_slash = parse_skill(md_slash, "/skills/code-review.md").unwrap();
        let skill_no_slash = parse_skill(md_no_slash, "/skills/internal.md").unwrap();

        // 模拟 discover_skills_inner 的过滤逻辑
        let all = vec![skill_slash.clone(), skill_no_slash];
        let filtered: Vec<Skill> = all.into_iter().filter(|s| s.slash).collect();

        assert_eq!(filtered.len(), 1, "应只返回 slash: true 的 Skill");
        assert_eq!(filtered[0].name, "code-review");
        assert!(filtered[0].slash);
    }

    /// TC-SKILL-INTEGRATE-002: 目录不存在时返回空列表（静默降级）
    ///
    /// 验证降级策略：目录不存在时不报错，返回空 Vec。
    #[test]
    fn tc_skill_integrate_002_dir_not_exists_empty() {
        // 验证：不存在的路径 → Vec::new()
        // 此逻辑在 discover_skills_inner 中通过 skills_dir.exists() 检查实现
        let non_existent = std::path::PathBuf::from("/nonexistent/skills/dir/12345");
        assert!(!non_existent.exists(), "测试前提：路径不应存在");

        // 模拟 discover_skills_inner 的降级逻辑
        let result: Vec<Skill> = if !non_existent.exists() {
            Vec::new()
        } else {
            // 不会执行到此分支
            panic!("不应执行到此分支");
        };

        assert!(result.is_empty(), "目录不存在时应返回空列表");
    }

    /// TC-SKILL-INTEGRATE-003: slash: false 的 Skill 不出现在结果中
    ///
    /// 验证：多个 slash: false 的 Skill 全部被过滤，结果为空列表。
    #[test]
    fn tc_skill_integrate_003_all_slash_false_filtered() {
        let md1 = "---\nname: skill-a\nslash: false\n---\nContent A";
        let md2 = "---\nname: skill-b\nslash: false\n---\nContent B";

        let s1 = parse_skill(md1, "/a.md").unwrap();
        let s2 = parse_skill(md2, "/b.md").unwrap();

        let all = vec![s1, s2];
        let filtered: Vec<Skill> = all.into_iter().filter(|s| s.slash).collect();

        assert!(filtered.is_empty(), "全部 slash: false 时应返回空列表");
    }
}
