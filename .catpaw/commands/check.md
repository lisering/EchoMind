---
description: 铁律检查（fmt + clippy + tsc + 禁止事项）
---

运行 EchoMind 项目铁律检查，确保代码符合 `.catpaw/rules/system-constraints.md` 的约束。

依次执行以下检查（任一失败即停止并修复）：

1. `cargo fmt --check` — 格式一致性
2. `cargo clippy --all-targets -- -D warnings` — Free Clippy 零警告
3. `npx tsc --noEmit` — TypeScript 类型检查零错误
4. `node scripts/build-ui.mjs` — 前端 esbuild 打包通过

全部通过后输出 "✓ 铁律检查全部通过"。

参数: $ARGUMENTS（可选，如果为 "fix" 则自动修复格式问题：`cargo fmt --all`；如果为 "pro" 则额外运行 Pro clippy：`cargo clippy --all-targets --features pro -- -D warnings`）

注意：铁律五禁止 `unwrap()/expect()/panic!()/unreachable!()/todo!()/unimplemented!()`，workspace deny lint 强制。
