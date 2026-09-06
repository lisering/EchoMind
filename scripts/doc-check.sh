#!/usr/bin/env bash
set -euo pipefail

# EchoMind 文档质量自动化检查
# 检查项: 版本一致性、内部死链、文档过时、Markdown格式

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

PASS=0
FAIL=0
WARN=0

pass() { PASS=$((PASS + 1)); echo -e "  ${GREEN}✓ PASS${NC}: $1"; }
fail() { FAIL=$((FAIL + 1)); echo -e "  ${RED}✗ FAIL${NC}: $1"; }
warn() { WARN=$((WARN + 1)); echo -e "  ${YELLOW}⚠ WARN${NC}: $1"; }
section() { echo -e "\n${CYAN}▸ $1${NC}"; }

# ─── 1. 版本号一致性 ───────────────────────────────────────────────
check_version_consistency() {
    section "1. 版本号一致性"

    local tauri_version
    tauri_version=$(grep -oE '"version":\s*"[0-9]+\.[0-9]+\.[0-9]+"' crates/tauri-app/tauri.conf.json \
        | head -1 | grep -oE '[0-9]+\.[0-9]+\.[0-9]+')

    if [ -z "$tauri_version" ]; then
        fail "无法从 crates/tauri-app/tauri.conf.json 提取版本号"
        return
    fi
    pass "tauri.conf.json 版本: v${tauri_version}"

    # AGENTS.md
    if grep -q "v${tauri_version}" AGENTS.md 2>/dev/null; then
        pass "AGENTS.md 与 tauri.conf.json 一致 (v${tauri_version})"
    else
        fail "AGENTS.md 版本号与 tauri.conf.json (v${tauri_version}) 不一致"
    fi

    # docs/CHANGELOG.md — 检查"当前版本"行
    local cl_version
    cl_version=$(grep -oE '当前版本.*v[0-9]+\.[0-9]+\.[0-9]+' docs/CHANGELOG.md 2>/dev/null \
        | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' || true)
    if [ -n "$cl_version" ]; then
        if [ "$cl_version" = "$tauri_version" ]; then
            pass "CHANGELOG.md 当前版本与 tauri.conf.json 一致"
        else
            fail "CHANGELOG.md 当前版本 (v${cl_version}) 与 tauri.conf.json (v${tauri_version}) 不一致"
        fi
    else
        warn "CHANGELOG.md 未找到'当前版本'标注"
    fi

    # docs/SOFTWARE_ASSET.md — 检查顶部版本行
    local asset_version
    asset_version=$(grep -oE '版本.*v[0-9]+\.[0-9]+\.[0-9]+' docs/SOFTWARE_ASSET.md 2>/dev/null \
        | head -1 | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' || true)
    if [ -n "$asset_version" ]; then
        if [ "$asset_version" = "$tauri_version" ]; then
            pass "SOFTWARE_ASSET.md 版本与 tauri.conf.json 一致"
        else
            fail "SOFTWARE_ASSET.md 版本 (v${asset_version}) 与 tauri.conf.json (v${tauri_version}) 不一致"
        fi
    else
        warn "SOFTWARE_ASSET.md 未找到版本标注"
    fi

    # docs/sessions/CURRENT_STATE.md
    local cs_version
    cs_version=$(grep -oE '版本号.*\*\*[0-9]+\.[0-9]+\.[0-9]+' docs/sessions/CURRENT_STATE.md 2>/dev/null \
        | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' || true)
    if [ -n "$cs_version" ]; then
        if [ "$cs_version" = "$tauri_version" ]; then
            pass "CURRENT_STATE.md 版本与 tauri.conf.json 一致"
        else
            fail "CURRENT_STATE.md 版本 (${cs_version}) 与 tauri.conf.json (v${tauri_version}) 不一致"
        fi
    else
        warn "CURRENT_STATE.md 未找到版本标注"
    fi
}

# ─── 2. Markdown 内部死链 ─────────────────────────────────────────
check_dead_links_internal() {
    section "2. Markdown 内部文件链接"

    local md_files
    md_files=$(find . -name '*.md' \
        -not -path './EchoMind/*' \
        -not -path './target/*' \
        -not -path '*/node_modules/*' \
        -not -path './.git/*' 2>/dev/null)

    local checked=0
    local broken=0

    while IFS= read -r file; do
        [ -z "$file" ] && continue
        # 提取 [text](path) 中的本地路径，排除 URL (http/https) 和锚点 (#)
        local links
        links=$(grep -oE '\]\([^)#]+\.[a-zA-Z]{1,4}\)' "$file" 2>/dev/null \
            | sed 's/\](//;s/)//' || true)

        while IFS= read -r link; do
            [ -z "$link" ] && continue
            # 跳过 URL
            case "$link" in
                http://*|https://*|mailto:*) continue ;;
            esac
            checked=$((checked + 1))
            # 解析相对路径
            local dir
            dir=$(dirname "$file")
            local target="${dir}/${link}"
            if [ ! -f "$target" ]; then
                fail "${file} → 死链: ${link}"
                broken=$((broken + 1))
            fi
        done <<< "$links"
    done <<< "$md_files"

    if [ "$broken" -eq 0 ]; then
        pass "所有内部链接有效 (检查了 ${checked} 个链接)"
    else
        fail "发现 ${broken} 个死链 (共检查 ${checked} 个链接)"
    fi
}

# ─── 3. 文档过时检查 (90天阈值) ────────────────────────────────────
check_staleness() {
    section "3. 文档过时检查 (>90天未更新)"

    local now
    now=$(date +%s)
    local threshold=$((90 * 86400))
    local stale_count=0

    local key_docs=(
        "docs/SOFTWARE_ASSET.md"
        "docs/SRS_v1.0.md"
        "docs/TEST_STRATEGY.md"
        "docs/sessions/CURRENT_STATE.md"
        "CONTRIBUTING.md"
        "AGENTS.md"
        "PROJECT_RULES.md"
    )

    for doc in "${key_docs[@]}"; do
        if [ ! -f "$doc" ]; then
            warn "关键文档不存在: ${doc}"
            continue
        fi

        local mtime
        mtime=$(stat -f %m "$doc" 2>/dev/null || stat -c %Y "$doc" 2>/dev/null || echo 0)
        local age=$(( now - mtime ))

        if [ "$age" -gt "$threshold" ]; then
            local days=$(( age / 86400 ))
            warn "${doc} 已 ${days} 天未更新"
            stale_count=$((stale_count + 1))
        fi
    done

    if [ "$stale_count" -eq 0 ]; then
        pass "所有关键文档在 90 天内有更新"
    else
        warn "${stale_count} 个文档超过 90 天未更新"
    fi
}

# ─── 4. Markdown 格式检查 ──────────────────────────────────────────
check_markdown_format() {
    section "4. Markdown 格式检查"

    local issues=0

    local md_files
    md_files=$(find docs/ -name '*.md' \
        -not -path './EchoMind/*' \
        -not -path './target/*' \
        -not -path '*/node_modules/*' 2>/dev/null)

    while IFS= read -r file; do
        [ -z "$file" ] && continue

        # 行尾空格 (兼容 macOS grep)
        local trailing
        trailing=$(grep -c ' $' "$file" 2>/dev/null || true)
        trailing=$(echo "$trailing" | tr -d '[:space:]')
        if [ -n "$trailing" ] && [ "$trailing" -gt 0 ] 2>/dev/null; then
            warn "${file}: 发现 ${trailing} 处行尾空格"
            issues=$((issues + 1))
        fi
    done <<< "$md_files"

    if [ "$issues" -eq 0 ]; then
        pass "Markdown 格式未发现明显问题"
    else
        warn "发现 ${issues} 个格式问题"
    fi
}

# ─── 主入口 ────────────────────────────────────────────────────────
main() {
    echo "╔══════════════════════════════════════════╗"
    echo "║    EchoMind 文档质量自动化检查            ║"
    echo "╚══════════════════════════════════════════╝"

    check_version_consistency
    check_dead_links_internal
    check_staleness
    check_markdown_format

    echo ""
    echo "═══════════════════════════════════════════"
    echo -e "  ${GREEN}PASS: ${PASS}${NC}  ${RED}FAIL: ${FAIL}${NC}  ${YELLOW}WARN: ${WARN}${NC}"
    echo "═══════════════════════════════════════════"

    if [ "$FAIL" -gt 0 ]; then
        echo -e "\n${RED}检查未通过，请修复以上 FAIL 项后重试。${NC}"
        exit 1
    elif [ "$WARN" -gt 0 ]; then
        echo -e "\n${YELLOW}检查通过，但有 ${WARN} 个警告建议修复。${NC}"
        exit 0
    else
        echo -e "\n${GREEN}全部检查通过！${NC}"
        exit 0
    fi
}

main "$@"
