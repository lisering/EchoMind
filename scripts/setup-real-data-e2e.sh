#!/usr/bin/env bash
# setup-real-data-e2e.sh — 配置 real-data-e2e 的 LLM 凭据并触发验证
#
# 用法（在用户自己的终端执行，key 不经过 AI 对话/日志）：
#   ./scripts/setup-real-data-e2e.sh sk-你的deepseek-key
#
# 可选环境变量：
#   ECHOMIND_LLM_BASE_URL（默认 https://api.deepseek.com）
#   ECHOMIND_LLM_MODEL（默认 deepseek-chat）
set -euo pipefail

if [ $# -lt 1 ]; then
  echo "用法: $0 <deepseek-api-key>"
  echo "示例: $0 sk-xxxxxxxxxxxxxxxx"
  exit 1
fi

KEY="$1"
BASE_URL="${ECHOMIND_LLM_BASE_URL:-https://api.deepseek.com}"
MODEL="${ECHOMIND_LLM_MODEL:-deepseek-chat}"
REPO="lisering/EchoMind"

echo "==> 1/3 配置 GitHub Actions secrets（key 不落盘、不进日志）"
gh secret set ECHOMIND_LLM_API_KEY --repo "$REPO" --body "$KEY"
gh secret set ECHOMIND_LLM_BASE_URL --repo "$REPO" --body "$BASE_URL"
gh secret set ECHOMIND_LLM_MODEL  --repo "$REPO" --body "$MODEL"
echo "    ✓ 3 个 secrets 已写入 $REPO"

echo "==> 2/3 写入本地 shell 配置（供本地 real-data 跑测使用）"
LINE="export ECHOMIND_LLM_API_KEY='$KEY' ECHOMIND_LLM_BASE_URL='$BASE_URL' ECHOMIND_LLM_MODEL='$MODEL'"
if ! grep -q 'ECHOMIND_LLM_API_KEY' ~/.zshrc 2>/dev/null; then
  echo "$LINE" >> ~/.zshrc
  echo "    ✓ 已追加到 ~/.zshrc（新终端生效）"
else
  echo "    ⚠ ~/.zshrc 已存在 ECHOMIND_LLM_API_KEY，跳过（如需更新请手动改）"
fi

echo "==> 3/3 触发 GitHub Actions real-data workflow"
gh workflow run "Real Data E2E" --repo "$REPO" 2>/dev/null \
  && echo "    ✓ 已触发，进度: https://github.com/$REPO/actions/workflows/real-data-e2e.yml" \
  || echo "    ⚠ 触发失败（workflow 文件可能尚未同步到默认分支），稍后手动触发"

echo ""
echo "✅ 完成。本地跑测：新开终端后 cd e2e-tests && npx playwright test tests/real-data.spec.ts"
echo "   （需先启动 cargo tauri dev）"
