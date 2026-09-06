# EchoMind 文档归档索引

> 归档日期：2026-09-07  
> 归档原因：文档已过时、已完成、或被新版本替代

---

## 📁 目录结构

```
docs/archive/
├── reports/        # 历史测试/验收/性能报告
├── planning/       # 已完成/被替代的规划文档
├── beta/           # Beta 阶段招募与测试计划（草案）
├── reviews/        # AI 代码审查集成完成报告
├── session-notes/  # 会话交接记录（已有 sessions/archive/）
├── misc/           # 其他过时文档
```

---

## 📋 归档清单

### reports/ — 历史报告
| 文件 | 原路径 | 归档理由 |
|------|--------|----------|
| `TEST_REPORT_2026-07-29.md` | `docs/` | 早期全面测试报告，已被后续验收替代 |
| `ACCEPTANCE_REPORT_2026-08-03.md` | `docs/` | v1.0 发布前验收报告，已完成 |
| `ROOT_CAUSE_ANALYSIS_2026-08-02.md` | `docs/` | 根因分析报告，问题已修复 |
| `PERFORMANCE_PROGRESS.md` | `docs/` | 性能优化进度交接，任务已完成 |
| `QUALITY_GATE_FIRST_RUN.md` | `docs/` | L3 质量门首轮实跑，已常态化 |
| `PERFORMANCE_BENCHMARK_REPORT.md` | `docs/` | 早期性能基准，数据已过时 |
| `COMPREHENSIVE_TEST_REPORT.md` | 根目录 | 全面 UX 挑刺测试报告，已完成 |

### planning/ — 已完成规划
| 文件 | 原路径 | 归档理由 |
|------|--------|----------|
| `v1.0-release-checklist.md` | `docs/` | v1.0 发布检查清单草案，已被 `v1.0-release-checklist-final.md` 替代 |
| `TEST_SYSTEM_REDESIGN.md` | `docs/` | 测试体系重设计方案，已实施完成 |
| `FUZZ_TESTING_PLAN.md` | `docs/` | 模糊测试计划，已集成到 CI |

### beta/ — Beta 阶段文档（草案）
| 文件 | 原路径 | 归档理由 |
|------|--------|----------|
| `BETA_RECRUITMENT.md` | `docs/` | Beta 招募材料模板，未执行 |
| `BETA_TEST_PLAN.md` | `docs/` | Beta 测试计划草案，未执行 |

### reviews/ — AI 审查集成（已完成）
| 文件 | 原路径 | 归档理由 |
|------|--------|----------|
| `ai-review-complete-implementation.md` | `docs/` | AI 审查完整实现报告 |
| `ai-review-implementation-report.md` | `docs/` | AI 审查实现最终报告 |
| `ai-review-final-summary.md` | `docs/` | AI 审查最终总结 |
| `ai-review-implementation-summary.md` | `docs/` | AI 审查实现摘要 |
| `ai-review-guide.md` | `docs/` | AI 审查使用指南 |

### misc/ — 其他过时文档
| 文件 | 原路径 | 归档理由 |
|------|--------|----------|
| `TEST_CASES_BETA.csv` | `docs/` | Beta 测试用例 CSV，未使用 |
| `RS_PRO_COMPARISON.md` | `docs/` | Rust vs Pro 对比分析，早期调研 |
| `MONETIZATION_ANALYSIS.md` | `docs/` | 商业化分析，早期调研 |
| `OBSIDIAN_MODEL_PLAN.md` | `docs/` | Obsidian 模型规划，早期调研 |
| `FIGMA_DESIGN_SPEC.md` | `docs/` | Figma 设计规格，早期设计 |
| `PROJECT_ASSESSMENT_2026-08-24.md` | `docs/` | 项目评估报告，已过时 |
| `E2E_COVERAGE.md` | `docs/` | E2E 覆盖矩阵，已整合到 UI_ACCEPTANCE.md |
| `CHAT_LATENCY_BENCH_GUIDE.md` | `docs/` | 聊天延迟基准指南，已整合 |
| `FEEDBACK_ANALYSIS_FRAMEWORK.md` | `docs/` | 反馈分析框架，早期草案 |

---

## 🔗 现行核心文档（未归档）

以下文档为当前维护的核心文档，**未归档**：

| 文件 | 说明 |
|------|------|
| `docs/SRS_v1.0.md` | 唯一需求真相源 |
| `docs/SOFTWARE_ASSET.md` | 软件资产清单 |
| `docs/UI_ACCEPTANCE.md` | UI 验收测试体系（当前版本 v1.3.0） |
| `docs/TEST_STRATEGY.md` | 测试策略 |
| `docs/dev-guide/` | 开发指南系列 |
| `docs/api/` | API 文档 |
| `docs/architecture/` | 架构文档 |
| `docs/security/` | 安全文档 |
| `docs/refactoring/` | 重构记录 |
| `docs/changelog/` | 变更日志 |
| `docs/sessions/CURRENT_STATE.md` | 当前会话状态快照 |
| `docs/sessions/LOG.md` | 会话日志 |
| `docs/sessions/HISTORY.md` | 会话历史 |
| `docs/sessions/README.md` | 会话目录说明 |
| `docs/sessions/NEXT_SESSION.md` | 下一会话提示词 |

---

## 📝 备注

- `docs/sessions/archive/` 目录已存在，存放历史会话记录，未在此重复归档
- 归档文档仅作历史参考，**不再维护**，如需查阅当前规范请参考上述核心文档
- 若需恢复某文档，请从归档目录复制回原位并更新内容