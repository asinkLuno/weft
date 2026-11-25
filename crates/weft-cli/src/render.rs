//! `ty` 风格诊断渲染器：支持 `full`（ariadne 彩色源码标注）、`concise`、`github` 与 `json`。

use std::io::{self, IsTerminal};

use ariadne::{Color, Config, IndexType, Label, Report, ReportKind, Source};
use clap::ValueEnum;
use serde::Serialize;
use weft_core::errors::{ByteSpan, Diagnostic, Severity};

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// Rich source snippets with colored underlines and secondary labels (like `ty`)
    Full,
    /// Single-line `file:line:col: severity[rule] message`
    Concise,
    /// GitHub Actions workflow annotations
    Github,
    /// Machine-readable JSON array
    Json,
}

pub fn should_use_color(no_color_flag: bool) -> bool {
    if no_color_flag || std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    io::stderr().is_terminal()
}

pub fn render_file_diagnostics(
    file: &str,
    src: &str,
    diagnostics: &[Diagnostic],
    format: OutputFormat,
    color: bool,
) {
    match format {
        OutputFormat::Full => render_full(file, src, diagnostics, color),
        OutputFormat::Concise => render_concise(file, src, diagnostics),
        OutputFormat::Github => render_github(file, src, diagnostics),
        OutputFormat::Json => {}
    }
}

fn render_full(file: &str, src: &str, diagnostics: &[Diagnostic], color: bool) {
    let config = Config::new()
        .with_index_type(IndexType::Byte)
        .with_color(color);

    for diag in diagnostics {
        let (kind, primary_color) = match diag.severity {
            Severity::Error => (ReportKind::Error, Color::Red),
            Severity::Warning => (ReportKind::Warning, Color::Yellow),
        };
        let primary_span = clamp_span(src, diag.span.unwrap_or_else(|| ByteSpan::new(0, 0)));

        let mut builder = Report::build(kind, (file, primary_span.range()))
            .with_config(config)
            .with_code(diag.rule)
            .with_message(&diag.message);

        if diag.span.is_some() {
            builder = builder.with_label(
                Label::new((file, primary_span.range()))
                    .with_message(&diag.message)
                    .with_color(primary_color)
                    .with_order(1)
                    .with_priority(1),
            );
        }

        for rel in &diag.related {
            let rel_span = clamp_span(src, rel.span);
            builder = builder.with_label(
                Label::new((file, rel_span.range()))
                    .with_message(&rel.message)
                    .with_color(Color::Cyan)
                    .with_order(0),
            );
        }

        if let Some(help) = &diag.help {
            builder = builder.with_help(help);
        }

        let _ = builder.finish().eprint((file, Source::from(src)));
    }
}

fn render_concise(file: &str, src: &str, diagnostics: &[Diagnostic]) {
    for diag in diagnostics {
        let level = match diag.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        match diag.span {
            Some(span) => {
                let (line, col) = span.line_col(src);
                eprintln!("{file}:{line}:{col}: {level}[{}] {}", diag.rule, diag.message);
            }
            None => {
                eprintln!("{file}: {level}[{}] {}", diag.rule, diag.message);
            }
        }
    }
}

fn render_github(file: &str, src: &str, diagnostics: &[Diagnostic]) {
    for diag in diagnostics {
        let level = match diag.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        let msg = diag.message.replace('\n', "%0A");
        match diag.span {
            Some(span) => {
                let (line, col) = span.line_col(src);
                println!("::{level} file={file},line={line},col={col},title={}::{msg}", diag.rule);
            }
            None => {
                println!("::{level} file={file},title={}::{msg}", diag.rule);
            }
        }
    }
}

#[derive(Serialize)]
pub struct JsonDiagnostic {
    pub file: String,
    pub severity: Severity,
    pub rule: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<ByteSpan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
}

impl JsonDiagnostic {
    pub fn from_diagnostic(file: &str, src: &str, diag: &Diagnostic) -> Self {
        let (line, column) = match diag.span {
            Some(span) => {
                let (l, c) = span.line_col(src);
                (Some(l), Some(c))
            }
            None => (None, None),
        };
        Self {
            file: file.to_owned(),
            severity: diag.severity,
            rule: diag.rule,
            message: diag.message.clone(),
            line,
            column,
            span: diag.span,
            help: diag.help.clone(),
        }
    }
}

fn clamp_span(src: &str, span: ByteSpan) -> ByteSpan {
    let mut start = span.start.min(src.len());
    while start > 0 && !src.is_char_boundary(start) {
        start -= 1;
    }
    let mut end = span.end.max(start).min(src.len());
    while end < src.len() && !src.is_char_boundary(end) {
        end += 1;
    }
    ByteSpan::new(start, end)
}
