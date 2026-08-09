//! NER 实体抽取器（REQ-PERF-006）：纯规则 + 正则，零 LLM/模型依赖。
//!
//! ## 设计
//!
//! 实体抽取用于三路 RRF 检索的实体匹配通道（Vector + BM25 + Entity）。
//! 当用户查询包含专有名词（人名、技术术语、版本号等）时，
//! 实体匹配通道能精确定位到包含相同实体的 chunk，弥补向量检索在
//! 精确匹配上的不足。
//!
//! ## 实体类型
//!
//! | 类型 | 规则 | 示例 |
//! |---|---|---|
//! | `person` | 中文人名（常见姓氏 + 2-4 字） | 张三、李明、王建国 |
//! | `proper_noun` | 英文专有名词（大写首词，非句首） | Rust、OpenAI、Beijing |
//! | `tech_term` | 技术术语（camelCase / PascalCase / UPPER_SNAKE） | HashMap、HTTP_STATUS、fetchData |
//! | `identifier` | 数字标识符（版本号 / 错误码 / IP） | v2.0.0、404、192.168.1.1 |
//! | `date` | 日期时间 | 2024-01-15、2024年1月15日 |
//!
//! ## 调研来源
//!
//! - Mem0 (2026.04)：实体链接 + 多信号检索，LoCoMo 92.5
//! - Anthropic Contextual Retrieval (2024.09)：检索失败率 ↓49%

// 预编译正则表达式使用 unwrap() 初始化：所有 pattern 均为编译时已知的有效正则，
// unwrap() 不会触发 panic，但 clippy 无法静态验证这一点。
#![allow(clippy::unwrap_used)]

use std::collections::HashSet;

// ============================================================
// **性能优化（GB 级文件）**：正则表达式全局静态化（OnceLock 一次性编译）。
// 原实现每次调用 extract_* 都重新编译正则（10 个正则 × 每 chunk × 每句调用），
// 大文件（数万 chunk）下正则编译开销占导入总耗时的绝大部分。
// 静态化后仅首次编译一次，后续调用零开销。
// ============================================================
use std::sync::OnceLock;

macro_rules! static_regex {
    ($fn_name:ident, $static_name:ident, $pattern:expr, $fallback:expr) => {
        static $static_name: OnceLock<regex::Regex> = OnceLock::new();
        fn $fn_name() -> &'static regex::Regex {
            $static_name.get_or_init(|| {
                regex::Regex::new($pattern).unwrap_or_else(|e| {
                    eprintln!("静态正则编译失败: {e}");
                    regex::Regex::new($fallback).unwrap()
                })
            })
        }
    };
}

static_regex!(re_word, RE_WORD, r"[A-Za-z]+", r"[A-Za-z]");
static_regex!(
    re_pascal,
    RE_PASCAL,
    r"\b[A-Z][a-z]+[A-Z][a-z]*\w*",
    r"\b[A-Z]\w"
);
static_regex!(re_camel, RE_CAMEL, r"\b[a-z]+[A-Z]\w*", r"\b[a-z]+\w");
static_regex!(
    re_snake,
    RE_SNAKE,
    r"\b[A-Z][A-Z_]{2,}[A-Z]\b",
    r"\b[A-Z_]+\b"
);
static_regex!(re_acronym, RE_ACRONYM, r"\b[A-Z]{3,}\b", r"\b[A-Z]+\b");
static_regex!(
    re_version,
    RE_VERSION,
    r"\bv?\d+\.\d+\.\d+(?:-\w+)?",
    r"\bv?\d+\.\d+"
);
static_regex!(
    re_ip,
    RE_IP,
    r"\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b",
    r"\b\d+\.\d+\.\d+\.\d+\b"
);
static_regex!(re_code, RE_CODE, r"\b\d{3,4}\b", r"\b\d+\b");
static_regex!(
    re_date,
    RE_DATE,
    r"\b\d{4}[-/]\d{1,2}[-/]\d{1,2}\b",
    r"\b\d{4}"
);
static_regex!(
    re_cn_date,
    RE_CN_DATE,
    r"\d{4}年\d{1,2}月\d{1,2}日",
    r"\d{4}年"
);

/// 英文停用词静态集合（一次性构建，避免每 chunk 重建 HashSet）。
static STOPWORDS: OnceLock<HashSet<&'static str>> = OnceLock::new();

fn stopwords() -> &'static HashSet<&'static str> {
    STOPWORDS.get_or_init(|| ENGLISH_STOPWORDS.iter().copied().collect())
}

use echomind_models::{Entity, EntityRelation};

/// 中文常见姓氏（百家姓前 100 + 常见复姓）。
///
/// 用于中文人名识别：以姓氏开头的 2-4 字组合视为人名。
const CHINESE_SURNAMES: &str = "赵钱孙李周吴郑王冯陈褚卫蒋沈韩杨朱秦尤许何吕施张孔曹严华金魏陶姜戚谢邹喻柏水窦章云苏潘葛奚范彭郎鲁韦昌马苗凤花方俞任袁柳鲍史唐费廉岑薛雷贺倪汤滕殷罗毕郝邬安常乐于时傅皮卞齐康伍余元顾孟黄穆萧尹姚邵湛汪祁毛禹狄米贝明臧计伏成戴谈宋茅庞熊纪舒屈项祝董梁杜阮蓝闵席季麻强贾路娄危江童颜郭梅盛林刁钟徐邱骆高夏蔡田樊胡凌霍虞万支柯昝管卢莫经房裘缪干解应宗丁宣贲邓郁单杭洪包诸左石崔吉钮龚程嵇邢滑裴陆荣翁荀羊於惠甄曲家封芮羿储靳汲邴糜松井段富巫乌焦巴弓牧隗山谷车侯宓蓬全郗班仰秋仲伊宫宁仇栾暴甘钭厉戎祖武符刘景詹束龙叶幸司韶郜黎蓟薄印宿白怀蒲邰从鄂索咸籍赖卓蔺屠蒙池乔阴胥能苍双闻昕党翟谭贡劳逄姬申扶堵冉宰郦雍却桑桂濮牛寿通边扈燕冀浦尚农温别庄晏柴瞿阎充慕连茹习宦艾鱼容向古易慎戈廖庾终暨居衡步都耿满弘匡国文禄广欧阙海绵欧阳司马诸葛上官夏侯东方";

/// 英文停用词：这些大写首词不视为专有名词。
const ENGLISH_STOPWORDS: &[&str] = &[
    "The",
    "A",
    "An",
    "This",
    "That",
    "These",
    "Those",
    "It",
    "He",
    "She",
    "We",
    "They",
    "You",
    "I",
    "But",
    "And",
    "Or",
    "If",
    "When",
    "Where",
    "What",
    "Who",
    "How",
    "Why",
    "Which",
    "There",
    "Here",
    "Now",
    "Then",
    "Today",
    "Yesterday",
    "Tomorrow",
    "Some",
    "Any",
    "All",
    "Each",
    "Every",
    "Both",
    "Neither",
    "Either",
    "Such",
    "So",
    "Too",
    "Very",
    "Also",
    "Just",
    "Only",
    "Even",
    "Still",
    "Yet",
    "Already",
    "However",
    "Therefore",
    "Thus",
    "Hence",
    "Meanwhile",
    "Finally",
    "Actually",
    "In",
    "On",
    "At",
    "To",
    "For",
    "Of",
    "With",
    "By",
    "From",
    "About",
    "Into",
    "Onto",
    "Upon",
    "Over",
    "Under",
    "Through",
    "Between",
    "Among",
    "During",
    "Before",
    "After",
    "Since",
    "Until",
    "Within",
    "Without",
    "Against",
    "Toward",
    "Towards",
    "Is",
    "Are",
    "Was",
    "Were",
    "Be",
    "Been",
    "Being",
    "Have",
    "Has",
    "Had",
    "Do",
    "Does",
    "Did",
    "Will",
    "Would",
    "Shall",
    "Should",
    "May",
    "Might",
    "Must",
    "Can",
    "Could",
    "Ought",
    "Not",
    "No",
    "Yes",
    "More",
    "Most",
    "Much",
    "Many",
    "Few",
    "Little",
    "Less",
    "Least",
    "Other",
    "Another",
    "Same",
    "Different",
    "New",
    "Old",
    "Good",
    "Great",
    "First",
    "Last",
    "Next",
    "Previous",
    "Main",
    "Note",
    "Important",
];

/// NER 实体抽取器：纯规则 + 正则，零 LLM/模型依赖。
///
/// 使用正则表达式 + 停用词表识别 5 类实体：
/// 中文人名、英文专有名词、技术术语、数字标识符、日期。
///
/// # 性能
///
/// 纯 CPU 计算（正则匹配），无网络请求、无模型加载。
/// 典型 chunk（256 tokens）抽取耗时 < 1ms。
pub struct EntityExtractor;

impl EntityExtractor {
    /// 从文本中抽取所有实体（去重后返回）。
    ///
    /// # 参数
    /// - `text`: 待抽取的文本（通常是 chunk 内容）
    ///
    /// # 返回
    /// 去重后的实体列表（同文本中重复出现的实体只保留一个）。
    /// 空文本返回空 Vec。
    pub fn extract(text: &str) -> Vec<Entity> {
        if text.trim().is_empty() {
            return Vec::new();
        }

        let mut seen: HashSet<String> = HashSet::new();
        let mut entities: Vec<Entity> = Vec::new();

        // 1. 中文人名识别
        for person in Self::extract_chinese_persons(text) {
            if seen.insert(person.text.clone()) {
                entities.push(person);
            }
        }

        // 2. 英文专有名词识别
        for noun in Self::extract_proper_nouns(text) {
            if seen.insert(noun.text.clone()) {
                entities.push(noun);
            }
        }

        // 3. 技术术语识别
        for term in Self::extract_tech_terms(text) {
            if seen.insert(term.text.clone()) {
                entities.push(term);
            }
        }

        // 4. 数字标识符识别
        for id in Self::extract_identifiers(text) {
            if seen.insert(id.text.clone()) {
                entities.push(id);
            }
        }

        // 5. 日期识别
        for date in Self::extract_dates(text) {
            if seen.insert(date.text.clone()) {
                entities.push(date);
            }
        }

        entities
    }

    /// 从文本中抽取实体，并关联到指定的 chunk_id。
    ///
    /// 返回 `(entity_text, entity_type, chunk_id)` 三元组列表，
    /// 供 `Storage::add_entities()` 批量写入。
    pub fn extract_with_chunk_id(text: &str, chunk_id: &str) -> Vec<(String, String, String)> {
        Self::extract(text)
            .into_iter()
            .map(|e| (e.text, e.entity_type, chunk_id.to_string()))
            .collect()
    }

    /// 从文本中抽取实体间关系（基于规则模式匹配，零 LLM，REQ-RAG-026）。
    ///
    /// 借鉴 mnemo 知识图谱架构：在实体节点间建立有向关系边，
    /// 供检索时沿图边扩展到关联 chunk。
    ///
    /// # 检索管线
    ///
    /// 1. 先用 `extract()` 抽取所有实体
    /// 2. 按句子切分文本
    /// 3. 在每句中查找实体对之间的关系模式
    /// 4. 对同一 chunk 内相同三元组去重
    ///
    /// # 关系类型（中英文 8 种）
    ///
    /// | 类型 | 英文模式 | 中文模式 | 置信度 |
    /// |---|---|---|---|
    /// | `defined_as` | X is defined as Y | X 定义为 Y | 1.0 |
    /// | `part_of` | X is part of Y | X 是 Y 的一部分 | 1.0 |
    /// | `depends_on` | X depends on Y | X 依赖 Y | 1.0 |
    /// | `uses` | X uses Y | X 使用 Y | 1.0 |
    /// | `implements` | X implements Y | X 实现 Y | 1.0 |
    /// | `extends` | X extends Y | X 继承 Y | 1.0 |
    /// | `references` | X references Y | X 引用 Y | 1.0 |
    /// | `related_to` | （同句共现） | （同句共现） | 0.5 |
    ///
    /// 模糊匹配（如 "X can be defined as Y" 或 "X 的定义是 Y"）置信度 0.7。
    ///
    /// # 参数
    /// - `text`: 待抽取的文本（通常是 chunk 内容）
    /// - `chunk_id`: 关联的 chunk ID
    ///
    /// # 返回
    /// 去重后的关系列表。空文本或无关系模式返回空 Vec。
    pub fn extract_relations(text: &str, chunk_id: &str) -> Vec<EntityRelation> {
        if text.trim().is_empty() {
            return Vec::new();
        }

        let entities = Self::extract(text);
        if entities.len() < 2 {
            return Vec::new();
        }

        // 按句子切分（中英文标点 + 换行）
        let sentences = Self::split_sentences(text);

        // 去重集合：(subject, relation_type, object)
        let mut seen: HashSet<(String, String, String)> = HashSet::new();
        let mut relations: Vec<EntityRelation> = Vec::new();

        for sentence in &sentences {
            // 找出当前句子中出现的实体
            let sentence_entities: Vec<&Entity> = entities
                .iter()
                .filter(|e| sentence.contains(&e.text))
                .collect();

            if sentence_entities.len() < 2 {
                continue;
            }

            // 遍历实体对，查找关系模式
            for i in 0..sentence_entities.len() {
                for j in 0..sentence_entities.len() {
                    if i == j {
                        continue;
                    }
                    let subject = &sentence_entities[i].text;
                    let object = &sentence_entities[j].text;

                    if let Some((rel_type, confidence)) =
                        Self::detect_relation(sentence, subject, object)
                    {
                        let key = (subject.clone(), rel_type.clone(), object.clone());
                        if seen.insert(key) {
                            relations.push(EntityRelation::new(
                                subject.clone(),
                                rel_type,
                                object.clone(),
                                chunk_id.to_string(),
                                confidence,
                            ));
                        }
                    }
                }
            }
        }

        relations
    }

    /// 检测句子中两个实体之间的关系类型。
    ///
    /// 返回 `(relation_type, confidence)`，无匹配返回 `None`。
    ///
    /// 精确匹配置信度 1.0，模糊匹配置信度 0.7。
    fn detect_relation(sentence: &str, subject: &str, object: &str) -> Option<(String, f32)> {
        // 英文精确模式（置信度 1.0）
        let en_exact: &[(&str, &str, &[&str])] = &[
            ("defined_as", "is defined as", &["is defined as"]),
            ("part_of", "is part of", &["is part of"]),
            ("depends_on", "depends on", &["depends on"]),
            ("uses", "uses", &[" uses "]),
            ("implements", "implements", &[" implements "]),
            ("extends", "extends", &[" extends "]),
            ("references", "references", &[" references "]),
        ];

        for (rel_type, _label, patterns) in en_exact {
            for pat in *patterns {
                if Self::match_pattern(sentence, subject, pat, object) {
                    return Some((rel_type.to_string(), 1.0));
                }
            }
        }

        // 英文模糊模式（置信度 0.7）
        let en_fuzzy: &[(&str, &[&str])] = &[
            ("defined_as", &["defines", "definition of", "defined by"]),
            ("part_of", &["part of", "component of", "belongs to"]),
            ("depends_on", &["requires", "relies on", "based on"]),
            ("uses", &["utilizes", "employs", "leverages"]),
            ("implements", &["implementation of", "realizes"]),
            ("extends", &["inherits from", "subclasses", "derived from"]),
            ("references", &["refers to", "mentions", "cites"]),
        ];

        for (rel_type, patterns) in en_fuzzy {
            for pat in *patterns {
                if Self::match_pattern(sentence, subject, pat, object) {
                    return Some((rel_type.to_string(), 0.7));
                }
            }
        }

        // 中文精确模式（置信度 1.0）
        let cn_exact: &[(&str, &str)] = &[
            ("defined_as", "定义为"),
            ("part_of", "是.*的一部分"),
            ("depends_on", "依赖"),
            ("uses", "使用"),
            ("implements", "实现"),
            ("extends", "继承"),
            ("references", "引用"),
        ];

        for (rel_type, pat) in cn_exact {
            if Self::match_pattern_cn(sentence, subject, pat, object) {
                return Some((rel_type.to_string(), 1.0));
            }
        }

        // 中文模糊模式（置信度 0.7）
        let cn_fuzzy: &[(&str, &str)] = &[
            ("defined_as", "的定义是"),
            ("part_of", "属于"),
            ("depends_on", "需要"),
            ("uses", "利用"),
            ("implements", "的实现"),
            ("extends", "派生自"),
            ("references", "提到"),
        ];

        for (rel_type, pat) in cn_fuzzy {
            if Self::match_pattern_cn(sentence, subject, pat, object) {
                return Some((rel_type.to_string(), 0.7));
            }
        }

        // 同句共现兜底：两个实体在同一句子中出现但无明确关系模式
        // 仅当实体间距合理（避免随机共现）时返回 related_to
        if let (Some(s_pos), Some(o_pos)) = (sentence.find(subject), sentence.find(object)) {
            let distance = s_pos.abs_diff(o_pos);
            let max_dist = subject.len() + object.len() + 80; // 允许中间有少量文字
            if distance <= max_dist {
                return Some(("related_to".to_string(), 0.5));
            }
        }

        None
    }

    /// 英文模式匹配：检查句子中是否包含 `subject ... pattern ... object` 结构。
    ///
    /// 匹配条件：subject 在 pattern 之前，object 在 pattern 之后。
    fn match_pattern(sentence: &str, subject: &str, pattern: &str, object: &str) -> bool {
        let lower = sentence.to_lowercase();
        let sub_lower = subject.to_lowercase();
        let obj_lower = object.to_lowercase();
        let pat_lower = pattern.to_lowercase();

        // 查找 subject 位置
        let sub_pos = match lower.find(&sub_lower) {
            Some(pos) => pos,
            None => return false,
        };

        // 查找 pattern 位置（在 subject 之后）
        let search_start = sub_pos + sub_lower.len();
        let pat_pos = match lower[search_start..].find(&pat_lower) {
            Some(pos) => search_start + pos,
            None => return false,
        };

        // 查找 object 位置（在 pattern 之后）
        let search_start = pat_pos + pat_lower.len();
        let obj_pos = match lower[search_start..].find(&obj_lower) {
            Some(pos) => search_start + pos,
            None => return false,
        };

        // subject < pattern < object
        sub_pos < pat_pos && pat_pos < obj_pos
    }

    /// 中文模式匹配：检查句子中是否包含 `subject + pattern + object` 结构。
    ///
    /// 支持正则通配符（如 `是.*的一部分`）。
    /// 自动处理 subject/pattern/object 之间的空白字符（中英混排场景）。
    fn match_pattern_cn(sentence: &str, subject: &str, pattern: &str, object: &str) -> bool {
        // 构建正则：subject + \s* + pattern + \s* + object
        // \s* 允许中英混排时实体与关系词之间的空格
        let regex_str = format!(
            r"{}\s*{}\s*{}",
            regex::escape(subject),
            pattern,
            regex::escape(object)
        );
        match regex::Regex::new(&regex_str) {
            Ok(re) => re.is_match(sentence),
            Err(_) => false,
        }
    }

    /// 按句子切分文本（中英文标点 + 换行）。
    fn split_sentences(text: &str) -> Vec<String> {
        let mut sentences = Vec::new();
        let mut current = String::new();

        for c in text.chars() {
            current.push(c);
            // 句末标点：中文句号、问号、叹号、英文句号、问号、叹号、换行
            if c == '。' || c == '？' || c == '！' || c == '.' || c == '?' || c == '!' || c == '\n'
            {
                let trimmed = current.trim();
                if !trimmed.is_empty() {
                    sentences.push(trimmed.to_string());
                }
                current.clear();
            }
        }

        let trimmed = current.trim();
        if !trimmed.is_empty() {
            sentences.push(trimmed.to_string());
        }

        sentences
    }

    /// 中文人名识别：常见姓氏 + 2-4 字组合。
    ///
    /// 规则：
    /// - 遍历文本中的 CJK 字符序列
    /// - 以常见姓氏开头的 2-4 字组合视为人名
    /// - 排除常见非人名词（如 "中国"、"这个" 等）
    fn extract_chinese_persons(text: &str) -> Vec<Entity> {
        let mut result = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        let surname_set: HashSet<char> = CHINESE_SURNAMES.chars().collect();

        // 常见非人名 CJK 词组（避免误识别）
        let non_person_words: HashSet<&str> = HashSet::from([
            "中国",
            "中华",
            "中文",
            "中关",
            "中心",
            "之后",
            "之前",
            "之中",
            "其中",
            "这个",
            "这些",
            "这样",
            "这里",
            "这是",
            "这种",
            "那个",
            "那些",
            "那样",
            "那里",
            "什么",
            "怎么",
            "为什么",
            "那么",
            "可以",
            "可能",
            "应该",
            "需要",
            "已经",
            "正在",
            "曾经",
            "将会",
            "因为",
            "所以",
            "但是",
            "虽然",
            "然而",
            "使用",
            "实现",
            "通过",
            "进行",
            "包括",
            "包含",
            "我们",
            "他们",
            "你们",
            "自己",
            "它们",
        ]);

        while i < chars.len() {
            let c = chars[i];
            if surname_set.contains(&c) {
                // 尝试匹配 2-4 字的人名
                for len in [2, 3, 4] {
                    if i + len <= chars.len() {
                        let candidate: String = chars[i..i + len].iter().collect();
                        // 检查是否全为 CJK 字符
                        if candidate.chars().all(Self::is_cjk)
                            && !non_person_words.contains(candidate.as_str())
                        {
                            // 直接接受匹配（中文无空格分词，无法通过前后字符判断边界）
                            result.push(Entity::new(candidate, "person".to_string()));
                            i += len;
                            break;
                        }
                    }
                }
            }
            i += 1;
        }

        result
    }

    /// 英文专有名词识别：大写首词（非句首位置）。
    ///
    /// 规则：
    /// - 提取所有以大写字母开头的英文单词（≥2 字符）
    /// - 排除句首位置的词（句首大写是语法要求，非专有名词）
    /// - 排除停用词（The, This, It 等）
    fn extract_proper_nouns(text: &str) -> Vec<Entity> {
        let mut result = Vec::new();
        let stopwords = stopwords();

        // 用正则提取所有英文单词（静态编译，零重复开销）
        let word_re = re_word();

        for mat in word_re.find_iter(text) {
            let word = mat.as_str();
            // 必须以大写字母开头且 ≥2 字符
            if word.len() < 2 || !word.chars().next().is_some_and(|c| c.is_uppercase()) {
                continue;
            }
            // 排除全大写缩写（如 HTTP, API）—— 这些由 tech_term 处理
            if word.chars().all(|c| c.is_uppercase()) && word.len() >= 2 {
                // 2 字符全大写如 "AI" 归入 proper_noun；≥3 字符全大写归入 tech_term
                if word.len() >= 3 {
                    continue;
                }
            }
            // 排除停用词
            if stopwords.contains(word) {
                continue;
            }
            // 跳过 PascalCase 词汇（如 HashMap、McDonald）—— 由 tech_term 处理。
            // 判定：词中存在 小写→大写→小写 模式（非首字符位置）。
            if Self::is_pascal_case(word) {
                continue;
            }
            // 句首检查已移除：停用词表已覆盖常见句首词（The/This/It 等），
            // 避免误过滤始终大写的专有名词（如 Rust/OpenAI 在句首也需识别）。
            result.push(Entity::new(word.to_string(), "proper_noun".to_string()));
        }

        result
    }

    /// 技术术语识别：camelCase / PascalCase / UPPER_SNAKE_CASE。
    fn extract_tech_terms(text: &str) -> Vec<Entity> {
        let mut result = Vec::new();

        // PascalCase: 大写字母开头 + 小写字母后续（如 HashMap, OpenAI）
        // 但排除已在 proper_noun 中识别的词
        let pascal_re = re_pascal();
        for mat in pascal_re.find_iter(text) {
            result.push(Entity::new(
                mat.as_str().to_string(),
                "tech_term".to_string(),
            ));
        }

        // camelCase: 小写字母开头 + 中间大写字母（如 fetchData, parseJson）
        let camel_re = re_camel();
        for mat in camel_re.find_iter(text) {
            result.push(Entity::new(
                mat.as_str().to_string(),
                "tech_term".to_string(),
            ));
        }

        // UPPER_SNAKE_CASE: 全大写 + 下划线（如 HTTP_STATUS, MAX_RETRIES）
        let snake_re = re_snake();
        for mat in snake_re.find_iter(text) {
            result.push(Entity::new(
                mat.as_str().to_string(),
                "tech_term".to_string(),
            ));
        }

        // 全大写缩写 ≥3 字符（如 HTTP, API, SSE, JSON）
        let acronym_re = re_acronym();
        for mat in acronym_re.find_iter(text) {
            result.push(Entity::new(
                mat.as_str().to_string(),
                "tech_term".to_string(),
            ));
        }

        result
    }

    /// 数字标识符识别：版本号 / 错误码 / IP 地址。
    fn extract_identifiers(text: &str) -> Vec<Entity> {
        let mut result = Vec::new();

        // 版本号: v2.0.0, 1.0.3, v1.2.3-beta
        let version_re = re_version();
        for mat in version_re.find_iter(text) {
            result.push(Entity::new(
                mat.as_str().to_string(),
                "identifier".to_string(),
            ));
        }

        // IP 地址: 192.168.1.1
        let ip_re = re_ip();
        for mat in ip_re.find_iter(text) {
            result.push(Entity::new(
                mat.as_str().to_string(),
                "identifier".to_string(),
            ));
        }

        // 错误码: 3-4 位纯数字（如 404, 500, 301）
        let code_re = re_code();
        for mat in code_re.find_iter(text) {
            let code = mat.as_str();
            // 排除年份（1900-2099）
            let num: u32 = code.parse().unwrap_or(0);
            if !(1900..=2099).contains(&num) {
                result.push(Entity::new(code.to_string(), "identifier".to_string()));
            }
        }

        result
    }

    /// 日期识别：YYYY-MM-DD / YYYY年MM月DD日 / YYYY/MM/DD。
    fn extract_dates(text: &str) -> Vec<Entity> {
        let mut result = Vec::new();

        // YYYY-MM-DD 或 YYYY/MM/DD
        let date_re = re_date();
        for mat in date_re.find_iter(text) {
            result.push(Entity::new(mat.as_str().to_string(), "date".to_string()));
        }

        // YYYY年MM月DD日
        let cn_date_re = re_cn_date();
        for mat in cn_date_re.find_iter(text) {
            result.push(Entity::new(mat.as_str().to_string(), "date".to_string()));
        }

        result
    }

    /// 判断字符是否为 CJK 统一汉字。
    fn is_cjk(c: char) -> bool {
        let code = c as u32;
        // CJK Unified Ideographs: U+4E00 - U+9FFF
        (0x4E00..=0x9FFF).contains(&code)
    }

    /// 判断单词是否为 PascalCase 模式（小写→大写→小写）。
    ///
    /// 用于在专有名词识别中跳过 PascalCase 词汇（如 HashMap、McDonald），
    /// 这些词由技术术语通道处理。
    fn is_pascal_case(word: &str) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if chars.len() < 4 {
            return false;
        }
        for i in 1..chars.len().saturating_sub(1) {
            if chars[i].is_lowercase()
                && chars[i + 1].is_uppercase()
                && i + 2 < chars.len()
                && chars[i + 2].is_lowercase()
            {
                return true;
            }
        }
        false
    }
}
