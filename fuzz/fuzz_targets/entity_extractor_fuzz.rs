//! Fuzz 目标：EntityExtractor 对任意 UTF-8 输入的鲁棒性。
//!
//! 不变量（铁律二.5）：
//! 1. 任意输入不得 panic
//! 2. 返回的实体 text 和 entity_type 非空
//! 3. 关系三元组中的实体引用有效

#![no_main]

use echomind_core::entity_extractor::EntityExtractor;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);

    // 测试 EntityExtractor::extract
    let entities = EntityExtractor::extract(&text);
    for entity in &entities {
        assert!(
            !entity.text.is_empty(),
            "EntityExtractor 返回了空 text"
        );
        assert!(
            !entity.entity_type.is_empty(),
            "EntityExtractor 返回了空 entity_type"
        );
    }

    // 测试 EntityExtractor::extract_relations
    let relations = EntityExtractor::extract_relations(&text, "fuzz-chunk");
    for rel in &relations {
        assert!(
            !rel.subject.is_empty(),
            "extract_relations 返回了空 subject"
        );
        assert!(
            !rel.object.is_empty(),
            "extract_relations 返回了空 object"
        );
        assert!(
            !rel.relation_type.is_empty(),
            "extract_relations 返回了空 relation_type"
        );
    }
});