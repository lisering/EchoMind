#!/usr/bin/env bash
# ============================================================================
# EchoMind macOS 代码签名与公证脚本 (REQ-SEC-023)
# ============================================================================
#
# 功能：
#   1. 从 .p12 文件导入 Developer ID Application 证书到临时 keychain
#   2. 执行 tauri build（自动签名）
#   3. 提交公证 (notarytool)
#   4. Staple 公证票据
#   5. 验证签名 + Gatekeeper + 公证状态
#
# 使用方式：
#   ./scripts/sign-macos.sh                    # 签名 + 公证
#   ./scripts/sign-macos.sh --skip-notarize   # 仅签名，跳过公证
#   ./scripts/sign-macos.sh --verify-only       # 仅验证已有 .app
#
# 需要的环境变量：
#   SIGNING_CERTIFICATE_P12  — .p12 证书文件路径
#   SIGNING_PASSWORD          — .p12 证书密码
#   APPLE_SIGNING_IDENTITY    — 签名身份名称（如 "Developer ID Application: Your Name (XXXXXXXXXX)"）
#   APPLE_ID                  — Apple ID 邮箱（公证用）
#   APPLE_PASSWORD             — App-Specific Password（公证用）
#   APPLE_TEAM_ID              — Apple Team ID（公证用）
#
# 无证书环境：脚本自动检测，若无证书则跳过签名，仅构建未签名版本。
# ============================================================================

set -euo pipefail

# ─── 颜色输出 ───
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

info()  { echo -e "${GREEN}[INFO]${NC}  $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
error() { echo -e "${RED}[ERROR]${NC} $*"; }

# ─── 参数解析 ───
SKIP_NOTARIZE=false
VERIFY_ONLY=false
TARGET=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --skip-notarize)
            SKIP_NOTARIZE=true
            shift
            ;;
        --verify-only)
            VERIFY_ONLY=true
            shift
            ;;
        --target)
            TARGET="$2"
            shift 2
            ;;
        *)
            error "未知参数: $1"
            exit 1
            ;;
    esac
done

# ─── 项目路径 ───
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
APP_NAME="EchoMind"
APP_PATH="$PROJECT_ROOT/target/release/bundle/macos/$APP_NAME.app"

# ─── 仅验证模式 ───
if [[ "$VERIFY_ONLY" == "true" ]]; then
    info "验证模式：检查已有 .app 签名状态"
    if [[ ! -d "$APP_PATH" ]]; then
        error "未找到 .app 文件: $APP_PATH"
        exit 1
    fi
    info "验证签名: codesign -v --deep --strict"
    codesign -v --deep --strict "$APP_PATH" && info "签名验证通过 ✅" || { error "签名验证失败 ❌"; exit 1; }
    info "验证 Gatekeeper: spctl --assess --type execute"
    spctl --assess --type execute -vv "$APP_PATH" 2>&1 && info "Gatekeeper 通过 ✅" || { error "Gatekeeper 未通过 ❌"; exit 1; }
    info "验证公证票据: xcrun stapler staple"
    xcrun stapler validate "$APP_PATH" && info "公证票据有效 ✅" || warn "无公证票据（未公证或公证失败）"
    exit 0
fi

# ─── 检查证书是否可用 ───
HAS_CERTIFICATE=false
if [[ -n "${SIGNING_CERTIFICATE_P12:-}" && -f "${SIGNING_CERTIFICATE_P12}" ]]; then
    if [[ -n "${SIGNING_PASSWORD:-}" ]]; then
        HAS_CERTIFICATE=true
    fi
fi

HAS_NOTARIZE_CREDS=false
if [[ -n "${APPLE_ID:-}" && -n "${APPLE_PASSWORD:-}" && -n "${APPLE_TEAM_ID:-}" ]]; then
    HAS_NOTARIZE_CREDS=true
fi

# ─── 无证书：跳过签名，仅构建 ───
if [[ "$HAS_CERTIFICATE" == "false" ]]; then
    warn "未检测到 Developer ID 证书，跳过签名步骤"
    warn "设置 SIGNING_CERTIFICATE_P12 + SIGNING_PASSWORD 环境变量以启用签名"
    info "构建未签名版本..."
    cd "$PROJECT_ROOT"
    cargo tauri build ${TARGET:+--target "$TARGET"}
    warn "构建完成（未签名）。用户首次启动时会看到 Gatekeeper 警告。"
    exit 0
fi

# ─── 有证书：完整签名 + 公证流程 ───
info "检测到 Developer ID 证书，开始完整签名流程"

# 步骤 1: 导入证书到临时 keychain
KEYCHAIN_NAME="echomind-build-$(date +%s).keychain-db"
KEYCHAIN_PASSWORD="$(openssl rand -hex 16)"

info "步骤 1/5: 导入证书到临时 keychain"
security create-keychain -p "$KEYCHAIN_PASSWORD" "$KEYCHAIN_NAME"
security set-keychain-settings -t 3600 -u "$KEYCHAIN_NAME"
security unlock-keychain -p "$KEYCHAIN_PASSWORD" "$KEYCHAIN_NAME"
security import "$SIGNING_CERTIFICATE_P12" \
    -k "$KEYCHAIN_NAME" \
    -P "$SIGNING_PASSWORD" \
    -T /usr/bin/codesign \
    -T /usr/bin/security
security set-key-partition-list -S apple-tools: -s -k "$KEYCHAIN_PASSWORD" "$KEYCHAIN_NAME"

# 将临时 keychain 加入搜索列表
security list-keychains -d user -s "$KEYCHAIN_NAME" login.keychain

# 清理函数
cleanup() {
    info "清理临时 keychain..."
    security delete-keychain "$KEYCHAIN_NAME" 2>/dev/null || true
}
trap cleanup EXIT

# 步骤 2: 构建（Tauri 自动签名）
info "步骤 2/5: 构建 .app（Tauri 自动签名）"
export APPLE_SIGNING_IDENTITY="${APPLE_SIGNING_IDENTITY:-}"
cd "$PROJECT_ROOT"
cargo tauri build ${TARGET:+--target "$TARGET"}

info "签名验证: codesign -v"
codesign -v --deep --strict "$APP_PATH" && info "签名验证通过 ✅" || { error "签名验证失败 ❌"; exit 1; }

# 步骤 3: 公证（如果提供了凭据且未跳过）
if [[ "$SKIP_NOTARIZE" == "true" ]]; then
    warn "跳过公证步骤（--skip-notarize）"
    warn "应用已签名但未公证，用户首次启动仍会看到 Gatekeeper 警告"
    exit 0
fi

if [[ "$HAS_NOTARIZE_CREDS" == "false" ]]; then
    warn "未提供 Apple ID 凭据，跳过公证"
    warn "设置 APPLE_ID + APPLE_PASSWORD + APPLE_TEAM_ID 环境变量以启用公证"
    exit 0
fi

info "步骤 3/5: 提交公证 (notarytool)"
# 创建 zip 用于公证
ZIP_PATH="/tmp/echomind-notarize-$(date +%s).zip"
ditto -c -k --keepParent "$APP_PATH" "$ZIP_PATH"

# 提交公证
xcrun notarytool submit "$ZIP_PATH" \
    --apple-id "$APPLE_ID" \
    --password "$APPLE_PASSWORD" \
    --team-id "$APPLE_TEAM_ID" \
    --wait

info "公证完成 ✅"
rm -f "$ZIP_PATH"

# 步骤 4: Staple 公证票据
info "步骤 4/5: Staple 公证票据"
xcrun stapler staple "$APP_PATH"
xcrun stapler validate "$APP_PATH" && info "公证票据验证通过 ✅" || { error "公证票据验证失败 ❌"; exit 1; }

# 步骤 5: 最终验证
info "步骤 5/5: 最终验证"
info "验证签名: codesign -v"
codesign -v --deep --strict "$APP_PATH" && info "签名验证通过 ✅" || { error "签名验证失败 ❌"; exit 1; }

info "验证 Gatekeeper: spctl --assess"
spctl --assess --type execute -vv "$APP_PATH" 2>&1 && info "Gatekeeper 通过 ✅" || { error "Gatekeeper 未通过 ❌"; exit 1; }

info "验证公证票据: stapler validate"
xcrun stapler validate "$APP_PATH" && info "公证票据有效 ✅" || { error "公证票据无效 ❌"; exit 1; }

echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}  ✅ macOS 签名与公证全部完成！${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
info "已签名+公证的 .app: $APP_PATH"
info "用户启动时不会看到 Gatekeeper 警告"
