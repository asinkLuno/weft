//! `.weft` 领域语言词法分析器、AST 与 chumsky 语法解析器。
//!
//! 语法为类 HCL 的声明式块语言：
//! - 顶层声明：`story { ... }`、`time <name> = <expr>`、`entity <name> { ... }`、
//!   `link <group> { a -- "rel" -- b }`、`event <name> { ... }`、
//!   `narrative <name> = [...]`
//! - 标识符支持中文与中点（如 `哈利·波特`），含空格/特殊符号/数字开头时用双引号（如 `"9又3/4站台"`）。
//! - 表达式支持标量、列表、对象、成员访问（`事件.start`）与 `+` / `-` 时间偏移运算。

use std::fmt;

use chumsky::{input::ValueInput, prelude::*};

use crate::weft::errors::{DocumentError, SourceSpan, ValidationError};

pub type Span = SimpleSpan;
pub type Spanned<T> = (T, Span);

fn clamp_char_boundary(src: &str, offset: usize) -> usize {
    let mut safe = offset.min(src.len());
    while safe > 0 && !src.is_char_boundary(safe) {
        safe -= 1;
    }
    safe
}

/// 把 chumsky 字节区间转成 UTF-16 偏移（对齐 CodeMirror）与 1-based `(line, column)`。
pub(crate) fn source_span(src: &str, span: Span) -> SourceSpan {
    let start_byte = clamp_char_boundary(src, span.start);
    let end_byte = clamp_char_boundary(src, span.end.max(start_byte));
    let prefix = &src[..start_byte];
    let start = prefix.encode_utf16().count();
    let end = start + src[start_byte..end_byte].encode_utf16().count();
    let line = prefix.bytes().filter(|&b| b == b'\n').count() + 1;
    let column = match prefix.rfind('\n') {
        Some(pos) => prefix[pos + 1..].chars().count() + 1,
        None => prefix.chars().count() + 1,
    };
    SourceSpan {
        start,
        end,
        line,
        column,
    }
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
    Narrative {
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

// ---- 词法分析器 ----

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '·' || c == '・'
}

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

    let token = choice((multiline_str, single_str, number, punct, ident));

    token
        .map_with(|tok, e| (tok, e.span()))
        .padded_by(comment.repeated())
        .padded()
        .repeated()
        .collect()
        .then_ignore(comment.repeated().padded())
        .then_ignore(end())
}

// ---- 语法解析器 ----

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

fn attr_block_parser<'tokens, I, E>(
    val_expr: E,
) -> impl Parser<'tokens, I, Vec<Spanned<Attr>>, extra::Err<Rich<'tokens, Token, Span>>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = Span>,
    E: Parser<'tokens, I, Spanned<Expr>, extra::Err<Rich<'tokens, Token, Span>>> + Clone,
{
    name_parser()
        .then_ignore(just(Token::Eq))
        .then(val_expr)
        .map(|(key, value)| Attr { key, value })
        .map_with(|attr, e| (attr, e.span()))
        .labelled("attribute")
        .then_ignore(opt_sep())
        .repeated()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LBrace), just(Token::RBrace))
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

    let kw = |word: &'static str| {
        select! {
            Token::Ident(s) if s == word => (),
        }
        .labelled(word)
    };

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
        .map_with(|edge, e| (edge, e.span()))
        .labelled("link edge");

    let link_decl = kw("link")
        .ignore_then(name.clone())
        .then(
            link_edge
                .then_ignore(opt_sep())
                .repeated()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LBrace), just(Token::RBrace)),
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
        .delimited_by(just(Token::LBracket), just(Token::RBracket));

    let narrative_decl = kw("narrative")
        .ignore_then(name)
        .then_ignore(just(Token::Eq))
        .then(name_list)
        .then_ignore(opt_sep())
        .map(|(name, events)| TopItem::Narrative { name, events })
        .labelled("narrative declaration");

    let top_item = choice((
        story_decl,
        time_decl,
        entity_decl,
        link_decl,
        event_decl,
        narrative_decl,
    ))
    .map_with(|item, e| (item, e.span()));

    top_item
        .repeated()
        .collect::<Vec<_>>()
        .then_ignore(end())
        .map(|items| WeftAst { items })
}

// ---- 对外入口 ----

fn syntax_error(src: &str, span: Span, details: impl Into<String>) -> DocumentError {
    DocumentError::validation_at(
        ValidationError::SyntaxError {
            details: details.into(),
        },
        source_span(src, span),
    )
}

/// 完整解析 `.weft` 源码为 AST；遇到词法或语法错误时返回带源码位置的 `DocumentError`。
pub(crate) fn parse_weft(src: &str) -> Result<WeftAst, DocumentError> {
    let (tokens, lex_errs) = lexer().parse(src).into_output_errors();
    if let Some(first) = lex_errs.first() {
        return Err(syntax_error(src, *first.span(), first.to_string()));
    }
    let tokens = tokens.unwrap_or_default();
    let eoi = Span::from(src.len()..src.len());
    let (ast, parse_errs) = parser()
        .parse(tokens.as_slice().map(eoi, |(tok, span)| (tok, span)))
        .into_output_errors();
    if let Some(first) = parse_errs.first() {
        return Err(syntax_error(src, *first.span(), first.to_string()));
    }
    ast.ok_or_else(|| syntax_error(src, eoi, "unexpected end of input"))
}
