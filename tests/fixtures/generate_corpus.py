#!/usr/bin/env python3
"""
EchoMind 混沌测试语料自动生成脚本（Phase 8.5，REQ-CHAOS）

功能：
  1. large_real.md      — 从 GitHub Raw 拉取知名开源项目超长 README
  2. complex_paper.pdf  — 从 arXiv 下载含图表/公式/多栏排版的真实学术论文 PDF
  3. mixed_encoding.txt — 混合 UTF-8 / GBK / 非法字节的编码地狱文本
  4. malicious.md       — 含 <script> / <img onerror> 的恶意 Markdown
  5. fake.pdf           — 纯文本内容但扩展名为 .pdf 的格式欺骗文件
  6. empty.md           — 空文件
  7. deep_nested/       — 50 层嵌套目录，每层一个 1KB txt 文件

设计原则：
  - 网络获取失败时使用本地 fallback，保证 CI 离线可跑
  - 幂等执行：重复运行不报错，文件覆盖更新
  - 代理规范（PROJECT_RULES.md 铁律一）：仅在直连失败时启用代理，用完即 unset

用法：
  python3 tests/fixtures/generate_corpus.py
"""

import os
import sys
import struct
import time
from pathlib import Path
from typing import Optional

# ── 路径配置 ──────────────────────────────────────────────
FIXTURES_DIR = Path(__file__).resolve().parent

# ── 代理配置（铁律一：仅在网络失败时启用，用完即 unset）─────
PROXY_ENV = {
    "https_proxy": "http://127.0.0.1:7890",
    "http_proxy": "http://127.0.0.1:7890",
    "all_proxy": "socks5://127.0.0.1:7890",
}


def _set_proxy():
    """启用本地代理（仅当直连失败时调用）。"""
    for k, v in PROXY_ENV.items():
        os.environ[k] = v


def _unset_proxy():
    """清除代理环境变量（铁律一：用完必须 unset）。"""
    for k in PROXY_ENV:
        os.environ.pop(k, None)


# ── HTTP 下载工具 ─────────────────────────────────────────
def download(url: str, timeout: int = 30) -> Optional[bytes]:
    """
    下载 URL 内容；直连失败时自动尝试代理，代理也失败则返回 None。

    Args:
        url: 要下载的 URL
        timeout: 超时秒数

    Returns:
        下载的字节内容，或 None（全部失败时）
    """
    import urllib.request

    # 第一次：直连
    try:
        print(f"  [直连] 下载 {url} ...")
        req = urllib.request.Request(url, headers={"User-Agent": "EchoMind-Test/1.0"})
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return resp.read()
    except Exception as e:
        print(f"  [直连失败] {e}")

    # 第二次：走代理
    _set_proxy()
    try:
        print(f"  [代理] 下载 {url} ...")
        req = urllib.request.Request(url, headers={"User-Agent": "EchoMind-Test/1.0"})
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            data = resp.read()
            print(f"  [代理成功] 下载 {len(data)} 字节")
            return data
    except Exception as e:
        print(f"  [代理失败] {e}")
        return None
    finally:
        _unset_proxy()  # 铁律一：用完即 unset


# ── 1. 大型真实 Markdown ──────────────────────────────────
def gen_large_real_md():
    """
    从 GitHub Raw 拉取 Rust 语言的超长 README 作为真实大型 Markdown 语料。
    失败时生成本地 fallback（包含重复段落的超长 Markdown）。
    """
    print("[1/7] 生成 large_real.md ...")
    target = FIXTURES_DIR / "large_real.md"

    # 尝试从 GitHub 拉取 Rust README（真实超长文档）
    urls = [
        "https://raw.githubusercontent.com/rust-lang/rust/master/README.md",
        "https://raw.githubusercontent.com/tauri-apps/tauri/dev/README.md",
    ]
    for url in urls:
        data = download(url)
        if data and len(data) > 5000:
            target.write_bytes(data)
            print(f"  ✓ 已保存 {len(data)} 字节（来源: {url}）")
            return

    # Fallback：本地生成超长 Markdown（~200KB）
    print("  [Fallback] 本地生成超长 Markdown ...")
    section = """\
# Section {n}: 测试抗压能力

## 概述

本节用于测试 EchoMind 文档导入管线对超长 Markdown 的处理能力。
包含代码块、列表、表格等多种 Markdown 元素，确保分块器正确处理。

## 代码示例

```rust
fn main() {{
    println!("Hello, EchoMind!");
    let data = vec![1, 2, 3, 4, 5];
    for item in &data {{
        println!("Item: {{}}", item);
    }}
}}
```

## 列表

- 项目 A
- 项目 B
- 项目 C

## 表格

| 列1 | 列2 | 列3 |
|-----|-----|-----|
| 数据 | 值 | 备注 |

## 中文内容测试

这是中文段落，用于验证 UTF-8 编码的中文文本在分块器中能正确处理。
灵犀本地知识库系统支持中英文混合文档的索引与检索。

"""
    content = "\n".join(section.format(n=i) for i in range(1, 201))
    target.write_text(content, encoding="utf-8")
    print(f"  ✓ Fallback 已保存 {len(content)} 字节")


# ── 2. 复杂学术论文 PDF ───────────────────────────────────
def gen_complex_paper_pdf():
    """
    从 arXiv 下载一篇包含图表、数学公式和多栏排版的真实学术论文 PDF。
    失败时生成一个包含多页文本的最小合法 PDF 作为 fallback。
    """
    print("[2/7] 生成 complex_paper.pdf ...")
    target = FIXTURES_DIR / "complex_paper.pdf"

    # 尝试从 arXiv 下载 "Attention Is All You Need" 论文
    url = "https://arxiv.org/pdf/1706.03762v7"
    data = download(url, timeout=60)
    if data and data[:4] == b"%PDF":
        target.write_bytes(data)
        print(f"  ✓ 已保存 {len(data)} 字节（来源: arXiv 1706.03762）")
        return

    # Fallback：生成最小合法多页 PDF
    print("  [Fallback] 生成最小合法 PDF ...")
    _generate_minimal_pdf(target)
    print(f"  ✓ Fallback PDF 已保存")


def _generate_minimal_pdf(target: Path):
    """生成包含多页文本内容的最小合法 PDF 文件。"""
    pages = []
    for i in range(1, 6):
        text = f"Page {i}: This is a test page for PDF parsing. " * 20
        content_stream = (
            f"BT\n/F1 12 Tf\n72 720 Td\n({text}) Tj\nET"
        ).encode("latin-1")
        pages.append((i, content_stream))

    objects = []
    # Object 1: Catalog
    objects.append(b"<< /Type /Catalog /Pages 2 0 R >>")
    # Object 2: Pages
    kids = " ".join(f"{3 + 2*i} 0 R" for i in range(len(pages)))
    objects.append(f"<< /Type /Pages /Kids [{kids}] /Count {len(pages)} >>".encode())

    # Font + Page objects
    for i, (page_num, stream) in enumerate(pages):
        # Font object
        objects.append(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")
        # Page content stream
        stream_obj = f"<< /Length {len(stream)} >>\nstream\n".encode() + stream + b"\nendstream"
        objects.append(stream_obj)

    # Page objects (reference font + content)
    page_objs = []
    for i in range(len(pages)):
        font_obj_num = 3 + 2 * i
        content_obj_num = 4 + 2 * i
        page_obj = (
            f"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] "
            f"/Resources << /Font << /F1 {font_obj_num} 0 R >> >> "
            f"/Contents {content_obj_num} 0 R >>"
        )
        page_objs.append(page_obj.encode())

    # Assemble PDF
    pdf = b"%PDF-1.4\n"
    offsets = []
    for i, obj in enumerate(objects):
        offsets.append(len(pdf))
        pdf += f"{i+1} 0 obj\n".encode() + obj + b"\nendobj\n"

    # Insert page objects after the content stream objects
    # Actually, we need to interleave: for each page, we have font obj + content obj + page obj
    # Let's rebuild properly
    pdf = b"%PDF-1.4\n"
    offsets = []

    # Obj 1: Catalog
    offsets.append(len(pdf))
    pdf += b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n"

    # Obj 2: Pages
    offsets.append(len(pdf))
    kids = " ".join(f"{4 + 3*i} 0 R" for i in range(len(pages)))
    pdf += f"2 0 obj\n<< /Type /Pages /Kids [{kids}] /Count {len(pages)} >>\nendobj\n".encode()

    # For each page: Font (obj), Content (obj), Page (obj)
    for i, (_, stream) in enumerate(pages):
        font_num = 3 + 3 * i
        content_num = 4 + 3 * i
        page_num = 5 + 3 * i

        # Font
        offsets.append(len(pdf))
        pdf += f"{font_num} 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".encode()

        # Content stream
        offsets.append(len(pdf))
        pdf += f"{content_num} 0 obj\n<< /Length {len(stream)} >>\nstream\n".encode()
        pdf += stream + b"\nendstream\nendobj\n"

        # Page
        offsets.append(len(pdf))
        pdf += (
            f"{page_num} 0 obj\n"
            f"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] "
            f"/Resources << /Font << /F1 {font_num} 0 R >> >> "
            f"/Contents {content_num} 0 R >>\n"
            f"endobj\n"
        ).encode()

    # Cross-reference table
    xref_offset = len(pdf)
    num_objects = len(offsets) + 1
    pdf += f"xref\n0 {num_objects}\n".encode()
    pdf += b"0000000000 65535 f \n"
    for offset in offsets:
        pdf += f"{offset:010d} 00000 n \n".encode()

    pdf += b"trailer\n"
    pdf += f"<< /Size {num_objects} /Root 1 0 R >>\n".encode()
    pdf += b"startxref\n"
    pdf += f"{xref_offset}\n".encode()
    pdf += b"%%EOF\n"

    target.write_bytes(pdf)


# ── 3. 编码地狱 TXT ───────────────────────────────────────
def gen_mixed_encoding_txt():
    """
    生成混合编码文本：UTF-8 中文 + GBK 中文 + 非法字节 \\xFF\\xFE。
    用于测试 from_utf8_lossy 容错机制。
    """
    print("[3/7] 生成 mixed_encoding.txt ...")
    target = FIXTURES_DIR / "mixed_encoding.txt"

    parts = []
    # UTF-8 编码的中文
    parts.append("这是 UTF-8 编码的中文文本。\n".encode("utf-8"))
    # 正常英文
    parts.append(b"Normal English text here.\n")
    # GBK 编码的中文（在 UTF-8 解码时会变成乱码但不崩溃）
    parts.append("这是 GBK 编码的中文。\n".encode("gbk"))
    # 非法字节序列
    parts.append(b"\xFF\xFE\xFF\xFE")
    # 更多 UTF-8 中文
    parts.append("\n这是恢复后的 UTF-8 中文文本。\n".encode("utf-8"))
    # 又一段 GBK
    parts.append("灵犀知识库测试。\n".encode("gbk"))
    # 尾部非法字节
    parts.append(b"\x80\x81\x82\xFF")

    target.write_bytes(b"".join(parts))
    print(f"  ✓ 已保存 {target.stat().st_size} 字节")


# ── 4. 恶意 Markdown ──────────────────────────────────────
def gen_malicious_md():
    """
    生成包含 XSS 攻击向量的恶意 Markdown：
    - <script>alert('XSS')</script>
    - <img src=x onerror=alert(1)>
    - <a href="javascript:void(0)">click</a>
    用于验证 MarkdownLoader 是否剥离原始 HTML 标签。
    """
    print("[4/7] 生成 malicious.md ...")
    target = FIXTURES_DIR / "malicious.md"

    content = """\
# Malicious Markdown Test

## XSS 攻击向量

<script>alert('XSS')</script>

<img src=x onerror="alert(document.cookie)">

<a href="javascript:void(0)">Click me</a>

<iframe src="javascript:alert(1)"></iframe>

## 正常内容

这段文字应该被保留。MarkdownLoader 应剥离上面的 HTML 标签但保留正文。

## 代码块中的 script（安全，应保留）

```html
<script>alert('safe in code block')</script>
```

## 混合内容

正常段落。<script>alert('inline')</script> 另一段正常文字。

<img src="valid.png" onerror="alert(1)" alt="image">

## 结尾正常文字

这是文件末尾的正常文字，用于验证恶意标签不会截断正文提取。
"""
    target.write_text(content, encoding="utf-8")
    print(f"  ✓ 已保存 {len(content)} 字节")


# ── 5. 格式欺骗文件 ───────────────────────────────────────
def gen_fake_pdf():
    """
    生成纯文本内容但保存为 .pdf 扩展名的格式欺骗文件。
    PdfLoader 解析时应返回 Err 而非 Panic。
    """
    print("[5/7] 生成 fake.pdf ...")
    target = FIXTURES_DIR / "fake.pdf"

    content = "This is a plain text file disguised as a PDF. There is no PDF header."
    target.write_text(content, encoding="utf-8")
    print(f"  ✓ 已保存 {len(content)} 字节")


# ── 6. 空文件 ─────────────────────────────────────────────
def gen_empty_md():
    """生成空 Markdown 文件，测试空内容边界处理。"""
    print("[6/7] 生成 empty.md ...")
    target = FIXTURES_DIR / "empty.md"
    target.write_text("", encoding="utf-8")
    print(f"  ✓ 已保存 0 字节")


# ── 7. 深套目录 ───────────────────────────────────────────
def gen_deep_nested():
    """
    生成 50 层嵌套目录，每层放入一个 1KB txt 文件。
    测试文件遍历与批量导入能力。
    """
    print("[7/7] 生成 deep_nested/ ...")
    base = FIXTURES_DIR / "deep_nested"

    # 清理旧目录
    import shutil
    if base.exists():
        shutil.rmtree(base)

    base.mkdir()
    current = base
    for level in range(1, 51):
        current = current / f"level_{level:02d}"
        current.mkdir()
        # 每层放一个 1KB txt 文件
        content = (
            f"Deep nested file at level {level}.\n"
            f"This file is at depth {level} in the directory tree.\n"
            f"Content for testing batch import of deeply nested files.\n"
        )
        # 填充到约 1KB
        content = content + "A" * (1024 - len(content) - 1) + "\n"
        (current / f"file_{level:02d}.txt").write_text(content, encoding="utf-8")

    total_files = sum(1 for _ in base.rglob("*.txt"))
    print(f"  ✓ 已生成 50 层目录，{total_files} 个 txt 文件")


# ── 主入口 ────────────────────────────────────────────────
def main():
    print("=" * 60)
    print("EchoMind 混沌测试语料生成脚本 (Phase 8.5)")
    print("=" * 60)
    print(f"输出目录: {FIXTURES_DIR}")
    print()

    gen_large_real_md()
    gen_complex_paper_pdf()
    gen_mixed_encoding_txt()
    gen_malicious_md()
    gen_fake_pdf()
    gen_empty_md()
    gen_deep_nested()

    print()
    print("=" * 60)
    print("✓ 所有语料生成完成！")
    print("=" * 60)

    # 列出生成的文件
    print("\n生成的文件清单:")
    for f in sorted(FIXTURES_DIR.iterdir()):
        if f.is_file() and f.name != "generate_corpus.py":
            size = f.stat().st_size
            print(f"  {f.name:30s} {size:>10,} bytes")
        elif f.is_dir():
            file_count = sum(1 for _ in f.rglob("*") if _.is_file())
            print(f"  {f.name + '/':30s} {file_count:>10} files")


if __name__ == "__main__":
    main()
