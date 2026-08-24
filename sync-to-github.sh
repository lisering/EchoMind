#!/usr/bin/env bash
#
# sync-to-github.sh — 将开发目录的源代码同步到 EchoMind/EchoMind 发布仓库
#
# 用法：在项目根目录 /Users/john/freesoft/EchoMind/ 下执行
#   ./sync-to-github.sh           # 同步并显示 diff
#   ./sync-to-github.sh --commit  # 同步 + 自动提交
#   ./sync-to-github.sh --push    # 同步 + 自动提交 + 推送到 GitHub
#
# 发布仓库专属文件（不从开发目录同步，在发布仓库中独立维护）：
#   .github/              CI/CD workflows + Issue/PR 模板
#   .coderabbit.yaml      CodeRabbit AI 审查配置
#   CONTRIBUTING.md       贡献指南
#   README.md             英文 README
#   README.zh-CN.md       中文 README
#

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"
RELEASE_DIR="$ROOT_DIR/EchoMind"

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}  EchoMind → GitHub 发布仓库同步${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""

# 检查发布目录存在
if [ ! -d "$RELEASE_DIR/.git" ]; then
  echo -e "${RED}错误：发布仓库 $RELEASE_DIR 不存在或不是 Git 仓库${NC}"
  exit 1
fi

# 同步函数
sync_file() {
  local src="$1"
  local dst="$2"
  if [ -e "$src" ]; then
    mkdir -p "$(dirname "$dst")"
    cp -f "$src" "$dst"
    echo -e "  ${GREEN}✓${NC} $src"
  else
    echo -e "  ${YELLOW}⚠ 跳过（不存在）：$src${NC}"
  fi
}

sync_dir() {
  local src="$1"
  local dst="$2"
  if [ -d "$src" ]; then
    mkdir -p "$dst"
    rsync -a --delete "$src/" "$dst/"
    echo -e "  ${GREEN}✓${NC} $src/ → $dst/"
  else
    echo -e "  ${YELLOW}⚠ 跳过（不存在）：$src/${NC}"
  fi
}

# ─── [1/10] 根配置文件 ───
echo -e "${BLUE}[1/10] 同步根配置文件${NC}"
sync_file "$ROOT_DIR/Cargo.toml"           "$RELEASE_DIR/Cargo.toml"
sync_file "$ROOT_DIR/Cargo.lock"            "$RELEASE_DIR/Cargo.lock"
sync_file "$ROOT_DIR/deny.toml"             "$RELEASE_DIR/deny.toml"
sync_file "$ROOT_DIR/.gitignore"            "$RELEASE_DIR/.gitignore"
sync_file "$ROOT_DIR/LICENSE"               "$RELEASE_DIR/LICENSE"
sync_file "$ROOT_DIR/package.json"          "$RELEASE_DIR/package.json"
sync_file "$ROOT_DIR/package-lock.json"     "$RELEASE_DIR/package-lock.json"
sync_file "$ROOT_DIR/tsconfig.json"         "$RELEASE_DIR/tsconfig.json"
sync_file "$ROOT_DIR/tailwind.config.js"    "$RELEASE_DIR/tailwind.config.js"
sync_dir "$ROOT_DIR/types"                  "$RELEASE_DIR/types"
sync_dir "$ROOT_DIR/scripts"                "$RELEASE_DIR/scripts"

# 从发布仓库的 Cargo.toml 中移除 license-issuer 成员（私有工具，不公开）
if [ -f "$RELEASE_DIR/Cargo.toml" ]; then
  sed -i '' '/"crates\/license-issuer"/d' "$RELEASE_DIR/Cargo.toml"
  echo -e "  ${GREEN}✓${NC} Cargo.toml (已移除 license-issuer 成员)"
fi

# ─── [2/10] crates/models ───
echo ""
echo -e "${BLUE}[2/10] 同步 crates/models${NC}"
sync_dir "$ROOT_DIR/crates/models/src"      "$RELEASE_DIR/crates/models/src"
sync_file "$ROOT_DIR/crates/models/Cargo.toml" "$RELEASE_DIR/crates/models/Cargo.toml"

# ─── [3/10] crates/prompt ───
echo ""
echo -e "${BLUE}[3/10] 同步 crates/prompt${NC}"
sync_dir "$ROOT_DIR/crates/prompt/src"      "$RELEASE_DIR/crates/prompt/src"
sync_file "$ROOT_DIR/crates/prompt/Cargo.toml" "$RELEASE_DIR/crates/prompt/Cargo.toml"

# ─── [3.5/10] crates/context（V3.0 新增，workspace 成员）───
echo ""
echo -e "${BLUE}[3.5/10] 同步 crates/context${NC}"
sync_dir "$ROOT_DIR/crates/context/src"      "$RELEASE_DIR/crates/context/src"
sync_file "$ROOT_DIR/crates/context/Cargo.toml" "$RELEASE_DIR/crates/context/Cargo.toml"

# ─── [4/10] crates/compact ───
echo ""
echo -e "${BLUE}[4/10] 同步 crates/compact${NC}"
sync_dir "$ROOT_DIR/crates/compact/src"     "$RELEASE_DIR/crates/compact/src"
sync_file "$ROOT_DIR/crates/compact/Cargo.toml" "$RELEASE_DIR/crates/compact/Cargo.toml"

# ─── [5/10] crates/core ───
echo ""
echo -e "${BLUE}[5/10] 同步 crates/core${NC}"
sync_dir "$ROOT_DIR/crates/core/src"        "$RELEASE_DIR/crates/core/src"
sync_file "$ROOT_DIR/crates/core/Cargo.toml" "$RELEASE_DIR/crates/core/Cargo.toml"

# ─── [6/10] crates/infra ───
echo ""
echo -e "${BLUE}[6/10] 同步 crates/infra${NC}"
sync_dir "$ROOT_DIR/crates/infra/src"       "$RELEASE_DIR/crates/infra/src"
sync_file "$ROOT_DIR/crates/infra/Cargo.toml" "$RELEASE_DIR/crates/infra/Cargo.toml"

# ─── [7/10] crates/tauri-app ───
echo ""
echo -e "${BLUE}[7/10] 同步 crates/tauri-app${NC}"
sync_dir "$ROOT_DIR/crates/tauri-app/src"   "$RELEASE_DIR/crates/tauri-app/src"
sync_dir "$ROOT_DIR/crates/tauri-app/capabilities" "$RELEASE_DIR/crates/tauri-app/capabilities"
sync_dir "$ROOT_DIR/crates/tauri-app/icons" "$RELEASE_DIR/crates/tauri-app/icons"
sync_dir "$ROOT_DIR/crates/tauri-app/tests" "$RELEASE_DIR/crates/tauri-app/tests"
# ─── [7.5/10] e2e-tests（CI frontend-checks 的 Vitest 与 E2E 依赖）───
echo ""
echo -e "${BLUE}[7.5/10] 同步 e2e-tests${NC}"
sync_file "$ROOT_DIR/e2e-tests/package.json"      "$RELEASE_DIR/e2e-tests/package.json"
sync_file "$ROOT_DIR/e2e-tests/package-lock.json" "$RELEASE_DIR/e2e-tests/package-lock.json"
sync_file "$ROOT_DIR/e2e-tests/playwright.config.ts" "$RELEASE_DIR/e2e-tests/playwright.config.ts"
sync_file "$ROOT_DIR/e2e-tests/vitest.config.ts"  "$RELEASE_DIR/e2e-tests/vitest.config.ts"
sync_file "$ROOT_DIR/e2e-tests/wdio.conf.mjs"     "$RELEASE_DIR/e2e-tests/wdio.conf.mjs" 2>/dev/null || true
sync_dir "$ROOT_DIR/e2e-tests/tests"              "$RELEASE_DIR/e2e-tests/tests"
sync_dir "$ROOT_DIR/e2e-tests/scripts"            "$RELEASE_DIR/e2e-tests/scripts"

# ─── [7.6/10] tests/（混沌测试语料生成器 + loader fixtures）───
echo ""
echo -e "${BLUE}[7.6/10] 同步 tests/${NC}"
sync_dir "$ROOT_DIR/tests"                   "$RELEASE_DIR/tests"

sync_file "$ROOT_DIR/crates/tauri-app/Cargo.toml" "$RELEASE_DIR/crates/tauri-app/Cargo.toml"
sync_file "$ROOT_DIR/crates/tauri-app/build.rs" "$RELEASE_DIR/crates/tauri-app/build.rs"
sync_file "$ROOT_DIR/crates/tauri-app/tauri.conf.json" "$RELEASE_DIR/crates/tauri-app/tauri.conf.json"

# ─── [8/10] license-issuer（跳过） ───
echo ""
echo -e "${BLUE}[8/10] 同步 crates/license-issuer${NC}"
echo -e "  ${YELLOW}⚠ 跳过（license-issuer 为私有工具，不公开到 GitHub）${NC}"

# ─── [9/10] ui/ ───
echo ""
echo -e "${BLUE}[9/10] 同步 ui/${NC}"
sync_file "$ROOT_DIR/ui/index.html"         "$RELEASE_DIR/ui/index.html"
sync_file "$ROOT_DIR/ui/logo.png"           "$RELEASE_DIR/ui/logo.png"
sync_file "$ROOT_DIR/ui/logo.svg"           "$RELEASE_DIR/ui/logo.svg"
sync_dir "$ROOT_DIR/ui/src"                "$RELEASE_DIR/ui/src"
sync_dir "$ROOT_DIR/ui/locales"             "$RELEASE_DIR/ui/locales"
sync_dir "$ROOT_DIR/ui/styles"              "$RELEASE_DIR/ui/styles"
sync_dir "$ROOT_DIR/ui/vendor"              "$RELEASE_DIR/ui/vendor"

# ─── [9.5/10] .github（CI workflows + 模板）───
echo ""
echo -e "${BLUE}[9.5/10] 同步 .github${NC}"
sync_dir "$ROOT_DIR/.github/workflows"      "$RELEASE_DIR/.github/workflows"
sync_file "$ROOT_DIR/.github/dependabot.yml" "$RELEASE_DIR/.github/dependabot.yml"
if [ -d "$ROOT_DIR/.github/ISSUE_TEMPLATE" ]; then
  sync_dir "$ROOT_DIR/.github/ISSUE_TEMPLATE" "$RELEASE_DIR/.github/ISSUE_TEMPLATE"
fi
if [ -f "$ROOT_DIR/.github/pull_request_template.md" ]; then
  sync_file "$ROOT_DIR/.github/pull_request_template.md" "$RELEASE_DIR/.github/pull_request_template.md"
fi

# ─── [10/10] 清理构建生成物 ───
echo ""
echo -e "${BLUE}[10/10] 清理构建生成物${NC}"
if [ -d "$RELEASE_DIR/crates/tauri-app/gen" ]; then
  rm -rf "$RELEASE_DIR/crates/tauri-app/gen"
  echo -e "  ${GREEN}✓${NC} 已删除 crates/tauri-app/gen/（Tauri 构建自动生成）"
else
  echo -e "  ${GREEN}✓${NC} 无需清理"
fi
if [ -d "$RELEASE_DIR/target" ]; then
  rm -rf "$RELEASE_DIR/target"
  echo -e "  ${GREEN}✓${NC} 已删除 target/（编译产物）"
fi
# 删除 .DS_Store
find "$RELEASE_DIR" -name '.DS_Store' -delete 2>/dev/null || true
echo -e "  ${GREEN}✓${NC} 已清理 .DS_Store"

# ─── 发布仓库专属文件提示 ───
echo ""
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}  发布仓库专属文件（不从开发目录同步）：${NC}"
echo -e "  ${GREEN}✓${NC} .github/workflows/ci.yml         — CI 构建+测试+lint"
echo -e "  ${GREEN}✓${NC} .github/workflows/release.yml     — 跨平台安装包构建"
echo -e "  ${GREEN}✓${NC} .github/workflows/ai-review.yml   — Claude AI 代码审查"
echo -e "  ${GREEN}✓${NC} .github/dependabot.yml            — 依赖自动更新"
echo -e "  ${GREEN}✓${NC} .github/pull_request_template.md  — PR 模板"
echo -e "  ${GREEN}✓${NC} .github/ISSUE_TEMPLATE/           — Issue 模板"
echo -e "  ${GREEN}✓${NC} .coderabbit.yaml                  — CodeRabbit AI 审查配置"
echo -e "  ${GREEN}✓${NC} CONTRIBUTING.md                   — 贡献指南"
echo -e "  ${GREEN}✓${NC} README.md / README.zh-CN.md       — 双语 README"
echo -e "  ${GREEN}✓${NC} LICENSE                           — BUSL-1.1 许可证"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""

# 进入发布仓库检查状态
cd "$RELEASE_DIR"

# 检查是否有变更
if git diff --quiet && git diff --cached --quiet && [ -z "$(git ls-files --others --exclude-standard)" ]; then
  echo -e "${GREEN}✓ 发布仓库已是最新，无变更。${NC}"
  exit 0
fi

echo -e "${YELLOW}发布仓库有变更：${NC}"
echo ""

# 显示变更摘要
CHANGED=$(git diff --name-only HEAD 2>/dev/null || true)
UNTRACKED=$(git ls-files --others --exclude-standard)
DELETED=$(git ls-files --deleted 2>/dev/null || true)

if [ -n "$CHANGED" ]; then
  echo -e "${YELLOW}已修改文件：${NC}"
  echo "$CHANGED" | sed 's/^/  /'
  echo ""
fi

if [ -n "$UNTRACKED" ]; then
  echo -e "${YELLOW}新增文件：${NC}"
  echo "$UNTRACKED" | sed 's/^/  /'
  echo ""
fi

if [ -n "$DELETED" ]; then
  echo -e "${YELLOW}已删除文件：${NC}"
  echo "$DELETED" | sed 's/^/  /'
  echo ""
fi

# 处理 --commit / --push 参数
ACTION="${1:-}"

if [ "$ACTION" = "--commit" ] || [ "$ACTION" = "--push" ]; then
  echo -e "${BLUE}自动提交中...${NC}"
  git add -A

  COMMIT_MSG="chore: sync from dev workspace ($(date '+%Y-%m-%d'))

Files: $(echo "$CHANGED$UNTRACKED$DELETED" | grep -c .) changed"

  git commit -m "$COMMIT_MSG"
  echo ""
  echo -e "${GREEN}✓ 已提交到发布仓库。${NC}"

  if [ "$ACTION" = "--push" ]; then
    echo -e "${BLUE}推送到 GitHub...${NC}"
    git push
    echo ""
    echo -e "${GREEN}✓ 已推送到 GitHub。${NC}"
  else
    echo -e "${BLUE}下一步：./sync-to-github.sh --push 或 cd EchoMind && git push${NC}"
  fi
else
  echo -e "${BLUE}下一步操作：${NC}"
  echo -e "  ./sync-to-github.sh --commit   # 提交"
  echo -e "  ./sync-to-github.sh --push     # 提交 + 推送"
  echo -e ""
  echo -e "  或手动操作："
  echo -e "  cd EchoMind"
  echo -e "  git add -A"
  echo -e "  git diff --cached   # 检查变更内容"
  echo -e "  git commit -m \"chore: sync source\""
  echo -e "  git push"
fi
