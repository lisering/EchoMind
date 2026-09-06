# L3 质量门首次实跑报告（V3.1 阶段一）

> 日期：2026-08-24 ｜ 工具版本：cargo-mutants 27.1.0 / cargo-fuzz 0.13.2 / nightly miri
> 背景：铁律二「分层测试体系」中 L3（突变/UB/模糊）此前从未实际执行，本次为首轮摸底。

## 一、突变测试（cargo-mutants）

**目标**：`crates/core/src/encryption.rs`（安全关键——加密/暴力破解防护/密码强度，铁律要求检出率 ≥90%）
**规模**：94 mutants（57 caught / 31 missed / 6 unviable → 三轮迭代后 **82 caught / 6 missed，检出率 93.2% ✅**）

### 发现与修复

| 轮次 | missed | 新增测试 |
|---|---|---|
| 首轮 | 31 | —（基线） |
| 第二轮 | 22 | BruteForce 精确计数/剩余次数递减/reset 清锁/锁定时长区间 5 个行为断言 |
| 第三轮 | 22→9 | remaining_lock_seconds 下界 ≥25s（杀灭 `/` 突变）；PasswordStrength 全分档边界 + 展示属性结构约束 |
| 第四轮 | 9→6 | suggestions 每条建议的精确触发条件断言 |

**剩余 6 个均为等价/不可达突变**（数学上不可杀灭，已记录）：
- `is_locked` 的 `<`→`<=` 与 `remaining_lock_seconds` 的 `>`→`>=`：时间戳同一秒内语义相同
- 3 个 match arm 删除：前置 guard（弱密码字典/长度/单一类型检查）短路后的防御性兜底分支，逻辑不可达
- ~~`color → "xyzzy"`~~ 已补 `#` 前缀断言杀灭（最终轮）

### 结论
安全关键模块的既有测试只覆盖 happy path——**突变测试找到了 25 个真实断言缺口并全部补齐**。该模式应推广至 hybrid_retriever / license 等模块（下轮）。

## 二、UB 检测（cargo miri）

- 安装 nightly + miri 成功；发现 weight_repack（Q8_0/Q4_0/Q4K/Q8K 量化内核指针读取）**零测试覆盖**
- 新增 `weight_repack_safety_tests`（3 测试）：单块手工点积对照 / 多行多块 offset 计算 / k 非整数倍拒绝——普通模式下全过
- **平台限制**：aarch64 macOS 下 `half` crate 的 f16 转换使用内联汇编，miri 不支持 → 本地无法完整跑
- **落地方式**：`.github/workflows/quality-gate.yml` 每周在 x86_64 Linux 执行（half 走 intrinsics 路径，miri 兼容）

## 三、模糊测试（cargo-fuzz）

新建 `fuzz/` 基建 + 2 个 target：

| Target | Smoke 结果 |
|---|---|
| normalize_query_fuzz | 193,304 runs，零 crash ✅ |
| semantic_splitter_fuzz | **首跑即抓到 crash！** |

### Crash 根因分析（fuzz artifact crash-f4203398）

输入特征：控制字符密集序列（`\x1b\x0b\x09\x00`），lossy 转换后 BPE 对控制字符逐个产 token 使段落超窗，走 recursive 分割链。

**根因**：`merge_pieces_static` / `split_recursive_static` 只做 `is_empty()` 长度检查，纯空白块（如 `"\n"`）穿透至输出——下游会为空块计算嵌入浪费算力甚至污染索引。

**修复**：`SemanticSplitter::split` 唯一出口统一 `retain(|c| !c.trim().is_empty())`（最小完备收口，避免逐层堵漏）。
**验证**：TDD 回归测试（内嵌 crash 语料 base64）红灯→绿灯；既有 23 个 semantic 测试无回归；重跑 fuzz 213,631 runs 零 crash。

## 四、Flaky 根治补充

混沌全量实证（`--include-ignored`，1732 passed 0 failed 两轮）：
- `tc_perf_003`：阈值 0.99 与 fastembed Dynamic 量化物理特性矛盾（batch 校准容差，姊妹测试 TC-PERF-004 已有 0.98 先例）→ 统一 0.98 并注明依据
- tc_nfr_002 / tc_obs_007-008：前序已修（抗抖动重试 + 互斥锁），本次实证通过
- E2E 重试策略：本地 retries=0（不掩盖）+ CI `PLAYWRIGHT_RETRIES=1`（收敛环境抖动）

## 五、后续建议

1. mutants 推广：hybrid_retriever.rs / license.rs / sqlite_storage 加密路径（每模块 ~20 分钟）
2. fuzz 目标扩展：markdown/csv/html loader 的文本提取函数（需先解耦 IO）
3. quality-gate.yml 合入后观察首个每周 cron 的稳定性，再考虑提升频率
