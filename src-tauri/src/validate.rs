use saphyr::{AnnotatedMappingOwned, LoadableYamlNode, MarkedYamlOwned, YamlDataOwned};
use serde::Serialize;

/// 一条校验问题。`code` 是 i18n key（前端按 `problems.<code>` 取文案），
/// 后端不产生面向用户的句子。
#[derive(Serialize)]
pub struct Problem {
    pub code: &'static str,
    pub line: usize, // 1-based
    pub col: usize,  // 1-based
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// 顶层故事文件：五个节各归其位。装不进任何字段的顶层 key 即 unknownKey。
/// 定了型的节（story）存具体对象，其余暂以标记树原样保存（保位置）。
// 字段暂无读者：渲染层和节内校验从这里开始消费。
#[allow(dead_code)]
pub struct StoryFile {
    pub story: Option<Story>,
    pub entity: Option<MarkedYamlOwned>,
    pub entity_rel: Option<MarkedYamlOwned>,
    pub events: Option<MarkedYamlOwned>,
    pub narrative: Option<MarkedYamlOwned>,
}

/// story 节：title 必填，description / author 可选。
// 字段暂无读者；字符串类型检查定下后再补。
#[allow(dead_code)]
pub struct Story {
    pub title: String,
    pub description: Option<String>,
    pub author: Option<String>,
}

impl Story {
    /// 从 story 节解析；节不是映射时记一条问题并返回空对象。
    fn from_node(node: MarkedYamlOwned, problems: &mut Vec<Problem>) -> Self {
        let span = node.span;
        let mut title = None;
        let mut story = Self {
            title: String::new(),
            description: None,
            author: None,
        };
        let YamlDataOwned::Mapping(mapping) = node.data else {
            problems.push(Problem {
                code: "structure.sectionMapping",
                line: span.start.line(),
                col: span.start.col() + 1, // saphyr 的列是 0-based
                detail: Some("story".into()),
            });
            return story;
        };
        for (key, value) in mapping {
            match key.data.as_str() {
                Some("title") => title = value.data.as_str().map(str::to_owned),
                Some("description") => story.description = value.data.as_str().map(str::to_owned),
                Some("author") => story.author = value.data.as_str().map(str::to_owned),
                _ => problems.push(Problem {
                    code: "structure.unknownKey",
                    line: key.span.start.line(),
                    col: key.span.start.col() + 1,
                    // 非字符串 key（数字、序列等）没有 detail，行号已指向它。
                    detail: key.data.as_str().map(str::to_owned),
                }),
            }
        }
        // title 必填：缺失时指向 story 块开头。
        story.title = match title {
            Some(t) => t,
            None => {
                problems.push(Problem {
                    code: "structure.missingKey",
                    line: span.start.line(),
                    col: span.start.col() + 1,
                    detail: Some("title".into()),
                });
                String::new()
            }
        };
        story
    }
}

impl StoryFile {
    fn from_mapping(
        mapping: AnnotatedMappingOwned<MarkedYamlOwned>,
        problems: &mut Vec<Problem>,
    ) -> Self {
        let mut file = Self {
            story: None,
            entity: None,
            entity_rel: None,
            events: None,
            narrative: None,
        };
        for (key, value) in mapping {
            match key.data.as_str() {
                Some("story") => file.story = Some(Story::from_node(value, problems)),
                Some("entity") => file.entity = Some(value),
                Some("entity-rel") => file.entity_rel = Some(value),
                Some("events") => file.events = Some(value),
                Some("narrative") => file.narrative = Some(value),
                _ => problems.push(Problem {
                    code: "structure.unknownKey",
                    line: key.span.start.line(),
                    col: key.span.start.col() + 1, // saphyr 的列是 0-based
                    detail: key.data.as_str().map(str::to_owned),
                }),
            }
        }
        file
    }
}

/// 校验一份故事文件，一次收全所有问题。
/// 纯函数：不碰文件系统、不依赖 Tauri，将来编辑器里做行内校验直接调。
pub fn validate(src: &str) -> Vec<Problem> {
    // 语法不过就到此为止：后面的结构、引用规则一概不跑——没有语法树它们无从谈起，
    // 混着报也只会让作者去改错地方。
    let docs = match MarkedYamlOwned::load_from_str(src) {
        Ok(docs) => docs,
        Err(e) => {
            return vec![Problem {
                code: "parse.syntax",
                line: e.marker().line(),
                col: e.marker().col() + 1, // saphyr 的列是 0-based
                // 解析器自己的英文原因：无法翻译，也不值得穷举映射，原样带给前端。
                detail: Some(e.info().to_owned()),
            }];
        }
    };

    // 结构层：解析进 StoryFile，装不进去的顶层 key 都记下来；再往下逐节校验。
    // 标记树（而非 serde）是刻意的选择：deny_unknown_fields 一次只报一个错且
    // 没有行号，收不齐问题清单。
    let mut problems = Vec::new();
    if docs.len() > 1 {
        problems.push(Problem {
            code: "structure.multiDoc",
            line: docs[1].span.start.line(),
            col: docs[1].span.start.col() + 1,
            detail: None,
        });
    }
    let Some(doc) = docs.into_iter().next() else {
        return problems; // 空文件：没有可校验的结构
    };
    if doc.data.is_null() {
        return problems; // 空文档（比如只有注释）：同空文件
    }
    let _file = match doc.data {
        YamlDataOwned::Mapping(mapping) => {
            let file = StoryFile::from_mapping(mapping, &mut problems);
            // story 节承载必填的 title，整节缺失同样记 missingKey，指向文件头。
            if file.story.is_none() {
                problems.push(Problem {
                    code: "structure.missingKey",
                    line: doc.span.start.line(),
                    col: doc.span.start.col() + 1,
                    detail: Some("story".into()),
                });
            }
            file
        }
        _ => {
            problems.push(Problem {
                code: "structure.topLevel",
                line: doc.span.start.line(),
                col: doc.span.start.col() + 1,
                detail: None,
            });
            // 顶层不成立就没有节可谈；空骨架保持后续流程同一形状。
            StoryFile {
                story: None,
                entity: None,
                entity_rel: None,
                events: None,
                narrative: None,
            }
        }
    };
    problems
}
