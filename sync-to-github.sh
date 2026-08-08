#!/usr/bin/env bash
#
# sync-to-github.sh — 将开发目录的源代码同步到 EchoMind/EchoMind 发布仓库
#
# 用法：在项目根目录 /Users/john/freesoft/EchoMind/ 下执行
#   ./sync-to-github.sh          # 同步并显示 diff
#   ./sync-to-github.sh --commit  # 同步并自动提交
#
# 详见铁律十一：GitHub 发布同步规则
#

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"
RELEASE_DIR="$ROOT_DIR/EchoMind"

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}  EchoMind → GitHub 发布仓库同步${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""

# 检查发布目录存在
if [ ! -d "$RELEASE_DIR/.git" ]; then
  echo -e "${RED}错误：发布仓库 $RELEASE_DIR 不存在或不是 Git 仓库${NC}"
  exit 1
fi

# 同步函数：rsync 单个文件或目录
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

echo -e "${BLUE}[1/9] 同步根配置文件${NC}"
sync_file "$ROOT_DIR/Cargo.toml"           "$RELEASE_DIR/Cargo.toml"
sync_file "$ROOT_DIR/Cargo.lock"            "$RELEASE_DIR/Cargo.lock"
sync_file "$ROOT_DIR/deny.toml"             "$RELEASE_DIR/deny.toml"
sync_file "$ROOT_DIR/.gitignore"            "$RELEASE_DIR/.gitignore"

# 从发布仓库的 Cargo.toml 中移除 license-issuer 成员（私有工具，不公开）
if [ -f "$RELEASE_DIR/Cargo.toml" ]; then
  sed -i '' '/"crates\/license-issuer"/d' "$RELEASE_DIR/Cargo.toml"
  echo -e "  ${GREEN}✓${NC} Cargo.toml (已移除 license-issuer 成员)"
fi

echo ""
echo -e "${BLUE}[2/9] 同步 crates/models${NC}"
sync_dir "$ROOT_DIR/crates/models/src"      "$RELEASE_DIR/crates/models/src"
sync_file "$ROOT_DIR/crates/models/Cargo.toml" "$RELEASE_DIR/crates/models/Cargo.toml"

echo ""
echo -e "${BLUE}[3/9] 同步 crates/prompt${NC}"
sync_dir "$ROOT_DIR/crates/prompt/src"      "$RELEASE_DIR/crates/prompt/src"
sync_file "$ROOT_DIR/crates/prompt/Cargo.toml" "$RELEASE_DIR/crates/prompt/Cargo.toml"

echo ""
echo -e "${BLUE}[4/9] 同步 crates/compact${NC}"
sync_dir "$ROOT_DIR/crates/compact/src"     "$RELEASE_DIR/crates/compact/src"
sync_file "$ROOT_DIR/crates/compact/Cargo.toml" "$RELEASE_DIR/crates/compact/Cargo.toml"

echo ""
echo -e "${BLUE}[5/9] 同步 crates/core${NC}"
sync_dir "$ROOT_DIR/crates/core/src"        "$RELEASE_DIR/crates/core/src"
sync_file "$ROOT_DIR/crates/core/Cargo.toml" "$RELEASE_DIR/crates/core/Cargo.toml"

echo ""
echo -e "${BLUE}[6/9] 同步 crates/infra${NC}"
sync_dir "$ROOT_DIR/crates/infra/src"       "$RELEASE_DIR/crates/infra/src"
sync_file "$ROOT_DIR/crates/infra/Cargo.toml" "$RELEASE_DIR/crates/infra/Cargo.toml"

echo ""
echo -e "${BLUE}[7/9] 同步 crates/tauri-app${NC}"
sync_dir "$ROOT_DIR/crates/tauri-app/src"   "$RELEASE_DIR/crates/tauri-app/src"
sync_dir "$ROOT_DIR/crates/tauri-app/capabilities" "$RELEASE_DIR/crates/tauri-app/capabilities"
sync_dir "$ROOT_DIR/crates/tauri-app/icons" "$RELEASE_DIR/crates/tauri-app/icons"
sync_dir "$ROOT_DIR/crates/tauri-app/tests" "$RELEASE_DIR/crates/tauri-app/tests"
sync_file "$ROOT_DIR/crates/tauri-app/Cargo.toml" "$RELEASE_DIR/crates/tauri-app/Cargo.toml"
sync_file "$ROOT_DIR/crates/tauri-app/build.rs" "$RELEASE_DIR/crates/tauri-app/build.rs"
sync_file "$ROOT_DIR/crates/tauri-app/tauri.conf.json" "$RELEASE_DIR/crates/tauri-app/tauri.conf.json"

echo ""
echo -e "${BLUE}[8/9] 同步 crates/license-issuer${NC}"
echo -e "  ${YELLOW}⚠ 跳过（license-issuer 为私有工具，不公开到 GitHub）${NC}"

echo ""
echo -e "${BLUE}[9/9] 同步 ui/${NC}"
sync_file "$ROOT_DIR/ui/index.html"         "$RELEASE_DIR/ui/index.html"
sync_file "$ROOT_DIR/ui/logo.png"           "$RELEASE_DIR/ui/logo.png"
sync_dir "$ROOT_DIR/ui/locales"             "$RELEASE_DIR/ui/locales"
sync_dir "$ROOT_DIR/ui/styles"             "$RELEASE_DIR/ui/styles"
sync_dir "$ROOT_DIR/ui/vendor"              "$RELEASE_DIR/ui/vendor"
# ui/src/ 仅开发模块化版本使用，index.html 内联了全部 JS，发布不需要

echo ""
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

# 如果带了 --commit 参数，自动提交
if [ "${1:-}" = "--commit" ]; then
  echo -e "${BLUE}自动提交中...${NC}"
  git add -A

  # 生成提交信息
  COMMIT_MSG="chore: sync source from dev workspace

Synced at $(date '+%Y-%m-%d %H:%M:%S')
Files changed: $(echo "$CHANGED$UNTRACKED" | grep -c .)"

  git commit -m "$COMMIT_MSG"
  echo ""
  echo -e "${GREEN}✓ 已提交到发布仓库。${NC}"
  echo -e "${BLUE}下一步：cd EchoMind && git push${NC}"
else
  echo -e "${BLUE}下一步操作：${NC}"
  echo -e "  cd EchoMind"
  echo -e "  git add -A"
  echo -e "  git diff --cached   # 检查变更内容"
  echo -e "  git commit -m \"chore: sync source\""
  echo -e "  git push"
  echo ""
  echo -e "${YELLOW}提示：使用 ./sync-to-github.sh --commit 可自动提交${NC}"
fi
