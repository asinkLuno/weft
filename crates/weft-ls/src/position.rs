//! UTF-8 字节偏移、LSP `(line, utf16_col)` 与 CodeMirror 平铺 UTF-16 偏移互转。

use lsp_types::{Position, Range};
use serde::{Deserialize, Serialize};
use weft_core::errors::ByteSpan;
use weft_core::parser::is_valid_bare_name;

/// 平铺 UTF-16 字符区间（供 CodeMirror 6 直接使用）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Utf16Range {
    pub from: usize,
    pub to: usize,
}

/// 源码行首字节索引，支持在 UTF-8 字节、LSP 行列与 UTF-16 平铺偏移间安全换算。
#[derive(Clone, Debug)]
pub struct LineIndex {
    line_starts: Vec<usize>,
}

impl LineIndex {
    pub fn new(src: &str) -> Self {
        let mut line_starts = vec![0];
        for (idx, b) in src.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(idx + 1);
            }
        }
        Self { line_starts }
    }

    pub fn byte_to_lsp(&self, src: &str, byte_offset: usize) -> Position {
        let offset = clamp_char_boundary(src, byte_offset);
        let line = match self.line_starts.binary_search(&offset) {
            Ok(idx) => idx,
            Err(idx) => idx.saturating_sub(1),
        };
        let line_start = self.line_starts[line];
        let col_utf16 = src[line_start..offset].encode_utf16().count();
        Position::new(line as u32, col_utf16 as u32)
    }

    pub fn lsp_to_byte(&self, src: &str, pos: Position) -> usize {
        let line = pos.line as usize;
        if line >= self.line_starts.len() {
            return src.len();
        }
        let line_start = self.line_starts[line];
        let line_end = self
            .line_starts
            .get(line + 1)
            .copied()
            .unwrap_or(src.len());
        let line_str = &src[line_start..line_end];
        let mut utf16_seen = 0u32;
        for (rel_byte, ch) in line_str.char_indices() {
            if ch == '\n' || ch == '\r' {
                return line_start + rel_byte;
            }
            if utf16_seen >= pos.character {
                return line_start + rel_byte;
            }
            utf16_seen += ch.len_utf16() as u32;
        }
        line_end
    }

    pub fn span_to_lsp_range(&self, src: &str, span: ByteSpan) -> Range {
        Range::new(
            self.byte_to_lsp(src, span.start),
            self.byte_to_lsp(src, span.end),
        )
    }

    pub fn lsp_range_to_span(&self, src: &str, range: Range) -> ByteSpan {
        ByteSpan::new(
            self.lsp_to_byte(src, range.start),
            self.lsp_to_byte(src, range.end),
        )
    }

    pub fn full_range(&self, src: &str) -> Range {
        Range::new(Position::new(0, 0), self.byte_to_lsp(src, src.len()))
    }
}

pub fn clamp_char_boundary(src: &str, offset: usize) -> usize {
    let mut safe = offset.min(src.len());
    while safe > 0 && !src.is_char_boundary(safe) {
        safe -= 1;
    }
    safe
}

pub fn byte_to_utf16(src: &str, byte_offset: usize) -> usize {
    let safe = clamp_char_boundary(src, byte_offset);
    src[..safe].encode_utf16().count()
}

pub fn utf16_to_byte(src: &str, utf16_offset: usize) -> usize {
    let mut seen = 0usize;
    for (byte_idx, ch) in src.char_indices() {
        if seen >= utf16_offset {
            return byte_idx;
        }
        seen += ch.len_utf16();
    }
    src.len()
}

pub fn span_to_utf16(src: &str, span: ByteSpan) -> Utf16Range {
    let start = clamp_char_boundary(src, span.start);
    let end = clamp_char_boundary(src, span.end.max(start));
    let from = src[..start].encode_utf16().count();
    let to = from + src[start..end].encode_utf16().count();
    Utf16Range { from, to }
}

/// 扩展给定字节区间以包含整行（含行尾换行），用于删除整行 Quick Fix。
pub fn expand_to_full_line(src: &str, span: ByteSpan) -> ByteSpan {
    let bytes = src.as_bytes();
    let mut start = clamp_char_boundary(src, span.start);
    while start > 0 && bytes[start - 1] != b'\n' {
        start -= 1;
    }
    let mut end = clamp_char_boundary(src, span.end.max(start));
    while end < bytes.len() && bytes[end] != b'\n' {
        end += 1;
    }
    if end < bytes.len() && bytes[end] == b'\n' {
        end += 1;
    }
    ByteSpan::new(start, end)
}

/// 转义双引号字符串内容。
pub fn escape_string_inner(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out
}

/// 按 Weft 标识符规则格式化名字：合法裸标识符不加引号，否则加双引号并转义。
pub fn format_weft_name(name: &str) -> String {
    if is_valid_bare_name(name) {
        name.to_owned()
    } else {
        format!("\"{}\"", escape_string_inner(name))
    }
}
