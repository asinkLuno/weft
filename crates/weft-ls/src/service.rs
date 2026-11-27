//! 纯 Rust 语言服务接口：供 `weft lsp`（stdio）与 Tauri 桌面端（进程内）共享。

use std::path::Path;

use serde::{Deserialize, Serialize};
use weft_core::compiler::{CheckOptions, WeftDoc, check_document};
use weft_core::errors::{ByteSpan, Diagnostic, Severity};
use weft_core::format::format_weft;
use weft_core::parser::is_ident_continue;

use crate::index::{DocumentIndex, SymbolKind};
use crate::position::{
    Utf16Range, clamp_char_boundary, escape_string_inner, expand_to_full_line, format_weft_name,
    span_to_utf16,
};
use crate::scanner::{CursorContext, CursorString, ScanSymbols, TimeFieldKind, scan_document, tokenize};
use crate::schema::{AttrBlockKind, fields_for, find_field, keyword_doc};

// ---- 面向前端与 LSP 共享的数据模型 ----

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LsSeverity {
    Error,
    Warning,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LsTextEdit {
    pub span: ByteSpan,
    pub utf16: Utf16Range,
    pub new_text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LsCodeAction {
    pub title: String,
    pub edits: Vec<LsTextEdit>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LsRelated {
    pub span: ByteSpan,
    pub utf16: Utf16Range,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LsDiagnostic {
    pub severity: LsSeverity,
    pub rule: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<ByteSpan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub utf16: Option<Utf16Range>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub related: Vec<LsRelated>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub quick_fixes: Vec<LsCodeAction>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LsCompletionKind {
    Keyword,
    Property,
    Entity,
    Event,
    TimeAnchor,
    CalculableProp,
    Relation,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LsCompletionItem {
    pub label: String,
    pub kind: LsCompletionKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,
    pub insert_text: String,
    pub is_snippet: bool,
    pub sort_priority: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LsCompletionList {
    pub replace_span: ByteSpan,
    pub replace_utf16: Utf16Range,
    pub items: Vec<LsCompletionItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LsHover {
    pub contents: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<ByteSpan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub utf16: Option<Utf16Range>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LsLocation {
    pub span: ByteSpan,
    pub utf16: Utf16Range,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LsDocumentSymbol {
    pub kind: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub name_span: ByteSpan,
    pub full_span: ByteSpan,
    pub utf16_name: Utf16Range,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LsPrepareRename {
    pub span: ByteSpan,
    pub utf16: Utf16Range,
    pub placeholder: String,
}

// ---- 诊断与 Quick Fix ----

pub fn convert_diagnostic(src: &str, diag: &Diagnostic, lang: &str) -> LsDiagnostic {
    let quick_fixes = quick_fixes_for_diagnostic(src, diag, lang);
    LsDiagnostic {
        severity: match diag.severity {
            Severity::Error => LsSeverity::Error,
            Severity::Warning => LsSeverity::Warning,
        },
        rule: diag.rule.to_owned(),
        message: diag.message.clone(),
        span: diag.span,
        utf16: diag.span.map(|s| span_to_utf16(src, s)),
        help: diag.help.clone(),
        related: diag
            .related
            .iter()
            .map(|r| LsRelated {
                span: r.span,
                utf16: span_to_utf16(src, r.span),
                message: r.message.clone(),
            })
            .collect(),
        quick_fixes,
    }
}

pub fn diagnostics(src: &str, base: &Path, lang: &str, run_scripts: bool) -> Vec<LsDiagnostic> {
    let out = check_document(
        src,
        base,
        &CheckOptions {
            run_scripts,
            lang: lang.to_owned(),
        },
    );
    out.diagnostics
        .iter()
        .map(|d| convert_diagnostic(src, d, lang))
        .collect()
}

pub fn quick_fixes_for_diagnostic(src: &str, diag: &Diagnostic, lang: &str) -> Vec<LsCodeAction> {
    let mut actions = Vec::new();
    let Some(span) = diag.span else {
        return actions;
    };
    let zh = lang == "zh";

    if let Some(suggestion) = &diag.suggestion {
        let replacement = format_weft_name(suggestion);
        let title = if zh {
            format!("替换为 `{replacement}`")
        } else {
            format!("Replace with `{replacement}`")
        };
        actions.push(LsCodeAction {
            title,
            edits: vec![LsTextEdit {
                span,
                utf16: span_to_utf16(src, span),
                new_text: replacement,
            }],
        });
    }

    match diag.rule {
        "self-link" | "duplicate-link" => {
            let line_span = expand_to_full_line(src, span);
            let title = if zh {
                if diag.rule == "self-link" {
                    "删除自环关系边"
                } else {
                    "删除重复关系边"
                }
            } else if diag.rule == "self-link" {
                "Remove self-link edge"
            } else {
                "Remove duplicate link edge"
            };
            actions.push(LsCodeAction {
                title: title.to_owned(),
                edits: vec![LsTextEdit {
                    span: line_span,
                    utf16: span_to_utf16(src, line_span),
                    new_text: String::new(),
                }],
            });
        }
        "unused-time-anchor" => {
            let line_span = expand_to_full_line(src, span);
            let title = if zh {
                "删除未使用的 time 锚点"
            } else {
                "Remove unused time anchor"
            };
            actions.push(LsCodeAction {
                title: title.to_owned(),
                edits: vec![LsTextEdit {
                    span: line_span,
                    utf16: span_to_utf16(src, line_span),
                    new_text: String::new(),
                }],
            });
        }
        _ => {}
    }

    actions
}

pub fn code_actions(src: &str, base: &Path, range: ByteSpan, lang: &str) -> Vec<LsCodeAction> {
    let out = check_document(
        src,
        base,
        &CheckOptions {
            run_scripts: true,
            lang: lang.to_owned(),
        },
    );
    let mut actions = Vec::new();
    for diag in &out.diagnostics {
        let overlaps = diag.span.is_some_and(|s| s.start <= range.end && range.start <= s.end);
        if overlaps {
            actions.extend(quick_fixes_for_diagnostic(src, diag, lang));
        }
    }
    actions
}

// ---- 补全 ----

pub fn completions(
    src: &str,
    cursor_byte: usize,
    lang: &str,
    explicit: bool,
) -> Option<LsCompletionList> {
    let pos = clamp_char_boundary(src, cursor_byte);
    let scanned = tokenize(src, pos);
    if scanned.cursor_in_comment {
        return None;
    }

    let (symbols, ctx) = scan_document(
        &scanned.tokens,
        pos,
        scanned.cursor_line,
        scanned.cursor_in_string,
    );
    if matches!(ctx, CursorContext::None) {
        return None;
    }

    let replace_start = match scanned.cursor_in_string {
        Some(cs) => (cs.span.start + 1).min(pos),
        None => ident_prefix_start(src, pos),
    };
    let has_prefix = pos > replace_start;

    if !explicit
        && !matches!(ctx, CursorContext::Member { .. })
        && scanned.cursor_in_string.is_none()
        && !has_prefix
    {
        return None;
    }

    let mut items = build_completion_items(&ctx, &symbols, lang, scanned.cursor_in_string);
    if items.is_empty() {
        return None;
    }

    items.sort_by(|a, b| {
        b.sort_priority
            .cmp(&a.sort_priority)
            .then_with(|| a.label.cmp(&b.label))
    });

    let has_closing_quote = scanned.cursor_in_string.is_some_and(|cs| {
        !cs.multiline && cs.closed && cs.span.end > pos && src.as_bytes()[cs.span.end - 1] == b'"'
    });
    let replace_end = if has_closing_quote {
        scanned.cursor_in_string.unwrap().span.end
    } else {
        pos
    };
    let replace_span = ByteSpan::new(replace_start, replace_end);

    Some(LsCompletionList {
        replace_span,
        replace_utf16: span_to_utf16(src, replace_span),
        items,
    })
}

fn ident_prefix_start(src: &str, pos: usize) -> usize {
    let mut start = pos;
    for (idx, ch) in src[..pos].char_indices().rev() {
        if is_ident_continue(ch) {
            start = idx;
        } else {
            break;
        }
    }
    start
}

fn apply_name(name: &str, in_string: Option<CursorString>, suffix: &str) -> String {
    if in_string.is_some() {
        format!("{}\"{suffix}", escape_string_inner(name))
    } else {
        format!("{}{suffix}", format_weft_name(name))
    }
}

fn build_completion_items(
    ctx: &CursorContext,
    symbols: &ScanSymbols,
    lang: &str,
    in_string: Option<CursorString>,
) -> Vec<LsCompletionItem> {
    let zh = lang == "zh";
    match ctx {
        CursorContext::None => Vec::new(),
        CursorContext::Top => top_completions(symbols, zh),
        CursorContext::AttrKey {
            block,
            existing_keys,
        } => attr_key_completions(*block, existing_keys, lang),
        CursorContext::Member { base } => member_completions(base, symbols, zh),
        CursorContext::EntityRef { exclude } => symbols
            .entities
            .values()
            .filter(|e| !exclude.contains(&e.name))
            .map(|e| LsCompletionItem {
                label: e.name.clone(),
                kind: LsCompletionKind::Entity,
                detail: Some(if zh { "实体" } else { "entity" }.to_owned()),
                documentation: e.description.clone(),
                insert_text: apply_name(&e.name, in_string, ""),
                is_snippet: false,
                sort_priority: 0,
            })
            .collect(),
        CursorContext::EventRef { exclude } => symbols
            .events
            .values()
            .filter(|e| !exclude.contains(&e.name))
            .map(|e| LsCompletionItem {
                label: e.name.clone(),
                kind: LsCompletionKind::Event,
                detail: Some(if zh { "事件" } else { "event" }.to_owned()),
                documentation: e.description.clone(),
                insert_text: apply_name(&e.name, in_string, ""),
                is_snippet: false,
                sort_priority: 0,
            })
            .collect(),
        CursorContext::CalcPropRef { exclude } => symbols
            .calculable_props
            .iter()
            .filter(|p| !exclude.contains(*p))
            .map(|p| LsCompletionItem {
                label: p.clone(),
                kind: LsCompletionKind::CalculableProp,
                detail: Some(
                    if zh { "可计算属性" } else { "calculable prop" }.to_owned(),
                ),
                documentation: None,
                insert_text: apply_name(p, in_string, ""),
                is_snippet: false,
                sort_priority: 0,
            })
            .collect(),
        CursorContext::RelationRef => symbols
            .relations
            .iter()
            .map(|rel| LsCompletionItem {
                label: rel.clone(),
                kind: LsCompletionKind::Relation,
                detail: Some(if zh { "已有关系" } else { "relation" }.to_owned()),
                documentation: None,
                insert_text: if in_string.is_some() {
                    format!("{}\"", escape_string_inner(rel))
                } else {
                    format!("\"{}\"", escape_string_inner(rel))
                },
                is_snippet: false,
                sort_priority: 0,
            })
            .collect(),
        CursorContext::TimeExpr { self_event, field } => {
            time_expr_completions(self_event.as_deref(), *field, symbols, zh, in_string)
        }
    }
}

fn top_completions(symbols: &ScanSymbols, zh: bool) -> Vec<LsCompletionItem> {
    let story_snippet = match (symbols.has_story, zh) {
        (true, true) => "story {\n  title = \"${1:标题}\"\n}",
        (false, true) => "story {\n  title  = \"${1:标题}\"\n  author = \"${2:作者}\"\n}",
        (true, false) => "story {\n  title = \"${1:Title}\"\n}",
        (false, false) => "story {\n  title  = \"${1:Title}\"\n  author = \"${2:Author}\"\n}",
    };
    vec![
        LsCompletionItem {
            label: "story".to_owned(),
            kind: LsCompletionKind::Keyword,
            detail: Some(
                if zh {
                    "故事元信息块（必填）"
                } else {
                    "Story metadata block (required)"
                }
                .to_owned(),
            ),
            documentation: None,
            insert_text: story_snippet.to_owned(),
            is_snippet: true,
            sort_priority: if symbols.has_story { -2 } else { 10 },
        },
        LsCompletionItem {
            label: "entity".to_owned(),
            kind: LsCompletionKind::Keyword,
            detail: Some(
                if zh {
                    "实体块（人物/团体/地点/道具）"
                } else {
                    "Entity block (person/group/place/item)"
                }
                .to_owned(),
            ),
            documentation: None,
            insert_text: if zh {
                "entity ${1:名称} {\n  description = \"$2\"\n}"
            } else {
                "entity ${1:name} {\n  description = \"$2\"\n}"
            }
            .to_owned(),
            is_snippet: true,
            sort_priority: 3,
        },
        LsCompletionItem {
            label: "event".to_owned(),
            kind: LsCompletionKind::Keyword,
            detail: Some(if zh { "事件块" } else { "Event block" }.to_owned()),
            documentation: None,
            insert_text: if zh {
                "event ${1:事件名} {\n  start    = [${2:1991}, ${3:9}, ${4:1}]\n  entities = [$0]\n}"
            } else {
                "event ${1:event_name} {\n  start    = [${2:1991}, ${3:9}, ${4:1}]\n  entities = [$0]\n}"
            }
            .to_owned(),
            is_snippet: true,
            sort_priority: 2,
        },
        LsCompletionItem {
            label: "link".to_owned(),
            kind: LsCompletionKind::Keyword,
            detail: Some(
                if zh { "实体关系组块" } else { "Grouped entity links" }.to_owned(),
            ),
            documentation: None,
            insert_text: if zh {
                "link ${1:关系组} {\n  ${2:实体A} -- \"${3:关系}\" -- ${4:实体B}\n}"
            } else {
                "link ${1:group} {\n  ${2:entity_a} -- \"${3:relation}\" -- ${4:entity_b}\n}"
            }
            .to_owned(),
            is_snippet: true,
            sort_priority: 1,
        },
        LsCompletionItem {
            label: "events".to_owned(),
            kind: LsCompletionKind::Keyword,
            detail: Some(
                if zh { "事件列表声明" } else { "Events declaration" }.to_owned(),
            ),
            documentation: None,
            insert_text: if zh {
                "events ${1:事件列表} = [$2]"
            } else {
                "events ${1:name} = [$2]"
            }
            .to_owned(),
            is_snippet: true,
            sort_priority: 1,
        },
        LsCompletionItem {
            label: "time".to_owned(),
            kind: LsCompletionKind::Keyword,
            detail: Some(
                if zh {
                    "独立命名时间锚点"
                } else {
                    "Named time anchor"
                }
                .to_owned(),
            ),
            documentation: None,
            insert_text: if zh {
                "time ${1:锚点名} = [${2:1991}, ${3:9}, ${4:1}]"
            } else {
                "time ${1:anchor_name} = [${2:1991}, ${3:9}, ${4:1}]"
            }
            .to_owned(),
            is_snippet: true,
            sort_priority: 0,
        },
    ]
}

fn attr_key_completions(
    block: AttrBlockKind,
    existing: &std::collections::BTreeSet<String>,
    lang: &str,
) -> Vec<LsCompletionItem> {
    fields_for(block)
        .iter()
        .map(|spec| LsCompletionItem {
            label: spec.name.to_owned(),
            kind: LsCompletionKind::Property,
            detail: Some(spec.short(lang).to_owned()),
            documentation: Some(spec.doc(lang).to_owned()),
            insert_text: spec.snippet.to_owned(),
            is_snippet: true,
            sort_priority: if existing.contains(spec.name) { -5 } else { 2 },
        })
        .collect()
}

fn member_completions(base: &str, symbols: &ScanSymbols, zh: bool) -> Vec<LsCompletionItem> {
    let start_detail = if zh { "事件开始时间" } else { "event start time" };
    let end_detail = if zh { "事件结束时间" } else { "event end time" };
    let base_detail = if zh { "实体基准时间" } else { "entity base_time" };

    if let Some(ev) = symbols.events.get(base) {
        return vec![
            LsCompletionItem {
                label: "start".to_owned(),
                kind: LsCompletionKind::Property,
                detail: Some(start_detail.to_owned()),
                documentation: None,
                insert_text: "start".to_owned(),
                is_snippet: false,
                sort_priority: 2,
            },
            LsCompletionItem {
                label: "end".to_owned(),
                kind: LsCompletionKind::Property,
                detail: Some(end_detail.to_owned()),
                documentation: None,
                insert_text: "end".to_owned(),
                is_snippet: false,
                sort_priority: if ev.has_end { 1 } else { -1 },
            },
        ];
    }

    if symbols.entities.contains_key(base) {
        return vec![LsCompletionItem {
            label: "base_time".to_owned(),
            kind: LsCompletionKind::Property,
            detail: Some(base_detail.to_owned()),
            documentation: None,
            insert_text: "base_time".to_owned(),
            is_snippet: false,
            sort_priority: 2,
        }];
    }

    vec![
        LsCompletionItem {
            label: "start".to_owned(),
            kind: LsCompletionKind::Property,
            detail: Some(start_detail.to_owned()),
            documentation: None,
            insert_text: "start".to_owned(),
            is_snippet: false,
            sort_priority: 0,
        },
        LsCompletionItem {
            label: "end".to_owned(),
            kind: LsCompletionKind::Property,
            detail: Some(end_detail.to_owned()),
            documentation: None,
            insert_text: "end".to_owned(),
            is_snippet: false,
            sort_priority: 0,
        },
        LsCompletionItem {
            label: "base_time".to_owned(),
            kind: LsCompletionKind::Property,
            detail: Some(base_detail.to_owned()),
            documentation: None,
            insert_text: "base_time".to_owned(),
            is_snippet: false,
            sort_priority: 0,
        },
    ]
}

fn time_expr_completions(
    self_event: Option<&str>,
    field: TimeFieldKind,
    symbols: &ScanSymbols,
    zh: bool,
    in_string: Option<CursorString>,
) -> Vec<LsCompletionItem> {
    let mut out = Vec::new();
    if in_string.is_none() && field == TimeFieldKind::End && self_event.is_some() {
        out.push(LsCompletionItem {
            label: "start + [...]".to_owned(),
            kind: LsCompletionKind::Keyword,
            detail: Some(
                if zh {
                    "在当前事件开始时间上偏移"
                } else {
                    "offset from current start time"
                }
                .to_owned(),
            ),
            documentation: None,
            insert_text: "start + [0, 0, 0, ${1:2}]".to_owned(),
            is_snippet: true,
            sort_priority: 10,
        });
        out.push(LsCompletionItem {
            label: "start".to_owned(),
            kind: LsCompletionKind::Keyword,
            detail: Some(
                if zh {
                    "当前事件开始时间"
                } else {
                    "current event start time"
                }
                .to_owned(),
            ),
            documentation: None,
            insert_text: "start".to_owned(),
            is_snippet: false,
            sort_priority: 8,
        });
    }

    for anchor in &symbols.time_anchors {
        out.push(LsCompletionItem {
            label: anchor.clone(),
            kind: LsCompletionKind::TimeAnchor,
            detail: Some(if zh { "时间锚点" } else { "time anchor" }.to_owned()),
            documentation: None,
            insert_text: apply_name(anchor, in_string, ""),
            is_snippet: false,
            sort_priority: 4,
        });
    }

    for ev in symbols.events.values() {
        if field == TimeFieldKind::Start && self_event == Some(ev.name.as_str()) {
            continue;
        }
        out.push(LsCompletionItem {
            label: format!("{}.start", ev.name),
            kind: LsCompletionKind::Event,
            detail: Some(if zh { "事件开始时间" } else { "event start time" }.to_owned()),
            documentation: ev.description.clone(),
            insert_text: apply_name(&ev.name, in_string, ".start"),
            is_snippet: false,
            sort_priority: 2,
        });
        if ev.has_end && !(field == TimeFieldKind::End && self_event == Some(ev.name.as_str())) {
            out.push(LsCompletionItem {
                label: format!("{}.end", ev.name),
                kind: LsCompletionKind::Event,
                detail: Some(if zh { "事件结束时间" } else { "event end time" }.to_owned()),
                documentation: ev.description.clone(),
                insert_text: apply_name(&ev.name, in_string, ".end"),
                is_snippet: false,
                sort_priority: 1,
            });
        }
    }

    for ent in symbols.entities.values() {
        if !ent.has_base_time {
            continue;
        }
        out.push(LsCompletionItem {
            label: format!("{}.base_time", ent.name),
            kind: LsCompletionKind::Entity,
            detail: Some(if zh { "实体基准时间" } else { "entity base_time" }.to_owned()),
            documentation: ent.description.clone(),
            insert_text: apply_name(&ent.name, in_string, ".base_time"),
            is_snippet: false,
            sort_priority: 1,
        });
    }

    out
}

// ---- Hover ----

pub fn hover(src: &str, base: &Path, byte_offset: usize, lang: &str) -> Option<LsHover> {
    let pos = clamp_char_boundary(src, byte_offset);
    let idx = DocumentIndex::build(src);
    let zh = lang == "zh";

    // 1. 命中的符号（实体 / 事件 / 锚点 / 事件列表 / 关系组 / 可计算属性）
    if let Some(occ) = idx.occurrence_at(pos) {
        let compiled = check_document(
            src,
            base,
            &CheckOptions {
                run_scripts: true,
                lang: lang.to_owned(),
            },
        )
        .doc;
        let md = format_symbol_hover(occ.kind, &occ.name, &idx, compiled.as_ref(), zh);
        if !md.is_empty() {
            return Some(LsHover {
                contents: md,
                span: Some(occ.span),
                utf16: Some(span_to_utf16(src, occ.span)),
            });
        }
    }

    // 2. 命中内置属性键
    if let Some(attr) = idx.attr_key_at(pos)
        && let Some(spec) = find_field(attr.block, &attr.key)
    {
        let header = if spec.required {
            if zh {
                format!("`{}`（必填）", spec.name)
            } else {
                format!("`{}` (required)", spec.name)
            }
        } else {
            format!("`{}`", spec.name)
        };
        return Some(LsHover {
            contents: format!("{header}\n\n{}", spec.doc(lang)),
            span: Some(attr.span),
            utf16: Some(span_to_utf16(src, attr.span)),
        });
    }

    // 3. 命中顶层关键字
    let scanned = tokenize(src, pos);
    if let Some(tok) = scanned
        .tokens
        .iter()
        .find(|t| t.span.start <= pos && pos <= t.span.end)
        && let Some(doc) = keyword_doc(&tok.value, lang)
    {
        return Some(LsHover {
            contents: doc.to_owned(),
            span: Some(tok.span),
            utf16: Some(span_to_utf16(src, tok.span)),
        });
    }

    None
}

fn format_symbol_hover(
    kind: SymbolKind,
    name: &str,
    idx: &DocumentIndex,
    compiled: Option<&WeftDoc>,
    zh: bool,
) -> String {
    let decl = idx.declaration(kind, name);
    let mut lines = Vec::new();

    match kind {
        SymbolKind::Entity => {
            let label = if zh { "实体" } else { "Entity" };
            lines.push(format!("**{label}** `{name}`"));
            if let Some(doc) = compiled
                && let Some(ent) = doc.entity.get(name)
            {
                if let Some(desc) = &ent.description
                    && !desc.is_empty()
                {
                    lines.push(desc.clone());
                }
                if let Some(bt) = &ent.base_time_humanizer {
                    let k = if zh { "基准时间" } else { "Base time" };
                    lines.push(format!("- **{k}**: {bt}"));
                }
                if let Some(d) = decl
                    && !d.belong_to.is_empty()
                {
                    let k = if zh { "归属" } else { "Belongs to" };
                    lines.push(format!("- **{k}**: {}", d.belong_to.join(", ")));
                }
            } else if let Some(d) = decl {
                if let Some(desc) = &d.description {
                    lines.push(desc.clone());
                }
                if !d.belong_to.is_empty() {
                    let k = if zh { "归属" } else { "Belongs to" };
                    lines.push(format!("- **{k}**: {}", d.belong_to.join(", ")));
                }
            }
        }
        SymbolKind::Event => {
            let label = if zh { "事件" } else { "Event" };
            lines.push(format!("**{label}** `{name}`"));
            if let Some(doc) = compiled
                && let Some(ev) = doc.event.get(name)
            {
                if let Some(desc) = &ev.description
                    && !desc.is_empty()
                {
                    lines.push(desc.clone());
                }
                let time_label = if zh { "时间" } else { "Time" };
                match &ev.end_display {
                    Some(e) => {
                        lines.push(format!("- **{time_label}**: {} → {e}", ev.start_display))
                    }
                    None => lines.push(format!("- **{time_label}**: {}", ev.start_display)),
                }
                if !ev.entities.is_empty() {
                    let k = if zh { "参与实体" } else { "Entities" };
                    lines.push(format!("- **{k}**: {}", ev.entities.join(", ")));
                }
            } else if let Some(d) = decl {
                if let Some(desc) = &d.description {
                    lines.push(desc.clone());
                }
                if !d.entities.is_empty() {
                    let k = if zh { "参与实体" } else { "Entities" };
                    lines.push(format!("- **{k}**: {}", d.entities.join(", ")));
                }
            }
        }
        SymbolKind::Events => {
            let label = if zh { "事件列表" } else { "Events" };
            lines.push(format!("**{label}** `{name}`"));
            if let Some(doc) = compiled
                && let Some(list) = doc.events.get(name)
            {
                let k = if zh { "包含事件数" } else { "Events" };
                lines.push(format!("- **{k}**: {}", list.len()));
            }
        }
        SymbolKind::TimeAnchor => {
            let label = if zh { "时间锚点" } else { "Time anchor" };
            lines.push(format!("**{label}** `{name}`"));
        }
        SymbolKind::LinkGroup => {
            let label = if zh { "关系组" } else { "Link group" };
            lines.push(format!("**{label}** `{name}`"));
            if let Some(doc) = compiled
                && let Some(edges) = doc.entity_link.get(name)
            {
                let k = if zh { "关系边数" } else { "Edges" };
                lines.push(format!("- **{k}**: {}", edges.len()));
            }
        }
        SymbolKind::CalculableProp => {
            let label = if zh { "可计算属性" } else { "Calculable property" };
            lines.push(format!("**{label}** `{name}`"));
            if let Some(d) = decl
                && let Some(script) = &d.description
            {
                let k = if zh { "脚本" } else { "Script" };
                lines.push(format!("- **{k}**: `{script}`"));
            }
        }
    }

    lines.join("\n\n")
}

// ---- 跳转定义 / 查找引用 / 重命名 / 文档符号 / 格式化 ----

pub fn goto_definition(src: &str, byte_offset: usize) -> Option<LsLocation> {
    let pos = clamp_char_boundary(src, byte_offset);
    let idx = DocumentIndex::build(src);
    let occ = idx.occurrence_at(pos)?;
    let decl = idx.declaration(occ.kind, &occ.name)?;
    Some(LsLocation {
        span: decl.name_span,
        utf16: span_to_utf16(src, decl.name_span),
    })
}

pub fn references(src: &str, byte_offset: usize, include_declaration: bool) -> Vec<LsLocation> {
    let pos = clamp_char_boundary(src, byte_offset);
    let idx = DocumentIndex::build(src);
    let Some(occ) = idx.occurrence_at(pos) else {
        return Vec::new();
    };
    idx.occurrences_of(occ.kind, &occ.name)
        .into_iter()
        .filter(|o| include_declaration || !o.is_definition)
        .map(|o| LsLocation {
            span: o.span,
            utf16: span_to_utf16(src, o.span),
        })
        .collect()
}

pub fn prepare_rename(src: &str, byte_offset: usize) -> Option<LsPrepareRename> {
    let pos = clamp_char_boundary(src, byte_offset);
    let idx = DocumentIndex::build(src);
    let occ = idx.occurrence_at(pos)?;
    // 只有已在文档中声明的符号才允许重命名
    idx.declaration(occ.kind, &occ.name)?;
    Some(LsPrepareRename {
        span: occ.span,
        utf16: span_to_utf16(src, occ.span),
        placeholder: occ.name.clone(),
    })
}

pub fn rename(src: &str, byte_offset: usize, new_name: &str) -> Option<Vec<LsTextEdit>> {
    let trimmed = new_name.trim();
    if trimmed.is_empty() {
        return None;
    }
    let pos = clamp_char_boundary(src, byte_offset);
    let idx = DocumentIndex::build(src);
    let occ = idx.occurrence_at(pos)?;
    idx.declaration(occ.kind, &occ.name)?;

    let formatted = format_weft_name(trimmed);
    let mut edits: Vec<LsTextEdit> = idx
        .occurrences_of(occ.kind, &occ.name)
        .into_iter()
        .map(|o| LsTextEdit {
            span: o.span,
            utf16: span_to_utf16(src, o.span),
            new_text: formatted.clone(),
        })
        .collect();
    edits.sort_by_key(|e| e.span.start);
    edits.dedup_by_key(|e| (e.span.start, e.span.end));
    Some(edits)
}

pub fn document_symbols(src: &str) -> Vec<LsDocumentSymbol> {
    let idx = DocumentIndex::build(src);
    idx.top_decls
        .into_iter()
        .map(|entry| LsDocumentSymbol {
            kind: entry.kind.to_owned(),
            name: entry.name,
            detail: entry.detail,
            name_span: entry.name_span,
            full_span: entry.full_span,
            utf16_name: span_to_utf16(src, entry.name_span),
        })
        .collect()
}

pub fn format_document(src: &str, lang: &str) -> Result<String, Vec<LsDiagnostic>> {
    format_weft(src).map_err(|mut diags| {
        for d in &mut diags {
            d.localize(lang);
        }
        diags
            .iter()
            .map(|d| convert_diagnostic(src, d, lang))
            .collect()
    })
}
