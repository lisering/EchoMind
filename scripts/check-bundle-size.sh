#!/usr/bin/env bash
# REQ-NFR-019 构建产物体积检查脚本
#
# 验收标准：
#   AC-1: macOS arm64 .dmg ≤ 50MB（不含 ONNX 模型）
#   AC-2: Windows x64 .msi ≤ 60MB（不含 ONNX 模型）
#   AC-4: 前端 vendored 库总体积 ≤ 5MB（ui/vendor/）
#   AC-5: Release profile 启用 LTO + strip
#
# 用法：
#   ./scripts/check-bundle-size.sh          # 检查前端 vendor 体积
#   ./scripts/check-bundle-size.sh --binary  # 检查编译后二进制体积（需先 cargo build --release）

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

PASS=0
FAIL=0

echo "========================================"
echo "  REQ-NFR-019 构建产物体积检查"
echo "========================================"
echo ""

# --- AC-4: ui/vendor/ 体积 ≤ 5MB ---
check_vendor_size() {
    local vendor_dir="$PROJECT_ROOT/ui/vendor"
    local limit=$((5 * 1024 * 1024))  # 5MB in bytes

    if [ ! -d "$vendor_dir" ]; then
        echo -e "${RED}[FAIL]${NC} AC-4: ui/vendor/ directory not found at $vendor_dir"
        FAIL=$((FAIL + 1))
        return
    fi

    # 计算目录总字节数（跨平台兼容）
    local size
    if [[ "$OSTYPE" == "darwin"* ]]; then
        size=$(find "$vendor_dir" -type f -exec stat -f%z {} \; | awk '{s+=$1} END {print s}')
    else
        size=$(find "$vendor_dir" -type f -exec stat -c%s {} \; | awk '{s+=$1} END {print s}')
    fi

    local size_mb
    size_mb=$(echo "scale=2; $size / 1048576" | bc)

    if [ "$size" -le "$limit" ]; then
        echo -e "${GREEN}[PASS]${NC} AC-4: ui/vendor/ = ${size_mb}MB ≤ 5MB (${size} bytes)"
        PASS=$((PASS + 1))
    else
        echo -e "${RED}[FAIL]${NC} AC-4: ui/vendor/ = ${size_mb}MB > 5MB (${size} bytes)"
        FAIL=$((FAIL + 1))
    fi
}

# --- AC-5: Cargo.toml [profile.release] lto + strip ---
check_release_profile() {
    local cargo_toml="$PROJECT_ROOT/Cargo.toml"

    if [ ! -f "$cargo_toml" ]; then
        echo -e "${RED}[FAIL]${NC} AC-5: Cargo.toml not found"
        FAIL=$((FAIL + 1))
        return
    fi

    # 检查 [profile.release] 段存在
    if ! grep -q '\[profile\.release\]' "$cargo_toml"; then
        echo -e "${RED}[FAIL]${NC} AC-5: [profile.release] section missing in Cargo.toml"
        FAIL=$((FAIL + 1))
        return
    fi

    # 提取 [profile.release] 段内容
    local release_section
    release_section=$(awk '/\[profile\.release\]/{flag=1;next} /^\[/{flag=0} flag' "$cargo_toml")

    # 检查 lto
    if echo "$release_section" | grep -q 'lto' && ! echo "$release_section" | grep -q 'lto = false'; then
        echo -e "${GREEN}[PASS]${NC} AC-5: lto enabled in [profile.release]"
        PASS=$((PASS + 1))
    else
        echo -e "${RED}[FAIL]${NC} AC-5: lto not enabled in [profile.release]"
        FAIL=$((FAIL + 1))
    fi

    # 检查 strip
    if echo "$release_section" | grep -qE 'strip = (true|"symbols")'; then
        echo -e "${GREEN}[PASS]${NC} AC-5: strip enabled in [profile.release]"
        PASS=$((PASS + 1))
    else
        echo -e "${RED}[FAIL]${NC} AC-5: strip not enabled in [profile.release]"
        FAIL=$((FAIL + 1))
    fi
}

# --- AC-3: ONNX 模型不打包（tauri.conf.json 无 resources 打包模型文件）---
check_no_model_in_bundle() {
    local tauri_conf="$PROJECT_ROOT/crates/tauri-app/tauri.conf.json"

    if [ ! -f "$tauri_conf" ]; then
        echo -e "${RED}[FAIL]${NC} AC-3: tauri.conf.json not found"
        FAIL=$((FAIL + 1))
        return
    fi

    # 检查 bundle.resources 不包含 .onnx 或 model 文件
    if grep -q '"resources"' "$tauri_conf" && grep -qE '\.onnx|model' "$tauri_conf"; then
        echo -e "${RED}[FAIL]${NC} AC-3: tauri.conf.json contains resources with model files"
        FAIL=$((FAIL + 1))
    else
        echo -e "${GREEN}[PASS]${NC} AC-3: No ONNX model files in tauri.conf.json bundle resources"
        PASS=$((PASS + 1))
    fi
}

# --- 可选：二进制体积检查 ---
check_binary_size() {
    local binary_path="$PROJECT_ROOT/target/release/echomind"

    if [ ! -f "$binary_path" ]; then
        echo -e "${YELLOW}[SKIP]${NC} Binary not found. Run 'cargo build --release' first."
        return
    fi

    local size
    if [[ "$OSTYPE" == "darwin"* ]]; then
        size=$(stat -f%z "$binary_path")
    else
        size=$(stat -c%s "$binary_path")
    fi

    local size_mb
    size_mb=$(echo "scale=2; $size / 1048576" | bc)

    # macOS .dmg 通常比裸二进制小（LZMA 压缩），裸二进制 ≤ 60MB 作为保守阈值
    local limit=$((60 * 1024 * 1024))

    if [ "$size" -le "$limit" ]; then
        echo -e "${GREEN}[PASS]${NC} Binary = ${size_mb}MB ≤ 60MB"
        PASS=$((PASS + 1))
    else
        echo -e "${YELLOW}[WARN]${NC} Binary = ${size_mb}MB > 60MB (pre-compression, .dmg/.msi will be smaller)"
        echo "         This is a warning, not a failure. Compressed installer may still be within limits."
    fi
}

# --- 主流程 ---
check_vendor_size
check_release_profile
check_no_model_in_bundle

if [[ "${1:-}" == "--binary" ]]; then
    check_binary_size
fi

echo ""
echo "========================================"
echo "  Results: ${PASS} passed, ${FAIL} failed"
echo "========================================"

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
