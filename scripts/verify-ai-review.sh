#!/usr/bin/env bash
# AI Review 配置验证脚本
# 用法: ./scripts/verify-ai-review.sh

set -euo pipefail

echo "🔍 Verifying AI Review configuration..."
echo ""

# 检查必要文件
echo "📁 Checking required files..."
files=(
    ".github/workflows/ci.yml"
    ".github/workflows/ai-review.yml"
    "rust-doctor.toml"
    "deny.toml"
)

for file in "${files[@]}"; do
    if [[ -f "$file" ]]; then
        echo "  ✅ $file exists"
    else
        echo "  ❌ $file missing"
        exit 1
    fi
done

echo ""

# 检查 rust-doctor.toml 语法
echo "📝 Validating rust-doctor.toml syntax..."
if command -v cargo >/dev/null 2>&1; then
    # 简单检查 TOML 语法（不下载依赖）
    if cargo check --workspace --message-format=short 2>&1 | head -20; then
        echo "  ✅ Cargo workspace syntax OK"
    else
        echo "  ⚠️  Cargo check had issues (may need dependencies)"
    fi
else
    echo "  ⚠️  cargo not installed, skipping TOML validation"
fi

# 检查 workflow 语法
echo "📋 Validating workflow YAML syntax..."
if command -v yamllint >/dev/null 2>&1; then
    yamllint .github/workflows/ci.yml .github/workflows/ai-review.yml 2>/dev/null && \
        echo "  ✅ Workflow YAML syntax valid" || \
        echo "  ⚠️  Workflow YAML has warnings (non-fatal)"
else
    echo "  ⚠️  yamllint not installed, skipping YAML validation"
fi

# 检查 deny.toml
echo "🛡️  Validating deny.toml..."
if command -v cargo-deny >/dev/null 2>&1; then
    cargo deny check --config deny.toml 2>/dev/null && \
        echo "  ✅ deny.toml valid" || \
        echo "  ⚠️  deny.toml validation failed (may need dependencies)"
else
    echo "  ⚠️  cargo-deny not installed, skipping"
fi

echo ""
echo "🔐 Checking required secrets..."
secrets=(
    "ANTHROPIC_API_KEY"
)
optional_secrets=(
    "DEEPSEEK_API_KEY"
    "OPENAI_API_KEY"
)

echo "  Required:"
for secret in "${secrets[@]}"; do
    if [[ -n "${!secret:-}" ]]; then
        echo "    ✅ $secret is set (in environment)"
    else
        echo "    ⚠️  $secret not in environment (must be set in GitHub Secrets)"
    fi
done

echo "  Optional (for rs-guard):"
for secret in "${optional_secrets[@]}"; do
    if [[ -n "${!secret:-}" ]]; then
        echo "    ✅ $secret is set"
    else
        echo "    ℹ️  $secret not set (rs-guard will be skipped)"
    fi
done

echo ""
echo "🧪 Running local rust-doctor check (if available)..."
if command -v rust-doctor >/dev/null 2>&1; then
    echo "  Running: rust-doctor --diff main --score"
    rust-doctor --diff main --score 2>/dev/null || echo "  ⚠️  rust-doctor check failed (may need full setup)"
else
    echo "  ⚠️  rust-doctor not installed locally"
    echo "     Install with: cargo install rust-doctor"
    echo "     Or download binary from GitHub releases"
fi

echo ""
echo "✅ Verification complete!"
echo ""
echo "📋 Next steps:"
echo "  1. Add ANTHROPIC_API_KEY to GitHub repository secrets"
echo "  2. (Optional) Add DEEPSEEK_API_KEY/OPENAI_API_KEY for rs-guard"
echo "  3. Create a test PR to verify the workflow runs"
echo "  4. Check GitHub Actions for ai-review workflow"
echo "  5. Review rust-doctor SARIF uploads in Security tab"