#!/usr/bin/env python3
"""Fix state.rs: Remove cache/compression/speculative/retrieval_memory/late_chunking fields and init logic"""
import re

filepath = 'crates/tauri-app/src/state.rs'
with open(filepath, 'r') as f:
    content = f.read()

# 1. Remove settings_keys entries for deleted features
content = content.replace(
    '            "rag.speculative_enabled",\n            "rag.retrieval_memory_enabled",\n',
    ''
)
content = content.replace(
    '            "rag.late_chunking",\n',
    ''
)

# 2. Remove cache initialization
content = content.replace(
    '        // 初始化语义缓存（REQ-PERF-001）：共享 SqliteStorage 连接池\n'
    '        let cache = SqliteCache::new(storage.pool_clone()).context("初始化语义缓存失败")?;\n\n',
    ''
)

# 3. Remove compression_ratio, speculative_enabled, retrieval_memory_enabled init
content = content.replace(
    '        // S7: 从批量读取结果解析各设置项（原逐个 get_setting 串行查询）\n'
    '        let compression_ratio = settings_map\n'
    '            .get("compression.ratio")\n'
    '            .and_then(|v| v.parse::<f32>().ok())\n'
    '            .filter(|v| *v >= 1.0 && *v <= 10.0)\n'
    '            .unwrap_or(1.0);\n\n'
    '        let speculative_enabled = settings_map\n'
    '            .get("rag.speculative_enabled")\n'
    '            .is_some_and(|&v| v == "true");\n\n'
    '        let retrieval_memory_enabled = settings_map\n'
    '            .get("rag.retrieval_memory_enabled")\n'
    '            .is_some_and(|&v| v == "true");\n\n'
    '        let graph_retriever_enabled = settings_map\n'
    '            .get("rag.graph_retriever_enabled")\n'
    '            .is_some_and(|&v| v == "true");',
    '        // S7: 从批量读取结果解析各设置项（原逐个 get_setting 串行查询）\n'
    '        let graph_retriever_enabled = settings_map\n'
    '            .get("rag.graph_retriever_enabled")\n'
    '            .is_some_and(|&v| v == "true");'
)

# 4. Remove late_chunking_enabled init
content = content.replace(
    '        let late_chunking_enabled = settings_map\n'
    '            .get("rag.late_chunking")\n'
    '            .is_some_and(|&v| v == "true");\n\n',
    ''
)

# 5. Remove fields from struct initialization
content = content.replace(
    '            cache,\n'
    '            step_cache,\n'
    '            compression_ratio,\n'
    '            speculative_enabled,\n'
    '            retrieval_memory_enabled,\n',
    '            step_cache,\n'
)
content = content.replace(
    '            contextual_retrieval_enabled,\n'
    '            late_chunking_enabled,\n',
    '            contextual_retrieval_enabled,\n'
)

# 6. Fix test module - update parse_settings function
# Remove compression_ratio, speculative_enabled, retrieval_memory_enabled, late_chunking_enabled from return tuple
old_parse = '''    fn parse_settings(
        settings_map: &std::collections::HashMap<&str, &str>,
    ) -> (
        f32,    // compression_ratio
        bool,   // speculative_enabled
        bool,   // retrieval_memory_enabled
        bool,   // graph_retriever_enabled
        bool,   // quality_gate_enabled
        bool,   // memory_enabled
        bool,   // web_search_enabled
        bool,   // contextual_retrieval_enabled
        bool,   // late_chunking_enabled
        String, // log_level
        SecurityPosture,
        String, // local_model
    ) {
        let compression_ratio = settings_map
            .get("compression.ratio")
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|v| *v >= 1.0 && *v <= 10.0)
            .unwrap_or(1.0);

        let speculative_enabled = settings_map
            .get("rag.speculative_enabled")
            .is_some_and(|&v| v == "true");

        let retrieval_memory_enabled = settings_map
            .get("rag.retrieval_memory_enabled")
            .is_some_and(|&v| v == "true");

        let graph_retriever_enabled = settings_map
            .get("rag.graph_retriever_enabled")
            .is_some_and(|&v| v == "true");

        let quality_gate_enabled = settings_map
            .get("rag.quality_gate_enabled")
            .is_some_and(|&v| v == "true");

        let memory_enabled = settings_map
            .get("memory.enabled")
            .is_some_and(|&v| v == "true");

        let web_search_enabled = settings_map
            .get("rag.web_search_enabled")
            .is_some_and(|&v| v == "true");

        let contextual_retrieval_enabled = settings_map
            .get("rag.contextual_retrieval")
            .map(|&v| v != "false")
            .unwrap_or(true);

        let late_chunking_enabled = settings_map
            .get("rag.late_chunking")
            .is_some_and(|&v| v == "true");

        let log_level = settings_map
            .get("log.level")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "info".to_string());

        let security_posture = settings_map
            .get("security.posture")
            .and_then(|v| SecurityPosture::parse_str(v))
            .unwrap_or_default();

        let local_model = settings_map
            .get("llm.local_model")
            .map(|v| v.to_string())
            .unwrap_or_default();

        (
            compression_ratio,
            speculative_enabled,
            retrieval_memory_enabled,
            graph_retriever_enabled,
            quality_gate_enabled,
            memory_enabled,
            web_search_enabled,
            contextual_retrieval_enabled,
            late_chunking_enabled,
            log_level,
            security_posture,
            local_model,
        )
    }'''

new_parse = '''    fn parse_settings(
        settings_map: &std::collections::HashMap<&str, &str>,
    ) -> (
        bool,   // graph_retriever_enabled
        bool,   // quality_gate_enabled
        bool,   // memory_enabled
        bool,   // web_search_enabled
        bool,   // contextual_retrieval_enabled
        String, // log_level
        SecurityPosture,
        String, // local_model
    ) {
        let graph_retriever_enabled = settings_map
            .get("rag.graph_retriever_enabled")
            .is_some_and(|&v| v == "true");

        let quality_gate_enabled = settings_map
            .get("rag.quality_gate_enabled")
            .is_some_and(|&v| v == "true");

        let memory_enabled = settings_map
            .get("memory.enabled")
            .is_some_and(|&v| v == "true");

        let web_search_enabled = settings_map
            .get("rag.web_search_enabled")
            .is_some_and(|&v| v == "true");

        let contextual_retrieval_enabled = settings_map
            .get("rag.contextual_retrieval")
            .map(|&v| v != "false")
            .unwrap_or(true);

        let log_level = settings_map
            .get("log.level")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "info".to_string());

        let security_posture = settings_map
            .get("security.posture")
            .and_then(|v| SecurityPosture::parse_str(v))
            .unwrap_or_default();

        let local_model = settings_map
            .get("llm.local_model")
            .map(|v| v.to_string())
            .unwrap_or_default();

        (
            graph_retriever_enabled,
            quality_gate_enabled,
            memory_enabled,
            web_search_enabled,
            contextual_retrieval_enabled,
            log_level,
            security_posture,
            local_model,
        )
    }'''

content = content.replace(old_parse, new_parse)

# 7. Fix test cases
# TC-BOOT-001
content = content.replace(
    '''        let (comp, spec, mem_retr, graph, gate, mem, web, ctx, late, log, posture, model) =
            parse_settings(&map);

        assert!((comp - 2.5).abs() < 0.01, "compression_ratio 应为 2.5");
        assert!(spec, "speculative_enabled 应为 true");
        assert!(mem_retr, "retrieval_memory_enabled 应为 true");
        assert!(!graph, "graph_retriever_enabled 应为 false");
        assert!(gate, "quality_gate_enabled 应为 true");
        assert!(!mem, "memory_enabled 应为 false");
        assert!(web, "web_search_enabled 应为 true");
        assert!(!ctx, "contextual_retrieval_enabled 应为 false");
        assert!(late, "late_chunking_enabled 应为 true");
        assert_eq!(log, "debug");
        assert_eq!(posture, SecurityPosture::Strict);
        assert_eq!(model, "mistral-7b");''',
    '''        let (graph, gate, mem, web, ctx, log, posture, model) = parse_settings(&map);

        assert!(!graph, "graph_retriever_enabled 应为 false");
        assert!(gate, "quality_gate_enabled 应为 true");
        assert!(!mem, "memory_enabled 应为 false");
        assert!(web, "web_search_enabled 应为 true");
        assert!(!ctx, "contextual_retrieval_enabled 应为 false");
        assert_eq!(log, "debug");
        assert_eq!(posture, SecurityPosture::Strict);
        assert_eq!(model, "mistral-7b");'''
)

# TC-BOOT-002
content = content.replace(
    '''        let (comp, spec, mem_retr, graph, gate, mem, web, ctx, late, log, posture, model) =
            parse_settings(&map);

        assert!((comp - 1.0).abs() < 0.01, "compression_ratio 默认 1.0");
        assert!(!spec, "speculative_enabled 默认 false");
        assert!(!mem_retr, "retrieval_memory_enabled 默认 false");
        assert!(!graph, "graph_retriever_enabled 默认 false");
        assert!(!gate, "quality_gate_enabled 默认 false");
        assert!(!mem, "memory_enabled 默认 false");
        assert!(!web, "web_search_enabled 默认 false");
        assert!(ctx, "contextual_retrieval_enabled 默认 true");
        assert!(!late, "late_chunking_enabled 默认 false");
        assert_eq!(log, "info");
        assert_eq!(posture, SecurityPosture::Auto);
        assert_eq!(model, "");''',
    '''        let (graph, gate, mem, web, ctx, log, posture, model) = parse_settings(&map);

        assert!(!graph, "graph_retriever_enabled 默认 false");
        assert!(!gate, "quality_gate_enabled 默认 false");
        assert!(!mem, "memory_enabled 默认 false");
        assert!(!web, "web_search_enabled 默认 false");
        assert!(ctx, "contextual_retrieval_enabled 默认 true");
        assert_eq!(log, "info");
        assert_eq!(posture, SecurityPosture::Auto);
        assert_eq!(model, "");'''
)

# TC-BOOT-003
content = content.replace(
    '''        let (comp, spec, mem_retr, graph, gate, mem, web, ctx, late, log, posture, model) =
            parse_settings(&map);

        assert!((comp - 1.0).abs() < 0.01, "compression_ratio 默认 1.0");
        assert!(!spec, "speculative_enabled 默认 false");
        assert!(!mem_retr, "retrieval_memory_enabled 默认 false");
        assert!(!graph, "graph_retriever_enabled 默认 false");
        assert!(!gate, "quality_gate_enabled 默认 false");
        assert!(!mem, "memory_enabled 默认 false");
        assert!(!web, "web_search_enabled 默认 false");
        assert!(ctx, "contextual_retrieval_enabled 默认 true");
        assert!(!late, "late_chunking_enabled 默认 false");
        assert_eq!(log, "info");
        assert_eq!(posture, SecurityPosture::Auto);
        assert_eq!(model, "");''',
    '''        let (graph, gate, mem, web, ctx, log, posture, model) = parse_settings(&map);

        assert!(!graph, "graph_retriever_enabled 默认 false");
        assert!(!gate, "quality_gate_enabled 默认 false");
        assert!(!mem, "memory_enabled 默认 false");
        assert!(!web, "web_search_enabled 默认 false");
        assert!(ctx, "contextual_retrieval_enabled 默认 true");
        assert_eq!(log, "info");
        assert_eq!(posture, SecurityPosture::Auto);
        assert_eq!(model, "");'''
)

# TC-BOOT-004
content = content.replace(
    '''        let (comp, spec, mem_retr, graph, gate, mem, web, ctx, late, log, posture, model) =
            parse_settings(&map);

        assert!((comp - 1.0).abs() < 0.01, "compression_ratio 默认 1.0");
        assert!(spec2, "speculative_enabled 从 DB 读取为 true");
        assert!(!mem_retr, "retrieval_memory_enabled 默认 false");
        assert!(!graph, "graph_retriever_enabled 默认 false");
        assert!(!gate, "quality_gate_enabled 默认 false");
        assert!(!mem, "memory_enabled 默认 false");
        assert!(!web, "web_search_enabled 默认 false");
        assert!(ctx, "contextual_retrieval_enabled 默认 true");
        assert!(!late, "late_chunking_enabled 默认 false");
        assert_eq!(log, "info");
        assert_eq!(posture, SecurityPosture::Auto);
        assert_eq!(model, "");''',
    '''        let (graph, gate, mem, web, ctx, log, posture, model) = parse_settings(&map);

        assert!(!graph, "graph_retriever_enabled 默认 false");
        assert!(!gate, "quality_gate_enabled 默认 false");
        assert!(!mem, "memory_enabled 默认 false");
        assert!(!web, "web_search_enabled 默认 false");
        assert!(ctx, "contextual_retrieval_enabled 默认 true");
        assert_eq!(log, "info");
        assert_eq!(posture, SecurityPosture::Auto);
        assert_eq!(model, "");'''
)

# Fix the test map insertions - remove deleted keys
content = content.replace(
    '        map.insert("compression.ratio", "2.5");\n'
    '        map.insert("rag.speculative_enabled", "true");\n'
    '        map.insert("rag.retrieval_memory_enabled", "true");\n',
    ''
)
content = content.replace(
    '        map.insert("rag.late_chunking", "true");\n',
    ''
)
# Fix partial test that references speculative
content = content.replace(
    '        partial.insert("rag.speculative_enabled", "true");\n',
    ''
)

with open(filepath, 'w') as f:
    f.write(content)

print("state.rs fixed")
