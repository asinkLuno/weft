//! 容错词法扫描与光标上下文推断：即使文档处于未写完的语法残缺状态也能稳定提供补全与符号兜底。

use std::collections::{BTreeMap, BTreeSet};

use weft_core::errors::ByteSpan;
use weft_core::parser::{is_ident_continue, is_ident_start};

use crate::schema::{AttrBlockKind, BlockKind, FieldValueKind, find_field};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokKind {
    Ident,
    Str,
    Num,
    Dash2,
    ArrowBoth,
    ArrowRight,
    ArrowLeft,
    Plus,
    Minus,
    Eq,
    Dot,
    Comma,
    Semi,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
}

#[derive(Clone, Debug)]
pub struct Tok {
    pub kind: TokKind,
    pub value: String,
    pub span: ByteSpan,
    pub line: usize,
}

impl Tok {
    pub fn is_name(&self) -> bool {
        matches!(self.kind, TokKind::Ident | TokKind::Str)
    }

    pub fn is_link_op(&self) -> bool {
        matches!(
            self.kind,
            TokKind::Dash2 | TokKind::ArrowBoth | TokKind::ArrowRight | TokKind::ArrowLeft
        )
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CursorString {
    pub span: ByteSpan,
    pub multiline: bool,
    pub closed: bool,
}

pub struct TokenizeResult {
    pub tokens: Vec<Tok>,
    pub cursor_line: usize,
    pub cursor_in_comment: bool,
    pub cursor_in_string: Option<CursorString>,
}

pub fn tokenize(src: &str, cursor_byte: usize) -> TokenizeResult {
    let mut tokens = Vec::new();
    let bytes = src.as_bytes();
    let n = bytes.len();
    let mut i = 0usize;
    let mut line = 0usize;
    let mut cursor_line = 0usize;
    let mut cursor_in_comment = false;
    let mut cursor_in_string = None;

    let advance_line = |pos: usize, line: &mut usize, cursor_line: &mut usize| {
        if pos < cursor_byte {
            *cursor_line += 1;
        }
        *line += 1;
    };

    while i < n {
        let b = bytes[i];
        if b == b'\n' {
            advance_line(i, &mut line, &mut cursor_line);
            i += 1;
            continue;
        }
        if b == b' ' || b == b'\t' || b == b'\r' {
            i += 1;
            continue;
        }

        // 行注释 `#` 或 `//`
        if b == b'#' || (b == b'/' && bytes.get(i + 1) == Some(&b'/')) {
            let start = i;
            while i < n && bytes[i] != b'\n' {
                i += 1;
            }
            if cursor_byte > start && cursor_byte <= i {
                cursor_in_comment = true;
            }
            continue;
        }

        // 块注释 `/* ... */`
        if b == b'/' && bytes.get(i + 1) == Some(&b'*') {
            let start = i;
            i += 2;
            let mut closed = false;
            while i < n {
                if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    i += 2;
                    closed = true;
                    break;
                }
                if bytes[i] == b'\n' {
                    advance_line(i, &mut line, &mut cursor_line);
                }
                i += 1;
            }
            if cursor_byte > start && (if closed { cursor_byte < i } else { cursor_byte <= i }) {
                cursor_in_comment = true;
            }
            continue;
        }

        // 三引号多行字符串
        if bytes[i..].starts_with(b"\"\"\"") {
            let start = i;
            let start_line = line;
            i += 3;
            let mut val = String::new();
            let mut closed = false;
            while i < n {
                if bytes[i..].starts_with(b"\"\"\"") {
                    i += 3;
                    closed = true;
                    break;
                }
                if bytes[i] == b'\\' && i + 1 < n {
                    let next = src[i + 1..].chars().next().unwrap();
                    if next == '\n' {
                        advance_line(i + 1, &mut line, &mut cursor_line);
                    }
                    val.push(unescape_char(next));
                    i += 1 + next.len_utf8();
                } else {
                    let ch = src[i..].chars().next().unwrap();
                    if ch == '\n' {
                        advance_line(i, &mut line, &mut cursor_line);
                    }
                    val.push(ch);
                    i += ch.len_utf8();
                }
            }
            if cursor_byte >= start + 3
                && (if closed {
                    cursor_byte <= i - 3
                } else {
                    cursor_byte <= i
                })
            {
                cursor_in_string = Some(CursorString {
                    span: ByteSpan::new(start, i),
                    multiline: true,
                    closed,
                });
            }
            tokens.push(Tok {
                kind: TokKind::Str,
                value: val.trim().to_owned(),
                span: ByteSpan::new(start, i),
                line: start_line,
            });
            continue;
        }

        // 单行字符串 `"..."`
        if b == b'"' {
            let start = i;
            let start_line = line;
            i += 1;
            let mut val = String::new();
            let mut closed = false;
            while i < n && bytes[i] != b'\n' && bytes[i] != b'\r' {
                if bytes[i] == b'\\' && i + 1 < n {
                    let next = src[i + 1..].chars().next().unwrap();
                    val.push(unescape_char(next));
                    i += 1 + next.len_utf8();
                } else if bytes[i] == b'"' {
                    i += 1;
                    closed = true;
                    break;
                } else {
                    let ch = src[i..].chars().next().unwrap();
                    val.push(ch);
                    i += ch.len_utf8();
                }
            }
            if cursor_byte > start && (if closed { cursor_byte < i } else { cursor_byte <= i }) {
                cursor_in_string = Some(CursorString {
                    span: ByteSpan::new(start, i),
                    multiline: false,
                    closed,
                });
            }
            tokens.push(Tok {
                kind: TokKind::Str,
                value: val,
                span: ByteSpan::new(start, i),
                line: start_line,
            });
            continue;
        }

        // 关系连线符 `<->` / `--` / `->` / `<-`
        if b == b'<' && bytes.get(i + 1) == Some(&b'-') && bytes.get(i + 2) == Some(&b'>') {
            tokens.push(Tok {
                kind: TokKind::ArrowBoth,
                value: "<->".to_owned(),
                span: ByteSpan::new(i, i + 3),
                line,
            });
            i += 3;
            continue;
        }
        if b == b'-' && bytes.get(i + 1) == Some(&b'-') {
            tokens.push(Tok {
                kind: TokKind::Dash2,
                value: "--".to_owned(),
                span: ByteSpan::new(i, i + 2),
                line,
            });
            i += 2;
            continue;
        }
        if b == b'-' && bytes.get(i + 1) == Some(&b'>') {
            tokens.push(Tok {
                kind: TokKind::ArrowRight,
                value: "->".to_owned(),
                span: ByteSpan::new(i, i + 2),
                line,
            });
            i += 2;
            continue;
        }
        if b == b'<' && bytes.get(i + 1) == Some(&b'-') {
            tokens.push(Tok {
                kind: TokKind::ArrowLeft,
                value: "<-".to_owned(),
                span: ByteSpan::new(i, i + 2),
                line,
            });
            i += 2;
            continue;
        }

        // 单字符符号
        if let Some(kind) = single_char_tok(b) {
            tokens.push(Tok {
                kind,
                value: src[i..i + 1].to_owned(),
                span: ByteSpan::new(i, i + 1),
                line,
            });
            i += 1;
            continue;
        }

        // 数字
        if b.is_ascii_digit() {
            let start = i;
            while i < n && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if bytes.get(i) == Some(&b'.') && bytes.get(i + 1).is_some_and(u8::is_ascii_digit) {
                i += 1;
                while i < n && bytes[i].is_ascii_digit() {
                    i += 1;
                }
            }
            tokens.push(Tok {
                kind: TokKind::Num,
                value: src[start..i].to_owned(),
                span: ByteSpan::new(start, i),
                line,
            });
            continue;
        }

        // Unicode 标识符
        let ch = src[i..].chars().next().unwrap();
        if is_ident_start(ch) {
            let start = i;
            i += ch.len_utf8();
            while i < n {
                let next = src[i..].chars().next().unwrap();
                if is_ident_continue(next) {
                    i += next.len_utf8();
                } else {
                    break;
                }
            }
            tokens.push(Tok {
                kind: TokKind::Ident,
                value: src[start..i].to_owned(),
                span: ByteSpan::new(start, i),
                line,
            });
            continue;
        }

        i += ch.len_utf8();
    }

    TokenizeResult {
        tokens,
        cursor_line,
        cursor_in_comment,
        cursor_in_string,
    }
}

fn unescape_char(ch: char) -> char {
    match ch {
        'n' => '\n',
        'r' => '\r',
        't' => '\t',
        other => other,
    }
}

fn single_char_tok(b: u8) -> Option<TokKind> {
    match b {
        b'+' => Some(TokKind::Plus),
        b'-' => Some(TokKind::Minus),
        b'=' => Some(TokKind::Eq),
        b'.' => Some(TokKind::Dot),
        b',' => Some(TokKind::Comma),
        b';' => Some(TokKind::Semi),
        b'{' => Some(TokKind::LBrace),
        b'}' => Some(TokKind::RBrace),
        b'[' => Some(TokKind::LBracket),
        b']' => Some(TokKind::RBracket),
        _ => None,
    }
}

// ---- 补全符号快照与光标上下文 ----

#[derive(Clone, Debug)]
pub struct ScanEntity {
    pub name: String,
    pub has_base_time: bool,
    pub description: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ScanEvent {
    pub name: String,
    pub has_end: bool,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct ScanSymbols {
    pub has_story: bool,
    pub entities: BTreeMap<String, ScanEntity>,
    pub events: BTreeMap<String, ScanEvent>,
    pub time_anchors: BTreeSet<String>,
    pub calculable_props: BTreeSet<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeFieldKind {
    BaseTime,
    Start,
    End,
    Anchor,
}

#[derive(Clone, Debug)]
pub enum CursorContext {
    None,
    Top,
    Member {
        base: String,
    },
    AttrKey {
        block: AttrBlockKind,
        existing_keys: BTreeSet<String>,
    },
    /// `link` 块行首：既可写图代数属性，也可写边端点节点。
    LinkLineStart {
        existing_keys: BTreeSet<String>,
    },
    BoolValue,
    EntityRef {
        exclude: BTreeSet<String>,
    },
    NodeRef {
        exclude: BTreeSet<String>,
    },
    EventRef {
        exclude: BTreeSet<String>,
    },
    CalcPropRef {
        exclude: BTreeSet<String>,
    },
    TimeExpr {
        self_event: Option<String>,
        allow_self_start_end: bool,
        field: TimeFieldKind,
    },
}

#[derive(Clone)]
struct CursorSnapshot {
    brace_depth: usize,
    bracket_depth: usize,
    block_kind: Option<BlockKind>,
    block_name: Option<String>,
    active_attr: Option<String>,
    active_attr_line: Option<usize>,
    top_time_assign_line: Option<usize>,
    in_events_list: bool,
    existing_keys: BTreeSet<String>,
    edge_existing_keys: BTreeSet<String>,
    list_items: BTreeSet<String>,
    last: Option<Tok>,
    prev: Option<Tok>,
    has_link_op_on_cursor_line: bool,
    last_link_op_on_cursor_line: Option<TokKind>,
}

fn is_before_cursor(tok: &Tok, pos: usize) -> bool {
    if tok.span.start >= pos {
        return false;
    }
    if tok.span.end >= pos && tok.is_name() {
        return false;
    }
    true
}

fn is_named_decl_keyword(word: &str) -> bool {
    matches!(word, "time" | "entity" | "link" | "event" | "events")
}

pub fn scan_document(
    tokens: &[Tok],
    cursor_byte: usize,
    cursor_line: usize,
    cursor_in_string: Option<CursorString>,
) -> (ScanSymbols, CursorContext) {
    let mut symbols = ScanSymbols::default();

    let mut brace_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut block_kind: Option<BlockKind> = None;
    let mut block_name: Option<String> = None;
    let mut in_story_calc_props = false;
    let mut in_link_edge_block = false;
    let mut active_attr: Option<String> = None;
    let mut active_attr_line: Option<usize> = None;
    let mut top_time_assign_line: Option<usize> = None;
    let mut in_events_list = false;
    let mut existing_keys: BTreeSet<String> = BTreeSet::new();
    let mut edge_existing_keys: BTreeSet<String> = BTreeSet::new();
    let mut list_items: BTreeSet<String> = BTreeSet::new();

    let mut last: Option<Tok> = None;
    let mut prev: Option<Tok> = None;
    let mut has_link_op_on_cursor_line = false;
    let mut last_link_op_on_cursor_line: Option<TokKind> = None;
    let mut snapshot: Option<CursorSnapshot> = None;

    macro_rules! capture_snapshot {
        () => {
            if snapshot.is_none() {
                snapshot = Some(CursorSnapshot {
                    brace_depth,
                    bracket_depth,
                    block_kind: if in_link_edge_block {
                        Some(BlockKind::LinkEdge)
                    } else {
                        block_kind
                    },
                    block_name: block_name.clone(),
                    active_attr: active_attr.clone(),
                    active_attr_line,
                    top_time_assign_line,
                    in_events_list,
                    existing_keys: existing_keys.clone(),
                    edge_existing_keys: edge_existing_keys.clone(),
                    list_items: list_items.clone(),
                    last: last.clone(),
                    prev: prev.clone(),
                    has_link_op_on_cursor_line,
                    last_link_op_on_cursor_line,
                });
            }
        };
    }

    macro_rules! consume {
        ($tok:expr) => {
            if snapshot.is_none() {
                let t: &Tok = $tok;
                if !is_before_cursor(t, cursor_byte) {
                    capture_snapshot!();
                } else {
                    prev = last.take();
                    if t.line == cursor_line && t.is_link_op() {
                        has_link_op_on_cursor_line = true;
                        last_link_op_on_cursor_line = Some(t.kind);
                    }
                    last = Some(t.clone());
                }
            }
        };
    }

    let mut i = 0usize;
    while i < tokens.len() {
        let tok = &tokens[i];
        consume!(tok);

        if brace_depth == 0 && bracket_depth == 0 && tok.kind == TokKind::Ident {
            if tok.value == "story"
                && tokens.get(i + 1).is_some_and(|t| t.kind == TokKind::LBrace)
            {
                consume!(&tokens[i + 1]);
                symbols.has_story = true;
                block_kind = Some(BlockKind::Story);
                block_name = None;
                existing_keys.clear();
                active_attr = None;
                active_attr_line = None;
                brace_depth = 1;
                i += 2;
                continue;
            }
            if tok.value == "time"
                && tokens.get(i + 1).is_some_and(Tok::is_name)
                && tokens.get(i + 2).is_some_and(|t| t.kind == TokKind::Eq)
            {
                symbols.time_anchors.insert(tokens[i + 1].value.clone());
                consume!(&tokens[i + 1]);
                consume!(&tokens[i + 2]);
                top_time_assign_line = Some(tokens[i + 2].line);
                i += 3;
                continue;
            }
            if tok.value == "events"
                && tokens.get(i + 1).is_some_and(Tok::is_name)
                && tokens.get(i + 2).is_some_and(|t| t.kind == TokKind::Eq)
                && tokens
                    .get(i + 3)
                    .is_some_and(|t| t.kind == TokKind::LBracket)
            {
                consume!(&tokens[i + 1]);
                consume!(&tokens[i + 2]);
                consume!(&tokens[i + 3]);
                in_events_list = true;
                list_items.clear();
                bracket_depth = 1;
                i += 4;
                continue;
            }
            if matches!(tok.value.as_str(), "entity" | "event" | "link")
                && tokens.get(i + 1).is_some_and(Tok::is_name)
            {
                let name = tokens[i + 1].value.clone();
                match tok.value.as_str() {
                    "entity" => {
                        symbols.entities.entry(name.clone()).or_insert(ScanEntity {
                            name: name.clone(),
                            has_base_time: false,
                            description: None,
                        });
                    }
                    "event" => {
                        symbols.events.entry(name.clone()).or_insert(ScanEvent {
                            name: name.clone(),
                            has_end: false,
                            description: None,
                        });
                    }
                    _ => {}
                }
                if tokens.get(i + 2).is_some_and(|t| t.kind == TokKind::LBrace) {
                    consume!(&tokens[i + 1]);
                    consume!(&tokens[i + 2]);
                    block_kind = match tok.value.as_str() {
                        "entity" => Some(BlockKind::Entity),
                        "event" => Some(BlockKind::Event),
                        _ => Some(BlockKind::Link),
                    };
                    block_name = Some(name);
                    existing_keys.clear();
                    active_attr = None;
                    active_attr_line = None;
                    brace_depth = 1;
                    i += 3;
                    continue;
                }
            }
        }

        match tok.kind {
            TokKind::LBrace => {
                brace_depth += 1;
                if block_kind == Some(BlockKind::Link) && brace_depth == 2 {
                    in_link_edge_block = true;
                    edge_existing_keys.clear();
                    active_attr = None;
                    active_attr_line = None;
                }
                i += 1;
                continue;
            }
            TokKind::RBrace => {
                if in_story_calc_props && brace_depth == 2 {
                    in_story_calc_props = false;
                }
                if in_link_edge_block && brace_depth == 2 {
                    in_link_edge_block = false;
                    edge_existing_keys.clear();
                    active_attr = None;
                    active_attr_line = None;
                }
                brace_depth = brace_depth.saturating_sub(1);
                if brace_depth == 0 {
                    block_kind = None;
                    block_name = None;
                    active_attr = None;
                    active_attr_line = None;
                    existing_keys.clear();
                }
                i += 1;
                continue;
            }
            TokKind::LBracket => {
                bracket_depth += 1;
                if bracket_depth == 1 {
                    list_items.clear();
                }
                i += 1;
                continue;
            }
            TokKind::RBracket => {
                bracket_depth = bracket_depth.saturating_sub(1);
                if bracket_depth == 0 {
                    in_events_list = false;
                    list_items.clear();
                }
                i += 1;
                continue;
            }
            _ => {}
        }

        if in_link_edge_block && brace_depth == 2 && bracket_depth == 0 {
            if tok.is_name() && tokens.get(i + 1).is_some_and(|t| t.kind == TokKind::Eq) {
                consume!(&tokens[i + 1]);
                edge_existing_keys.insert(tok.value.clone());
                active_attr = Some(tok.value.clone());
                active_attr_line = Some(tokens[i + 1].line);
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }

        if brace_depth == 1 && bracket_depth == 0 {
            if tok.is_name() && tokens.get(i + 1).is_some_and(|t| t.kind == TokKind::Eq) {
                consume!(&tokens[i + 1]);
                existing_keys.insert(tok.value.clone());
                active_attr = Some(tok.value.clone());
                active_attr_line = Some(tokens[i + 1].line);

                if block_kind == Some(BlockKind::Story)
                    && tok.value == "calculable_props"
                    && tokens.get(i + 2).is_some_and(|t| t.kind == TokKind::LBrace)
                {
                    consume!(&tokens[i + 2]);
                    in_story_calc_props = true;
                    brace_depth = 2;
                    i += 3;
                    continue;
                }

                if block_kind == Some(BlockKind::Entity)
                    && let Some(ref name) = block_name
                    && let Some(ent) = symbols.entities.get_mut(name)
                {
                    if tok.value == "base_time" {
                        ent.has_base_time = true;
                    } else if tok.value == "description"
                        && let Some(desc_tok) = tokens.get(i + 2)
                        && desc_tok.kind == TokKind::Str
                    {
                        ent.description = Some(desc_tok.value.clone());
                    }
                } else if block_kind == Some(BlockKind::Event)
                    && let Some(ref name) = block_name
                    && let Some(ev) = symbols.events.get_mut(name)
                {
                    if tok.value == "end" {
                        ev.has_end = true;
                    } else if tok.value == "description"
                        && let Some(desc_tok) = tokens.get(i + 2)
                        && desc_tok.kind == TokKind::Str
                    {
                        ev.description = Some(desc_tok.value.clone());
                    }
                }

                i += 2;
                continue;
            }
        }

        if in_story_calc_props && brace_depth == 2 {
            if tok.is_name() && tokens.get(i + 1).is_some_and(|t| t.kind == TokKind::Eq) {
                symbols.calculable_props.insert(tok.value.clone());
            }
            i += 1;
            continue;
        }

        if bracket_depth == 1 && tok.is_name() && snapshot.is_none() {
            list_items.insert(tok.value.clone());
        }

        i += 1;
    }

    capture_snapshot!();
    let ctx = resolve_cursor_context(snapshot.unwrap(), cursor_line, cursor_in_string);
    (symbols, ctx)
}

fn resolve_cursor_context(
    s: CursorSnapshot,
    cursor_line: usize,
    cursor_in_string: Option<CursorString>,
) -> CursorContext {
    if cursor_in_string.is_some_and(|cs| cs.multiline) {
        return CursorContext::None;
    }

    let member_ctx = || -> Option<CursorContext> {
        if s.last.as_ref().is_some_and(|t| t.kind == TokKind::Dot)
            && let Some(prev) = s.prev.as_ref()
            && prev.is_name()
        {
            return Some(CursorContext::Member {
                base: prev.value.clone(),
            });
        }
        None
    };

    if s.in_events_list && s.bracket_depth == 1 {
        return CursorContext::EventRef {
            exclude: s.list_items,
        };
    }

    if s.brace_depth == 0 && s.bracket_depth == 0 && s.top_time_assign_line == Some(cursor_line) {
        return member_ctx().unwrap_or(CursorContext::TimeExpr {
            self_event: None,
            allow_self_start_end: false,
            field: TimeFieldKind::Anchor,
        });
    }

    if s.brace_depth == 0 {
        if cursor_in_string.is_some() {
            return CursorContext::None;
        }
        if s.last
            .as_ref()
            .is_some_and(|t| t.kind == TokKind::Ident && is_named_decl_keyword(&t.value))
        {
            return CursorContext::None;
        }
        return CursorContext::Top;
    }

    let Some(block_kind) = s.block_kind else {
        return CursorContext::None;
    };

    if block_kind == BlockKind::LinkEdge && s.brace_depth == 2 {
        let field_spec = s
            .active_attr
            .as_deref()
            .and_then(|attr| find_field(BlockKind::LinkEdge, attr));
        let on_attr_value = s.active_attr.is_some()
            && (s.active_attr_line == Some(cursor_line)
                || s.last
                    .as_ref()
                    .is_some_and(|t| matches!(t.kind, TokKind::Eq | TokKind::Plus | TokKind::Minus)));
        if on_attr_value && s.bracket_depth == 0 {
            if let Some(spec) = field_spec
                && spec.value_kind == FieldValueKind::Time
            {
                let field = match spec.name {
                    "start" => TimeFieldKind::Start,
                    _ => TimeFieldKind::End,
                };
                return member_ctx().unwrap_or(CursorContext::TimeExpr {
                    self_event: None,
                    allow_self_start_end: true,
                    field,
                });
            }
            return CursorContext::None;
        }
        if cursor_in_string.is_some() {
            return CursorContext::None;
        }
        return CursorContext::AttrKey {
            block: AttrBlockKind::LinkEdge,
            existing_keys: s.edge_existing_keys,
        };
    }

    if s.brace_depth > 1 {
        return CursorContext::None;
    }

    if block_kind == BlockKind::Link {
        if s.bracket_depth == 1 {
            return CursorContext::NodeRef {
                exclude: s.list_items,
            };
        }
        let on_attr_value = s.active_attr.is_some()
            && (s.active_attr_line == Some(cursor_line)
                || s.last.as_ref().is_some_and(|t| t.kind == TokKind::Eq));
        if on_attr_value {
            if let Some(spec) = s
                .active_attr
                .as_deref()
                .and_then(|attr| find_field(BlockKind::Link, attr))
                && spec.value_kind == FieldValueKind::Bool
            {
                return CursorContext::BoolValue;
            }
            return CursorContext::None;
        }
        if s.has_link_op_on_cursor_line {
            if matches!(
                s.last_link_op_on_cursor_line,
                Some(TokKind::Dash2 | TokKind::ArrowLeft)
            ) {
                return CursorContext::None;
            }
            return CursorContext::NodeRef {
                exclude: BTreeSet::new(),
            };
        }
        if cursor_in_string.is_some() {
            return CursorContext::NodeRef {
                exclude: BTreeSet::new(),
            };
        }
        return CursorContext::LinkLineStart {
            existing_keys: s.existing_keys,
        };
    }

    let field_spec = s
        .active_attr
        .as_deref()
        .and_then(|attr| find_field(block_kind, attr));

    if s.bracket_depth == 1 {
        if let Some(spec) = field_spec {
            match spec.value_kind {
                FieldValueKind::EntityList => {
                    return CursorContext::EntityRef {
                        exclude: s.list_items,
                    };
                }
                FieldValueKind::CalcPropList => {
                    return CursorContext::CalcPropRef {
                        exclude: s.list_items,
                    };
                }
                _ => return CursorContext::None,
            }
        }
        return CursorContext::None;
    }

    let on_attr_value = s.active_attr.is_some()
        && (s.active_attr_line == Some(cursor_line)
            || s.last
                .as_ref()
                .is_some_and(|t| matches!(t.kind, TokKind::Eq | TokKind::Plus | TokKind::Minus)));

    if on_attr_value && s.bracket_depth == 0 {
        if let Some(spec) = field_spec {
            match spec.value_kind {
                FieldValueKind::Time => {
                    let field = match spec.name {
                        "base_time" => TimeFieldKind::BaseTime,
                        "start" => TimeFieldKind::Start,
                        _ => TimeFieldKind::End,
                    };
                    return member_ctx().unwrap_or(CursorContext::TimeExpr {
                        self_event: if block_kind == BlockKind::Event {
                            s.block_name
                        } else {
                            None
                        },
                        allow_self_start_end: block_kind == BlockKind::Event,
                        field,
                    });
                }
                FieldValueKind::Bool => {
                    return CursorContext::BoolValue;
                }
                _ => {}
            }
        }
        return CursorContext::None;
    }

    if cursor_in_string.is_some() {
        return CursorContext::None;
    }

    match block_kind.as_attr_block() {
        Some(block) => CursorContext::AttrKey {
            block,
            existing_keys: s.existing_keys,
        },
        None => CursorContext::None,
    }
}
