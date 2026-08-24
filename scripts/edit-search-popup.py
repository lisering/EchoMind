#!/usr/bin/env python3
"""Add search mode toggle buttons to the conversation search popup."""
import sys

filepath = 'ui/index.html'

with open(filepath, 'r', encoding='utf-8') as f:
    content = f.read()

old = 'placeholder:text-text-quaternary" />\n          <kbd class="text-[11px] text-text-quaternary bg-surface-3 px-1.5 rounded-xs shrink-0">Esc</kbd>\n          <button id="convSearchClose"'

new = """placeholder:text-text-quaternary" />
          <!-- 搜索模式切换：会话标题 / 对话内容（REQ-RAG-040） -->
          <div class="flex items-center gap-0.5 shrink-0 bg-surface-3 rounded-xs p-0.5">
            <button id="convSearchModeTitle" class="px-2 py-0.5 text-[11px] rounded-xs transition-colors bg-accent/20 text-accent" data-i18n="sidebar.search_mode_title">会话</button>
            <button id="convSearchModeContent" class="px-2 py-0.5 text-[11px] rounded-xs transition-colors text-text-tertiary hover:text-text-secondary" data-i18n="sidebar.search_mode_content">对话</button>
          </div>
          <kbd class="text-[11px] text-text-quaternary bg-surface-3 px-1.5 rounded-xs shrink-0">Esc</kbd>
          <button id="convSearchClose\""""

if old in content:
    content = content.replace(old, new, 1)
    with open(filepath, 'w', encoding='utf-8') as f:
        f.write(content)
    print('OK: replaced successfully')
else:
    print('NOT FOUND - checking with different approach')
    # Try to find a smaller match
    idx = content.find('id="convSearchPopupInput"')
    if idx >= 0:
        print(f'Found convSearchPopupInput at index {idx}')
        # Show surrounding context
        start = max(0, idx - 50)
        end = min(len(content), idx + 300)
        print(repr(content[start:end]))
    else:
        print('convSearchPopupInput not found at all')
    sys.exit(1)
