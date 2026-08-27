#!/usr/bin/env python3
"""Phase 1 R1: Remove academic RAG traits and Storage methods from core/lib.rs"""
import re
import sys

filepath = 'crates/core/src/lib.rs'
with open(filepath, 'r') as f:
    lines = f.readlines()

output = []
skip_until_closing_brace = False
brace_depth = 0
i = 0

while i < len(lines):
    line = lines[i]
    
    # Skip PromptCompressor trait block (doc comment + trait + NoCompressor struct + impl)
    if 'PromptCompressor' in line and 'pub trait' in line:
        # Skip backwards to remove doc comments already added
        while output and (output[-1].strip().startswith('///') or output[-1].strip() == ''):
            output.pop()
        # Skip the entire trait block until matching closing brace
        brace_depth = 0
        while i < len(lines):
            if '{' in lines[i]:
                brace_depth += lines[i].count('{')
            if '}' in lines[i]:
                brace_depth -= lines[i].count('}')
            i += 1
            if brace_depth <= 0:
                break
        continue
    
    # Skip NoCompressor struct + impl
    if 'pub struct NoCompressor' in line:
        while output and (output[-1].strip().startswith('///') or output[-1].strip().startswith('#[') or output[-1].strip() == ''):
            output.pop()
        # Skip struct + impl block
        while i < len(lines):
            if 'impl PromptCompressor for NoCompressor' in lines[i]:
                brace_depth = 0
                while i < len(lines):
                    if '{' in lines[i]:
                        brace_depth += lines[i].count('{')
                    if '}' in lines[i]:
                        brace_depth -= lines[i].count('}')
                    i += 1
                    if brace_depth <= 0:
                        break
                break
            i += 1
        continue
    
    # Skip ResponseCache trait block
    if 'pub trait ResponseCache' in line:
        while output and (output[-1].strip().startswith('///') or output[-1].strip() == ''):
            output.pop()
        brace_depth = 0
        while i < len(lines):
            if '{' in lines[i]:
                brace_depth += lines[i].count('{')
            if '}' in lines[i]:
                brace_depth -= lines[i].count('}')
            i += 1
            if brace_depth <= 0:
                break
        continue
    
    # Skip DomainClassifier trait block
    if 'pub trait DomainClassifier' in line:
        while output and (output[-1].strip().startswith('///') or output[-1].strip() == ''):
            output.pop()
        brace_depth = 0
        while i < len(lines):
            if '{' in lines[i]:
                brace_depth += lines[i].count('{')
            if '}' in lines[i]:
                brace_depth -= lines[i].count('}')
            i += 1
            if brace_depth <= 0:
                break
        continue
    
    # Skip Storage trait methods related to propositions
    skip_patterns = [
        'add_propositions', 'add_proposition_embeddings', 'list_propositions_by_doc',
        'proposition_search', 'rebuild_proposition_index',
        'add_summary_nodes', 'update_summary_embedding', 'list_summary_nodes',
        'summary_search', 'rebuild_summary_tree',
    ]
    
    should_skip = False
    for pattern in skip_patterns:
        if f'fn {pattern}' in line:
            should_skip = True
            break
    
    if should_skip:
        # Remove preceding doc comments
        while output and (output[-1].strip().startswith('///') or output[-1].strip().startswith('//') or output[-1].strip() == ''):
            output.pop()
        # Skip the method (async fn ... { ... } or fn ... { ... })
        # These are default impls with braces
        brace_depth = 0
        found_brace = False
        while i < len(lines):
            if '{' in lines[i]:
                brace_depth += lines[i].count('{')
                found_brace = True
            if '}' in lines[i]:
                brace_depth -= lines[i].count('}')
            i += 1
            if found_brace and brace_depth <= 0:
                break
            if not found_brace and lines[i-1].strip().endswith(';'):
                # It's a trait method declaration without body
                break
        continue
    
    # Skip ResponseCache methods in Storage trait
    cache_methods = ['lookup_exact', 'lookup_semantic', 'lookup_retrieval', 
                     'insert_exact', 'insert_semantic', 'insert_retrieval',
                     'clear_all', 'get_stats']
    # Only skip if these are in the Storage trait (not ResponseCache which we already removed)
    # Since ResponseCache is already removed, these would be in Storage if they exist
    # Actually, get_stats and clear_all are in ResponseCache trait which we already removed
    # The Storage trait doesn't have these methods directly - they're in ResponseCache
    # So we don't need to skip them here
    
    output.append(line)
    i += 1

with open(filepath, 'w') as f:
    f.writelines(output)

print(f"Done. Original lines: {len(lines)}, Output lines: {len(output)}")
