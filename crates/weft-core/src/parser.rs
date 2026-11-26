//! `.weft` 领域语言词法分析器、AST、注释扫描器与支持错误恢复的 chumsky 语法解析器。
//!
//! 语法为类 HCL 的声明式块语言：
//! - 顶层声明：`story { ... }`、`time <name> = <expr>`、`entity <name> { ... }`、
//!   `link <group> { a -- "rel" -- b }`、`event <name> { ... }`、
//!   `events <name> = [...]`
//! - 标识符支持中文与中点（如 `哈利·波特`），含空格/特殊符号/数字开头时用双引号（如 `"9又3/4站台"`）。
//! - 表达式支持标量、列表、对象、成员访问（`事件.start`）与 `+` / `-` 时间偏移运算。

use std::fmt;

use chumsky::{input::ValueInput, prelude::*};

use crate::errors::{ByteSpan, Diagnostic, DocumentError, ValidationError};

pub type Span = SimpleSpan;
pub type Spanned<T> = (T, Span);

pub(crate) fn to_byte_span(span: Span) -> ByteSpan {
    ByteSpan::new(span.start, span.end)
}

// ---- AST ----

#[derive(Clone, Debug)]
pub struct WeftAst {
    pub items: Vec<Spanned<TopItem>>,
}

#[derive(Clone, Debug)]
pub enum TopItem {
    Story(Vec<Spanned<Attr>>),
    TimeAnchor {
        name: Spanned<String>,
        expr: Spanned<Expr>,
    },
    Entity {
        name: Spanned<String>,
        attrs: Vec<Spanned<Attr>>,
    },
    Link {
        group: Spanned<String>,
        edges: Vec<Spanned<LinkEdge>>,
    },
    Event {
        name: Spanned<String>,
        attrs: Vec<Spanned<Attr>>,
    },
    Events {
        name: Spanned<String>,
        events: Vec<Spanned<String>>,
    },
}

#[derive(Clone, Debug)]
pub struct Attr {
    pub key: Spanned<String>,
    pub value: Spanned<Expr>,
}

#[derive(Clone, Debug)]
pub struct LinkEdge {
    pub left: Spanned<String>,
    pub relation: Spanned<String>,
    pub right: Spanned<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
}

#[derive(Clone, Debug)]
pub enum Expr {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Ident(String),
    Member {
        base: Spanned<String>,
        field: Spanned<String>,
    },
    List(Vec<Spanned<Expr>>),
    Object(Vec<Spanned<Attr>>),
    Binary {
        lhs: Box<Spanned<Expr>>,
        op: BinOp,
        rhs: Box<Spanned<Expr>>,
    },
}

/// 源码中的注释记录（供格式化器保真回写）。
#[derive(Clone, Debug)]
pub struct Comment {
    pub span: ByteSpan,
    pub text: String,
    pub start_line: usize,
    pub end_line: usize,
    pub is_block: bool,
}

// ---- Token ----

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Ident(String),
    LinkDash,
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

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Null => write!(f, "null"),
            Token::Bool(b) => write!(f, "{b}"),
            Token::Int(n) => write!(f, "{n}"),
            Token::Float(n) => write!(f, "{n}"),
            Token::Str(s) => write!(f, "\"{s}\""),
            Token::Ident(s) => write!(f, "{s}"),
            Token::LinkDash => write!(f, "--"),
            Token::Plus => write!(f, "+"),
            Token::Minus => write!(f, "-"),
            Token::Eq => write!(f, "="),
            Token::Dot => write!(f, "."),
            Token::Comma => write!(f, ","),
            Token::Semi => write!(f, ";"),
            Token::LBrace => write!(f, "{{"),
            Token::RBrace => write!(f, "}}"),
            Token::LBracket => write!(f, "["),
            Token::RBracket => write!(f, "]"),
        }
    }
}

// ---- 标识符规则（词法与格式化共用） ----

pub fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

pub fn is_ident_continue(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '·' || c == '・'
}

pub fn is_valid_bare_name(s: &str) -> bool {
    if s.is_empty() || matches!(s, "true" | "false" | "null") {
        return false;
    }
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    is_ident_start(first) && chars.all(is_ident_continue)
}

// ---- 注释扫描器 ----

/// 扫描 `.weft` 源码中的全部注释（跳过字符串字面量内的 `#` / `//` / `/*`）。
pub fn extract_comments(src: &str) -> Vec<Comment> {
    let mut comments = Vec::new();
    let bytes = src.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut line = 1usize;

    while i < len {
        // 三引号多行字符串
        if bytes[i..].starts_with(b"\"\"\"") {
            i += 3;
            while i < len {
                if bytes[i] == b'\\' && i + 1 < len {
                    if bytes[i + 1] == b'\n' {
                        line += 1;
                    }
                    i += 2;
                } else if bytes[i..].starts_with(b"\"\"\"") {
                    i += 3;
                    break;
                } else {
                    if bytes[i] == b'\n' {
                        line += 1;
                    }
                    i += 1;
                }
            }
            continue;
        }

        // 单行双引号字符串
        if bytes[i] == b'"' {
            i += 1;
            while i < len {
                if bytes[i] == b'\\' && i + 1 < len {
                    i += 2;
                } else if bytes[i] == b'"' {
                    i += 1;
                    break;
                } else if bytes[i] == b'\n' {
                    line += 1;
                    i += 1;
                    break;
                } else {
                    i += 1;
                }
            }
            continue;
        }

        // 行注释 `# ...` 或 `// ...`
        if bytes[i] == b'#' || bytes[i..].starts_with(b"//") {
            let start = i;
            let start_line = line;
            while i < len && bytes[i] != b'\n' {
                i += 1;
            }
            let text = src[start..i].trim_end_matches('\r').trim_end().to_owned();
            comments.push(Comment {
                span: ByteSpan::new(start, i),
                text,
                start_line,
                end_line: start_line,
                is_block: false,
            });
            continue;
        }

        // 块注释 `/* ... */`
        if bytes[i..].starts_with(b"/*") {
            let start = i;
            let start_line = line;
            i += 2;
            while i < len && !bytes[i..].starts_with(b"*/") {
                if bytes[i] == b'\n' {
                    line += 1;
                }
                i += 1;
            }
            if i < len {
                i += 2;
            }
            comments.push(Comment {
                span: ByteSpan::new(start, i),
                text: src[start..i].to_owned(),
                start_line,
                end_line: line,
                is_block: true,
            });
            continue;
        }

        if bytes[i] == b'\n' {
            line += 1;
        }
        i += 1;
    }

    comments
}

// ---- 词法分析器 ----

fn dedent_multiline(raw: &str) -> String {
    let trimmed = raw
        .strip_prefix("\r\n")
        .or_else(|| raw.strip_prefix('\n'))
        .unwrap_or(raw);
    let mut lines: Vec<&str> = trimmed.lines().collect();
    if !trimmed.ends_with('\n')
        && lines
            .last()
            .is_some_and(|last| last.bytes().all(|b| b == b' ' || b == b'\t'))
    {
        lines.pop();
    }
    let min_indent = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.bytes().take_while(|&b| b == b' ' || b == b'\t').count())
        .min()
        .unwrap_or(0);
    lines
        .into_iter()
        .map(|line| {
            if line.trim().is_empty() {
                ""
            } else {
                &line[min_indent..]
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn lexer<'src>()
-> impl Parser<'src, &'src str, Vec<Spanned<Token>>, extra::Err<Rich<'src, char, Span>>> {
    let escape = just('\\').ignore_then(choice((
        just('\\').to('\\'),
        just('"').to('"'),
        just('n').to('\n'),
        just('r').to('\r'),
        just('t').to('\t'),
    )));

    // 三引号多行字符串 """..."""（必须先于普通双引号尝试）
    let multiline_str = just("\"\"\"")
        .ignore_then(
            escape
                .or(none_of('\\').and_is(just("\"\"\"").not()))
                .repeated()
                .collect::<String>(),
        )
        .then_ignore(just("\"\"\""))
        .map(|s| Token::Str(dedent_multiline(&s)))
        .labelled("multiline string");

    // 普通双引号字符串 "..."
    let single_str = just('"')
        .ignore_then(
            escape
                .or(none_of("\\\"\r\n"))
                .repeated()
                .collect::<String>(),
        )
        .then_ignore(just('"'))
        .map(Token::Str)
        .labelled("string");

    // 数字：整数或浮点（负号留给语法层的表达式解析，避免 `a - 1` 歧义）
    let number = text::int(10)
        .then(just('.').then(text::digits(10)).or_not())
        .to_slice()
        .try_map(|slice: &str, span| {
            if slice.contains('.') {
                slice
                    .parse::<f64>()
                    .map(Token::Float)
                    .map_err(|e| Rich::custom(span, format!("invalid float: {e}")))
            } else {
                slice
                    .parse::<i64>()
                    .map(Token::Int)
                    .map_err(|e| Rich::custom(span, format!("invalid integer: {e}")))
            }
        })
        .labelled("number");

    // 标识符（含中文与中点 `·`）与字面量关键字（true / false / null）
    let ident = any()
        .filter(|c: &char| is_ident_start(*c))
        .then(any().filter(|c: &char| is_ident_continue(*c)).repeated())
        .to_slice()
        .map(|s: &str| match s {
            "true" => Token::Bool(true),
            "false" => Token::Bool(false),
            "null" => Token::Null,
            _ => Token::Ident(s.to_owned()),
        })
        .labelled("identifier");

    let punct = choice((
        just("--").to(Token::LinkDash),
        just('+').to(Token::Plus),
        just('-').to(Token::Minus),
        just('=').to(Token::Eq),
        just('.').to(Token::Dot),
        just(',').to(Token::Comma),
        just(';').to(Token::Semi),
        just('{').to(Token::LBrace),
        just('}').to(Token::RBrace),
        just('[').to(Token::LBracket),
        just(']').to(Token::RBracket),
    ));

    let line_comment = just('#')
        .or(just("//").ignored().map(|()| '#'))
        .then(any().and_is(just('\n').not()).repeated())
        .ignored();

    let block_comment = just("/*")
        .then(any().and_is(just("*/").not()).repeated())
        .then(just("*/"))
        .ignored();

    let comment = line_comment.or(block_comment).padded();

    let token = choice((multiline_str, single_str, number, punct, ident))
        .map_with(|tok, e| Some((tok, e.span())))
        .recover_with(skip_then_retry_until(any().ignored(), end()));

    token
        .padded_by(comment.repeated())
        .padded()
        .repeated()
        .collect::<Vec<_>>()
        .map(|items| items.into_iter().flatten().collect())
        .then_ignore(comment.repeated().padded())
        .then_ignore(end())
}

// ---- 语法解析器（带块级与顶层错误恢复） ----

fn opt_sep<'tokens, I>()
-> impl Parser<'tokens, I, (), extra::Err<Rich<'tokens, Token, Span>>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = Span>,
{
    just(Token::Comma).or(just(Token::Semi)).or_not().ignored()
}

fn name_parser<'tokens, I>()
-> impl Parser<'tokens, I, Spanned<String>, extra::Err<Rich<'tokens, Token, Span>>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = Span>,
{
    select! {
        Token::Ident(s) => s,
        Token::Str(s) => s,
    }
    .map_with(|s, e| (s, e.span()))
    .labelled("name")
}

fn kw<'tokens, I>(
    word: &'static str,
) -> impl Parser<'tokens, I, (), extra::Err<Rich<'tokens, Token, Span>>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = Span>,
{
    select! {
        Token::Ident(s) if s == word => (),
    }
    .labelled(word)
}

fn top_kw<'tokens, I>()
-> impl Parser<'tokens, I, (), extra::Err<Rich<'tokens, Token, Span>>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = Span>,
{
    choice((
        kw("story"),
        kw("time"),
        kw("entity"),
        kw("link"),
        kw("event"),
        kw("events"),
    ))
}

fn attr_block_parser<'tokens, I, E>(
    val_expr: E,
) -> impl Parser<'tokens, I, Vec<Spanned<Attr>>, extra::Err<Rich<'tokens, Token, Span>>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = Span>,
    E: Parser<'tokens, I, Spanned<Expr>, extra::Err<Rich<'tokens, Token, Span>>> + Clone,
{
    let attr = name_parser()
        .then_ignore(just(Token::Eq))
        .then(val_expr)
        .map(|(key, value)| Attr { key, value })
        .map_with(|attr, e| Some((attr, e.span())))
        .labelled("attribute")
        .then_ignore(opt_sep())
        .recover_with(via_parser(
            any()
                .and_is(
                    just(Token::RBrace)
                        .ignored()
                        .or(name_parser().then(just(Token::Eq)).ignored())
                        .not(),
                )
                .repeated()
                .at_least(1)
                .to(None),
        ));

    attr.repeated()
        .collect::<Vec<_>>()
        .map(|items| items.into_iter().flatten().collect())
        .delimited_by(just(Token::LBrace), just(Token::RBrace))
        .recover_with(via_parser(nested_delimiters(
            Token::LBrace,
            Token::RBrace,
            [(Token::LBracket, Token::RBracket)],
            |_| Vec::new(),
        )))
}

fn parser<'tokens, I>() -> impl Parser<'tokens, I, WeftAst, extra::Err<Rich<'tokens, Token, Span>>>
where
    I: ValueInput<'tokens, Token = Token, Span = Span>,
{
    let ident = select! {
        Token::Ident(s) => s,
    }
    .labelled("identifier");

    let name = name_parser();

    let expr = recursive(|expr| {
        let object = attr_block_parser(expr.clone())
            .map(Expr::Object)
            .labelled("object");

        let list = expr
            .clone()
            .separated_by(just(Token::Comma))
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(just(Token::LBracket), just(Token::RBracket))
            .recover_with(via_parser(nested_delimiters(
                Token::LBracket,
                Token::RBracket,
                [(Token::LBrace, Token::RBrace)],
                |_| Vec::new(),
            )))
            .map(Expr::List)
            .labelled("list");

        let signed_num = just(Token::Minus)
            .or_not()
            .then(select! {
                Token::Int(n) => (Some(n), None),
                Token::Float(f) => (None, Some(f)),
            })
            .try_map(|(neg, (int, float)), span| match (neg, int, float) {
                (Some(_), Some(n), _) => n
                    .checked_neg()
                    .map(Expr::Int)
                    .ok_or_else(|| Rich::custom(span, "integer overflow")),
                (None, Some(n), _) => Ok(Expr::Int(n)),
                (Some(_), _, Some(f)) => Ok(Expr::Float(-f)),
                (None, _, Some(f)) => Ok(Expr::Float(f)),
                _ => unreachable!(),
            })
            .labelled("number");

        let literal = select! {
            Token::Null => Expr::Null,
            Token::Bool(b) => Expr::Bool(b),
        };

        // 标识符或字符串；后面若紧跟 `.field` 则构成成员访问（如 `海格午夜送信.start` 或 `"9又3/4站台".base_time`）。
        let named_or_member = select! {
            Token::Ident(s) => (s, true),
            Token::Str(s) => (s, false),
        }
        .map_with(|(s, is_ident), e| ((s, e.span()), is_ident))
        .then(
            just(Token::Dot)
                .ignore_then(ident.map_with(|f, e| (f, e.span())))
                .or_not(),
        )
        .map(|((base, is_ident), field)| match field {
            Some(field) => Expr::Member { base, field },
            None if is_ident => Expr::Ident(base.0),
            None => Expr::Str(base.0),
        });

        let atom = choice((literal, signed_num, list, object, named_or_member))
            .map_with(|node, e| (node, e.span()));

        // 左结合 `+` / `-` 运算（主要用于时间锚点偏移，如 `海格午夜送信.start - [0, 0, 7] + [0, 0, 0, 2]`）。
        let op = just(Token::Plus)
            .to(BinOp::Add)
            .or(just(Token::Minus).to(BinOp::Sub));

        atom.clone().foldl(
            op.then(atom).repeated(),
            |lhs: Spanned<Expr>, (op, rhs): (BinOp, Spanned<Expr>)| {
                let span = Span::from(lhs.1.start..rhs.1.end);
                (
                    Expr::Binary {
                        lhs: Box::new(lhs),
                        op,
                        rhs: Box::new(rhs),
                    },
                    span,
                )
            },
        )
    });

    let attr_block = attr_block_parser(expr.clone());

    let story_decl = kw("story")
        .ignore_then(attr_block.clone())
        .map(TopItem::Story)
        .labelled("story block");

    let time_decl = kw("time")
        .ignore_then(name.clone())
        .then_ignore(just(Token::Eq))
        .then(expr)
        .then_ignore(opt_sep())
        .map(|(name, expr)| TopItem::TimeAnchor { name, expr })
        .labelled("time anchor");

    let entity_decl = kw("entity")
        .ignore_then(name.clone())
        .then(attr_block.clone())
        .map(|(name, attrs)| TopItem::Entity { name, attrs })
        .labelled("entity block");

    let link_edge = name
        .clone()
        .then_ignore(just(Token::LinkDash))
        .then(name.clone())
        .then_ignore(just(Token::LinkDash))
        .then(name.clone())
        .map(|((left, relation), right)| LinkEdge {
            left,
            relation,
            right,
        })
        .map_with(|edge, e| Some((edge, e.span())))
        .labelled("link edge")
        .then_ignore(opt_sep());

    let link_decl = kw("link")
        .ignore_then(name.clone())
        .then(
            link_edge
                .repeated()
                .collect::<Vec<_>>()
                .map(|items| items.into_iter().flatten().collect())
                .delimited_by(just(Token::LBrace), just(Token::RBrace))
                .recover_with(via_parser(nested_delimiters(
                    Token::LBrace,
                    Token::RBrace,
                    [(Token::LBracket, Token::RBracket)],
                    |_| Vec::new(),
                ))),
        )
        .map(|(group, edges)| TopItem::Link { group, edges })
        .labelled("link block");

    let event_decl = kw("event")
        .ignore_then(name.clone())
        .then(attr_block)
        .map(|(name, attrs)| TopItem::Event { name, attrs })
        .labelled("event block");

    let name_list = name
        .clone()
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LBracket), just(Token::RBracket))
        .recover_with(via_parser(nested_delimiters(
            Token::LBracket,
            Token::RBracket,
            [(Token::LBrace, Token::RBrace)],
            |_| Vec::new(),
        )));

    let events_decl = kw("events")
        .ignore_then(name)
        .then_ignore(just(Token::Eq))
        .then(name_list)
        .then_ignore(opt_sep())
        .map(|(name, events)| TopItem::Events { name, events })
        .labelled("events declaration");

    let top_item = choice((
        story_decl,
        time_decl,
        entity_decl,
        link_decl,
        event_decl,
        events_decl,
    ))
    .map_with(|item, e| Some((item, e.span())))
    .recover_with(via_parser(
        any()
            .and_is(top_kw().not())
            .repeated()
            .at_least(1)
            .to(None),
    ));

    top_item
        .repeated()
        .collect::<Vec<_>>()
        .then_ignore(end())
        .map(|items| WeftAst {
            items: items.into_iter().flatten().collect(),
        })
}

// ---- 对外入口 ----

fn syntax_diagnostic(span: Span, details: impl Into<String>) -> Diagnostic {
    Diagnostic::from_validation(
        ValidationError::SyntaxError {
            details: details.into(),
        },
        Some(to_byte_span(span)),
    )
}

/// 容错解析 `.weft` 源码：尽可能恢复并产出部分 AST 与全部词法/语法诊断。
pub fn parse_weft_recover(src: &str) -> (Option<WeftAst>, Vec<Diagnostic>) {
    let mut diagnostics = Vec::new();
    let (tokens, lex_errs) = lexer().parse(src).into_output_errors();
    for err in lex_errs {
        diagnostics.push(syntax_diagnostic(*err.span(), err.to_string()));
    }
    let Some(tokens) = tokens else {
        return (None, diagnostics);
    };
    let eoi = Span::from(src.len()..src.len());
    let (ast, parse_errs) = parser()
        .parse(tokens.as_slice().map(eoi, |(tok, span)| (tok, span)))
        .into_output_errors();
    for err in parse_errs {
        diagnostics.push(syntax_diagnostic(*err.span(), err.to_string()));
    }
    if ast.is_none() && diagnostics.is_empty() {
        diagnostics.push(syntax_diagnostic(eoi, "unexpected end of input"));
    }
    (ast, diagnostics)
}

/// 严格解析 `.weft` 源码（供格式化器等需要零语法错误的场景使用）。
pub fn parse_weft(src: &str) -> Result<WeftAst, DocumentError> {
    let (ast, diagnostics) = parse_weft_recover(src);
    if let Some(first) = diagnostics.into_iter().next() {
        let span = first.span.unwrap_or_else(|| ByteSpan::new(src.len(), src.len()));
        let err = first
            .error
            .unwrap_or_else(|| ValidationError::SyntaxError {
                details: first.message,
            });
        return Err(DocumentError::validation_at(err, span.to_source_span(src)));
    }
    ast.ok_or_else(|| {
        DocumentError::validation_at(
            ValidationError::SyntaxError {
                details: "unexpected end of input".to_owned(),
            },
            ByteSpan::new(src.len(), src.len()).to_source_span(src),
        )
    })
}
