//! `.weft` 源码格式化器：保留全部注释、对齐属性等号、智能单行/多行列表，且严格幂等。

use unicode_width::UnicodeWidthStr;

use crate::errors::Diagnostic;
use crate::parser::{
    Attr, BinOp, Comment, Expr, LinkEdge, LinkEdgeKind, LinkEndpoint, Span, Spanned, TopItem,
    WeftAst, extract_comments, is_valid_bare_name, parse_weft_recover,
};

const INDENT_UNIT: &str = "  ";
const MAX_LINE_WIDTH: usize = 88;
const MAX_INLINE_NARRATIVE_ITEMS: usize = 4;

/// 格式化 `.weft` 源码。若存在词法或语法错误，拒绝格式化并返回全部语法诊断。
pub fn format_weft(src: &str) -> Result<String, Vec<Diagnostic>> {
    let (ast, diagnostics) = parse_weft_recover(src);
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let Some(ast) = ast else {
        return Ok(String::new());
    };
    let comments = extract_comments(src);
    let mut formatter = Formatter::new(src, comments);
    formatter.format_ast(&ast);
    Ok(formatter.finish())
}

struct Formatter<'a> {
    src: &'a str,
    line_starts: Vec<usize>,
    comments: Vec<Comment>,
    comment_idx: usize,
    out: String,
}

impl<'a> Formatter<'a> {
    fn new(src: &'a str, comments: Vec<Comment>) -> Self {
        let mut line_starts = vec![0];
        for (i, b) in src.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }
        Self {
            src,
            line_starts,
            comments,
            comment_idx: 0,
            out: String::with_capacity(src.len()),
        }
    }

    fn finish(mut self) -> String {
        while self.out.ends_with('\n') {
            self.out.pop();
        }
        if !self.out.is_empty() {
            self.out.push('\n');
        }
        self.out
    }

    fn line_of(&self, byte_offset: usize) -> usize {
        let clamped = byte_offset.min(self.src.len());
        match self.line_starts.binary_search(&clamped) {
            Ok(idx) => idx + 1,
            Err(idx) => idx,
        }
    }

    fn has_comments_in(&self, start: usize, end: usize) -> bool {
        self.comments[self.comment_idx..]
            .iter()
            .take_while(|c| c.span.start < end)
            .any(|c| c.span.start >= start && c.span.end <= end)
    }

    fn push_indent(&mut self, level: usize) {
        for _ in 0..level {
            self.out.push_str(INDENT_UNIT);
        }
    }

    fn current_line_width(&self) -> usize {
        let last_line = match self.out.rfind('\n') {
            Some(pos) => &self.out[pos + 1..],
            None => &self.out,
        };
        UnicodeWidthStr::width(last_line)
    }

    /// 输出 `before_byte` 之前的全部未消费注释，保留组间最多一个空行。
    /// 返回是否输出了注释以及最后一条注释的结束行号。
    fn emit_leading_comments(
        &mut self,
        before_byte: usize,
        indent: usize,
        ensure_blank_before_first: bool,
        prev_end_line: Option<usize>,
    ) -> Option<usize> {
        let mut last_comment_end: Option<usize> = None;
        while let Some(c) = self.comments.get(self.comment_idx) {
            if c.span.end > before_byte {
                break;
            }
            let comment_start_line = c.start_line;
            let comment_end_line = c.end_line;
            let text = c.text.clone();
            let is_block = c.is_block;
            self.comment_idx += 1;

            if let Some(prev) = last_comment_end {
                if comment_start_line > prev + 1 {
                    self.out.push('\n');
                }
            } else if !self.out.is_empty() {
                let had_blank = prev_end_line.is_some_and(|p| comment_start_line > p + 1);
                if ensure_blank_before_first || had_blank {
                    self.out.push('\n');
                }
            }

            if is_block && text.contains('\n') {
                for line in text.lines() {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        self.push_indent(indent);
                        self.out.push_str(trimmed);
                    }
                    self.out.push('\n');
                }
            } else {
                self.push_indent(indent);
                self.out.push_str(&text);
                self.out.push('\n');
            }
            last_comment_end = Some(comment_end_line);
        }
        last_comment_end
    }

    /// 若紧随当前节点同一行末尾有行内注释，输出 ` # ...` 并消费该注释。
    fn emit_trailing_comment(&mut self, after_byte: usize, node_end_line: usize) {
        if let Some(c) = self.comments.get(self.comment_idx) {
            if c.span.start >= after_byte && c.start_line == node_end_line && !c.text.contains('\n')
            {
                let text = c.text.clone();
                self.comment_idx += 1;
                self.out.push(' ');
                self.out.push_str(&text);
            }
        }
    }

    fn format_ast(&mut self, ast: &WeftAst) {
        let mut prev_end_line: Option<usize> = None;

        for (item, span) in &ast.items {
            let item_start_line = self.line_of(span.start);
            let last_comment_line = self.emit_leading_comments(
                span.start,
                0,
                prev_end_line.is_some(),
                prev_end_line,
            );

            if let Some(comment_end) = last_comment_line {
                if item_start_line > comment_end + 1 {
                    self.out.push('\n');
                }
            } else if prev_end_line.is_some() {
                self.out.push('\n');
            }

            self.format_top_item(item, *span);
            let end_line = self.line_of(span.end.saturating_sub(1));
            self.emit_trailing_comment(span.end, end_line);
            self.out.push('\n');
            prev_end_line = Some(end_line);
        }

        // 文件末尾剩余注释
        self.emit_leading_comments(self.src.len(), 0, prev_end_line.is_some(), prev_end_line);
    }

    fn format_top_item(&mut self, item: &TopItem, span: Span) {
        match item {
            TopItem::Story(attrs) => {
                self.out.push_str("story ");
                self.format_attr_block(attrs, span, 0);
            }
            TopItem::TimeAnchor { name, expr } => {
                self.out.push_str("time ");
                self.out.push_str(&format_name(&name.0));
                self.out.push_str(" = ");
                self.format_expr(expr, 0);
            }
            TopItem::Entity { name, attrs } => {
                self.out.push_str("entity ");
                self.out.push_str(&format_name(&name.0));
                self.out.push(' ');
                self.format_attr_block(attrs, span, 0);
            }
            TopItem::Link { name, attrs, edges } => {
                self.out.push_str("link ");
                self.out.push_str(&format_name(&name.0));
                self.out.push(' ');
                self.format_link_block(attrs, edges, span, 0);
            }
            TopItem::Event { name, attrs } => {
                self.out.push_str("event ");
                self.out.push_str(&format_name(&name.0));
                self.out.push(' ');
                self.format_attr_block(attrs, span, 0);
            }
            TopItem::Events { name, events } => {
                self.out.push_str("events ");
                self.out.push_str(&format_name(&name.0));
                self.out.push_str(" = ");
                self.format_name_list(events, span, 0);
            }
        }
    }

    fn format_attr_block(&mut self, attrs: &[Spanned<Attr>], block_span: Span, indent: usize) {
        let inner_indent = indent + 1;
        let has_inner_comments = self.has_comments_in(block_span.start, block_span.end);
        if attrs.is_empty() && !has_inner_comments {
            self.out.push_str("{}");
            return;
        }

        self.out.push_str("{\n");

        // 将连续属性按空行或独立注释分成若干等号对齐组
        let groups = partition_attr_groups(attrs, |start, end| {
            self.has_comments_in(start, end)
        }, |offset| self.line_of(offset));

        let mut prev_end_line: Option<usize> = None;

        for group in groups {
            let align_width = group
                .iter()
                .map(|(attr, _)| UnicodeWidthStr::width(format_name(&attr.key.0).as_str()))
                .max()
                .unwrap_or(0);

            for (attr, attr_span) in group {
                let start_line = self.line_of(attr_span.start);
                let last_comment = self.emit_leading_comments(
                    attr_span.start,
                    inner_indent,
                    false,
                    prev_end_line,
                );
                if let Some(comment_end) = last_comment {
                    if start_line > comment_end + 1 {
                        self.out.push('\n');
                    }
                } else if let Some(prev) = prev_end_line {
                    if start_line > prev + 1 {
                        self.out.push('\n');
                    }
                }

                self.push_indent(inner_indent);
                let key = format_name(&attr.key.0);
                let key_width = UnicodeWidthStr::width(key.as_str());
                self.out.push_str(&key);
                for _ in key_width..align_width {
                    self.out.push(' ');
                }
                self.out.push_str(" = ");
                self.format_expr(&attr.value, inner_indent);

                let end_line = self.line_of(attr_span.end.saturating_sub(1));
                self.emit_trailing_comment(attr_span.end, end_line);
                self.out.push('\n');
                prev_end_line = Some(end_line);
            }
        }

        // 块尾注释（最后一个属性之后、`}` 之前）
        self.emit_leading_comments(block_span.end, inner_indent, false, prev_end_line);
        self.push_indent(indent);
        self.out.push('}');
    }

    fn format_link_block(
        &mut self,
        attrs: &[Spanned<Attr>],
        edges: &[Spanned<LinkEdge>],
        block_span: Span,
        indent: usize,
    ) {
        let inner_indent = indent + 1;
        let has_inner_comments = self.has_comments_in(block_span.start, block_span.end);
        if attrs.is_empty() && edges.is_empty() && !has_inner_comments {
            self.out.push_str("{}");
            return;
        }

        self.out.push_str("{\n");
        let mut prev_end_line: Option<usize> = None;

        // 1. 块级图代数属性（与普通属性块保持一致的等号对齐）
        if !attrs.is_empty() {
            let groups = partition_attr_groups(
                attrs,
                |start, end| self.has_comments_in(start, end),
                |offset| self.line_of(offset),
            );
            for group in groups {
                let align_width = group
                    .iter()
                    .map(|(attr, _)| UnicodeWidthStr::width(format_name(&attr.key.0).as_str()))
                    .max()
                    .unwrap_or(0);

                for (attr, attr_span) in group {
                    let start_line = self.line_of(attr_span.start);
                    let last_comment = self.emit_leading_comments(
                        attr_span.start,
                        inner_indent,
                        false,
                        prev_end_line,
                    );
                    if let Some(comment_end) = last_comment {
                        if start_line > comment_end + 1 {
                            self.out.push('\n');
                        }
                    } else if let Some(prev) = prev_end_line {
                        if start_line > prev + 1 {
                            self.out.push('\n');
                        }
                    }

                    self.push_indent(inner_indent);
                    let key = format_name(&attr.key.0);
                    let key_width = UnicodeWidthStr::width(key.as_str());
                    self.out.push_str(&key);
                    for _ in key_width..align_width {
                        self.out.push(' ');
                    }
                    self.out.push_str(" = ");
                    self.format_expr(&attr.value, inner_indent);

                    let end_line = self.line_of(attr_span.end.saturating_sub(1));
                    self.emit_trailing_comment(attr_span.end, end_line);
                    self.out.push('\n');
                    prev_end_line = Some(end_line);
                }
            }
        }

        // 2. 边与团语句（属性与边之间默认留一个空行）
        let mut first_edge = true;
        for (edge, edge_span) in edges {
            let start_line = self.line_of(edge_span.start);
            let need_blank_after_attrs = first_edge && !attrs.is_empty();
            let last_comment = self.emit_leading_comments(
                edge_span.start,
                inner_indent,
                need_blank_after_attrs,
                prev_end_line,
            );
            if let Some(comment_end) = last_comment {
                if start_line > comment_end + 1 {
                    self.out.push('\n');
                }
            } else if need_blank_after_attrs {
                self.out.push('\n');
            } else if let Some(prev) = prev_end_line {
                if start_line > prev + 1 {
                    self.out.push('\n');
                }
            }
            first_edge = false;

            self.push_indent(inner_indent);
            match &edge.kind {
                LinkEdgeKind::Directed { left, right } => {
                    self.format_link_endpoint(left, inner_indent);
                    match &edge.label {
                        Some((label, _)) => {
                            self.out.push_str(" -- ");
                            self.out.push_str(&format_quoted_string(label));
                            self.out.push_str(" -> ");
                        }
                        None => self.out.push_str(" -> "),
                    }
                    self.format_link_endpoint(right, inner_indent);
                }
                LinkEdgeKind::Symmetric { left, right } => {
                    self.format_link_endpoint(left, inner_indent);
                    match &edge.label {
                        Some((label, _)) => {
                            self.out.push_str(" <- ");
                            self.out.push_str(&format_quoted_string(label));
                            self.out.push_str(" -> ");
                        }
                        None => self.out.push_str(" <-> "),
                    }
                    self.format_link_endpoint(right, inner_indent);
                }
                LinkEdgeKind::Clique { members, list_span } => {
                    match &edge.label {
                        Some((label, _)) => {
                            self.out.push_str("<- ");
                            self.out.push_str(&format_quoted_string(label));
                            self.out.push_str(" -> ");
                        }
                        None => self.out.push_str("<-> "),
                    }
                    self.format_name_list(members, *list_span, inner_indent);
                }
            }

            if let Some((edge_attrs, attr_span)) = &edge.attrs {
                self.out.push(' ');
                self.format_attr_block(edge_attrs, *attr_span, inner_indent);
            }

            let end_line = self.line_of(edge_span.end.saturating_sub(1));
            self.emit_trailing_comment(edge_span.end, end_line);
            self.out.push('\n');
            prev_end_line = Some(end_line);
        }

        self.emit_leading_comments(block_span.end, inner_indent, false, prev_end_line);
        self.push_indent(indent);
        self.out.push('}');
    }

    fn format_link_endpoint(&mut self, endpoint: &LinkEndpoint, indent: usize) {
        match endpoint {
            LinkEndpoint::Single((name, _)) => self.out.push_str(&format_name(name)),
            LinkEndpoint::List(names, list_span) => {
                self.format_name_list(names, *list_span, indent);
            }
        }
    }

    fn format_name_list(&mut self, items: &[Spanned<String>], list_span: Span, indent: usize) {
        let has_inner_comments = self.has_comments_in(list_span.start, list_span.end);
        if items.is_empty() && !has_inner_comments {
            self.out.push_str("[]");
            return;
        }

        let formatted: Vec<String> = items.iter().map(|(s, _)| format_name(s)).collect();
        let inline = format!("[{}]", formatted.join(", "));
        let inline_fits = !has_inner_comments
            && items.len() <= MAX_INLINE_NARRATIVE_ITEMS
            && self.current_line_width() + UnicodeWidthStr::width(inline.as_str())
                <= MAX_LINE_WIDTH;

        if inline_fits {
            self.out.push_str(&inline);
            return;
        }

        let inner_indent = indent + 1;
        self.out.push_str("[\n");
        let mut prev_end_line: Option<usize> = None;

        for ((_, item_span), text) in items.iter().zip(formatted.iter()) {
            let start_line = self.line_of(item_span.start);
            let last_comment =
                self.emit_leading_comments(item_span.start, inner_indent, false, prev_end_line);
            if let Some(comment_end) = last_comment {
                if start_line > comment_end + 1 {
                    self.out.push('\n');
                }
            } else if let Some(prev) = prev_end_line {
                if start_line > prev + 1 {
                    self.out.push('\n');
                }
            }

            self.push_indent(inner_indent);
            self.out.push_str(text);
            self.out.push(',');

            let end_line = self.line_of(item_span.end.saturating_sub(1));
            self.emit_trailing_comment(item_span.end, end_line);
            self.out.push('\n');
            prev_end_line = Some(end_line);
        }

        self.emit_leading_comments(list_span.end, inner_indent, false, prev_end_line);
        self.push_indent(indent);
        self.out.push(']');
    }

    fn format_expr(&mut self, expr: &Spanned<Expr>, indent: usize) {
        let (node, span) = expr;
        match node {
            Expr::Null => self.out.push_str("null"),
            Expr::Bool(b) => self.out.push_str(if *b { "true" } else { "false" }),
            Expr::Int(n) => self.out.push_str(&n.to_string()),
            Expr::Float(f) => self.out.push_str(&format_float(*f)),
            Expr::Str(s) => self.format_string(s, indent),
            Expr::Ident(s) => self.out.push_str(s),
            Expr::Member { base, field } => {
                self.out.push_str(&format_name(&base.0));
                self.out.push('.');
                self.out.push_str(&field.0);
            }
            Expr::Binary { lhs, op, rhs } => {
                self.format_expr(lhs, indent);
                match op {
                    BinOp::Add => self.out.push_str(" + "),
                    BinOp::Sub => self.out.push_str(" - "),
                }
                self.format_expr(rhs, indent);
            }
            Expr::Object(attrs) => self.format_attr_block(attrs, *span, indent),
            Expr::List(items) => self.format_expr_list(items, *span, indent),
        }
    }

    fn format_expr_list(&mut self, items: &[Spanned<Expr>], list_span: Span, indent: usize) {
        let has_inner_comments = self.has_comments_in(list_span.start, list_span.end);
        if items.is_empty() && !has_inner_comments {
            self.out.push_str("[]");
            return;
        }

        if !has_inner_comments
            && let Some(inline) = try_inline_expr_list(items)
            && self.current_line_width() + UnicodeWidthStr::width(inline.as_str())
                <= MAX_LINE_WIDTH
        {
            self.out.push_str(&inline);
            return;
        }

        let inner_indent = indent + 1;
        self.out.push_str("[\n");
        let mut prev_end_line: Option<usize> = None;

        for item in items {
            let item_span = item.1;
            let start_line = self.line_of(item_span.start);
            let last_comment =
                self.emit_leading_comments(item_span.start, inner_indent, false, prev_end_line);
            if let Some(comment_end) = last_comment {
                if start_line > comment_end + 1 {
                    self.out.push('\n');
                }
            } else if let Some(prev) = prev_end_line {
                if start_line > prev + 1 {
                    self.out.push('\n');
                }
            }

            self.push_indent(inner_indent);
            self.format_expr(item, inner_indent);
            self.out.push(',');

            let end_line = self.line_of(item_span.end.saturating_sub(1));
            self.emit_trailing_comment(item_span.end, end_line);
            self.out.push('\n');
            prev_end_line = Some(end_line);
        }

        self.emit_leading_comments(list_span.end, inner_indent, false, prev_end_line);
        self.push_indent(indent);
        self.out.push(']');
    }

    fn format_string(&mut self, s: &str, indent: usize) {
        if can_use_multiline_literal(s) {
            self.out.push_str("\"\"\"\n");
            for line in s.split('\n') {
                if !line.is_empty() {
                    self.push_indent(indent + 1);
                    self.out.push_str(&escape_multiline_line(line));
                }
                self.out.push('\n');
            }
            self.push_indent(indent);
            self.out.push_str("\"\"\"");
        } else {
            self.out.push_str(&format_quoted_string(s));
        }
    }
}

/// 将同一块内的属性按空行或夹杂注释切分成独立的 `=` 对齐组。
fn partition_attr_groups<F, L>(
    attrs: &[Spanned<Attr>],
    has_comments_between: F,
    line_of: L,
) -> Vec<&[Spanned<Attr>]>
where
    F: Fn(usize, usize) -> bool,
    L: Fn(usize) -> usize,
{
    if attrs.is_empty() {
        return Vec::new();
    }
    let mut groups = Vec::new();
    let mut start_idx = 0;
    for i in 1..attrs.len() {
        let prev_span = attrs[i - 1].1;
        let curr_span = attrs[i].1;
        let prev_end_line = line_of(prev_span.end.saturating_sub(1));
        let curr_start_line = line_of(curr_span.start);
        let has_blank = curr_start_line > prev_end_line + 1;
        let has_comment_line = has_comments_between(prev_span.end, curr_span.start)
            && line_of(curr_span.start) > prev_end_line;
        // 仅行内注释（与前一条属性同行）不打断对齐组；独立注释行或空行才打断。
        let broken_by_standalone_comment = has_comment_line
            && (curr_start_line > prev_end_line + 1
                || has_standalone_comment_gap(prev_end_line, curr_start_line));
        if has_blank || broken_by_standalone_comment {
            groups.push(&attrs[start_idx..i]);
            start_idx = i;
        }
    }
    groups.push(&attrs[start_idx..]);
    groups
}

fn has_standalone_comment_gap(prev_end_line: usize, curr_start_line: usize) -> bool {
    curr_start_line > prev_end_line + 1
}

fn try_inline_expr_list(items: &[Spanned<Expr>]) -> Option<String> {
    let mut parts = Vec::with_capacity(items.len());
    for (expr, _) in items {
        parts.push(try_inline_expr(expr)?);
    }
    Some(format!("[{}]", parts.join(", ")))
}

fn try_inline_expr(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Null => Some("null".to_owned()),
        Expr::Bool(b) => Some(if *b { "true" } else { "false" }.to_owned()),
        Expr::Int(n) => Some(n.to_string()),
        Expr::Float(f) => Some(format_float(*f)),
        Expr::Str(s) => {
            if s.contains('\n') {
                None
            } else {
                Some(format_quoted_string(s))
            }
        }
        Expr::Ident(s) => Some(s.clone()),
        Expr::Member { base, field } => Some(format!("{}.{}", format_name(&base.0), field.0)),
        Expr::Binary { lhs, op, rhs } => {
            let l = try_inline_expr(&lhs.0)?;
            let r = try_inline_expr(&rhs.0)?;
            let sym = match op {
                BinOp::Add => "+",
                BinOp::Sub => "-",
            };
            Some(format!("{l} {sym} {r}"))
        }
        Expr::List(items) => try_inline_expr_list(items),
        Expr::Object(attrs) if attrs.is_empty() => Some("{}".to_owned()),
        Expr::Object(_) => None,
    }
}

fn format_name(name: &str) -> String {
    if is_valid_bare_name(name) {
        name.to_owned()
    } else {
        format_quoted_string(name)
    }
}

fn format_quoted_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn format_float(f: f64) -> String {
    let s = f.to_string();
    if f.is_finite() && !s.contains('.') && !s.contains('e') && !s.contains('E') {
        format!("{s}.0")
    } else {
        s
    }
}

/// 只有包含换行、且经 `dedent_multiline` 回读后与原文完全一致的字符串才输出 `"""..."""`。
fn can_use_multiline_literal(s: &str) -> bool {
    if !s.contains('\n') || s.contains('\r') || s.starts_with('\n') || s.ends_with('\n') {
        return false;
    }
    let has_non_empty = s.lines().any(|l| !l.trim().is_empty());
    if !has_non_empty {
        return false;
    }
    // 若所有非空行都带共同前导空白，dedent_multiline 会削掉它，此时回退普通引号以保语义无损。
    let all_indented = s
        .lines()
        .filter(|l| !l.trim().is_empty())
        .all(|l| l.starts_with(' ') || l.starts_with('\t'));
    !all_indented
}

fn escape_multiline_line(line: &str) -> String {
    let escaped_backslashes = line.replace('\\', "\\\\").replace('\t', "\\t");
    escaped_backslashes.replace("\"\"\"", "\\\"\\\"\\\"")
}
