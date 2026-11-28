//! 传输无关的 LSP 3.17 JSON-RPC 协议处理器：
//! 同时驱动 `weft lsp`（stdio）与 Tauri 桌面端（进程内 `@codemirror/lsp-client`）。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use lsp_server::{ErrorCode, Message, Notification, Request, Response};
use lsp_types::notification::{
    DidChangeConfiguration, DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument,
    DidSaveTextDocument, Exit, Initialized, Notification as _, PublishDiagnostics,
};
use lsp_types::request::{
    CodeActionRequest, Completion, DocumentSymbolRequest, Formatting, GotoDefinition, HoverRequest,
    Initialize, PrepareRenameRequest, References, Rename, Request as _, Shutdown,
};
use lsp_types::{
    CodeAction, CodeActionKind, CodeActionOptions, CodeActionOrCommand, CodeActionProviderCapability,
    CompletionItem, CompletionItemKind, CompletionOptions, CompletionResponse,
    Diagnostic as LspDiagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, DocumentSymbol,
    DocumentSymbolResponse, GotoDefinitionResponse, Hover, HoverContents, HoverProviderCapability,
    InitializeResult, InsertTextFormat, Location, MarkupContent, MarkupKind, NumberOrString, OneOf,
    PrepareRenameResponse, PublishDiagnosticsParams, Range, RenameOptions, ServerCapabilities,
    ServerInfo, SymbolKind as LspSymbolKind, TextDocumentSyncCapability, TextDocumentSyncKind,
    TextEdit, Uri, WorkspaceEdit,
};

use crate::position::LineIndex;
use crate::service::{self, LsCodeAction, LsCompletionKind, LsDiagnostic, LsSeverity};

struct OpenDoc {
    text: String,
    version: Option<i32>,
}

/// 传输无关的 WEFT LSP 状态机。
pub struct LspServer {
    lang: String,
    documents: HashMap<Uri, OpenDoc>,
    shutdown_requested: bool,
}

impl LspServer {
    pub fn new(lang: String) -> Self {
        Self {
            lang,
            documents: HashMap::new(),
            shutdown_requested: false,
        }
    }

    pub fn capabilities() -> ServerCapabilities {
        ServerCapabilities {
            text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
            completion_provider: Some(CompletionOptions {
                trigger_characters: Some(vec![".".to_owned(), "\"".to_owned(), "[".to_owned()]),
                ..Default::default()
            }),
            hover_provider: Some(HoverProviderCapability::Simple(true)),
            definition_provider: Some(OneOf::Left(true)),
            references_provider: Some(OneOf::Left(true)),
            rename_provider: Some(OneOf::Right(RenameOptions {
                prepare_provider: Some(true),
                work_done_progress_options: Default::default(),
            })),
            document_symbol_provider: Some(OneOf::Left(true)),
            code_action_provider: Some(CodeActionProviderCapability::Options(CodeActionOptions {
                code_action_kinds: Some(vec![CodeActionKind::QUICKFIX]),
                ..Default::default()
            })),
            document_formatting_provider: Some(OneOf::Left(true)),
            ..Default::default()
        }
    }

    /// 处理单条 JSON-RPC 消息（字符串形态，供 Tauri 进程内传输层直接调用）。
    pub fn handle_json(&mut self, raw_json: &str) -> Vec<String> {
        let Ok(msg) = serde_json::from_str::<Message>(raw_json) else {
            return Vec::new();
        };
        let (outgoing, _) = self.handle_message(msg);
        outgoing
            .into_iter()
            .filter_map(|m| serde_json::to_string(&m).ok())
            .collect()
    }

    /// 处理单条 `lsp_server::Message`，返回待发出消息列表及是否应当退出事件循环。
    pub fn handle_message(&mut self, msg: Message) -> (Vec<Message>, bool) {
        match msg {
            Message::Request(req) => (self.handle_request(req), false),
            Message::Notification(notif) => self.handle_notification(notif),
            Message::Response(_) => (Vec::new(), false),
        }
    }

    fn handle_notification(&mut self, notif: Notification) -> (Vec<Message>, bool) {
        let mut out = Vec::new();
        match notif.method.as_str() {
            Exit::METHOD => return (out, true),
            Initialized::METHOD => {}
            DidChangeConfiguration::METHOD => {
                if let Some(new_lang) = notif
                    .params
                    .get("settings")
                    .and_then(|s| s.get("lang"))
                    .and_then(|v| v.as_str())
                    .filter(|l| matches!(*l, "zh" | "en"))
                {
                    self.lang = new_lang.to_owned();
                    let uris: Vec<Uri> = self.documents.keys().cloned().collect();
                    for uri in uris {
                        if let Some(m) = self.build_publish_diagnostics(&uri) {
                            out.push(m);
                        }
                    }
                }
            }
            DidOpenTextDocument::METHOD => {
                if let Ok(params) =
                    serde_json::from_value::<lsp_types::DidOpenTextDocumentParams>(notif.params)
                {
                    let uri = params.text_document.uri;
                    self.documents.insert(
                        uri.clone(),
                        OpenDoc {
                            text: params.text_document.text,
                            version: Some(params.text_document.version),
                        },
                    );
                    if let Some(m) = self.build_publish_diagnostics(&uri) {
                        out.push(m);
                    }
                }
            }
            DidChangeTextDocument::METHOD => {
                if let Ok(params) =
                    serde_json::from_value::<lsp_types::DidChangeTextDocumentParams>(notif.params)
                {
                    let uri = params.text_document.uri;
                    if let Some(change) = params.content_changes.into_iter().last() {
                        self.documents.insert(
                            uri.clone(),
                            OpenDoc {
                                text: change.text,
                                version: Some(params.text_document.version),
                            },
                        );
                        if let Some(m) = self.build_publish_diagnostics(&uri) {
                            out.push(m);
                        }
                    }
                }
            }
            DidSaveTextDocument::METHOD => {
                if let Ok(params) =
                    serde_json::from_value::<lsp_types::DidSaveTextDocumentParams>(notif.params)
                {
                    let uri = params.text_document.uri;
                    if let Some(text) = params.text
                        && let Some(doc) = self.documents.get_mut(&uri)
                    {
                        doc.text = text;
                    }
                    if let Some(m) = self.build_publish_diagnostics(&uri) {
                        out.push(m);
                    }
                }
            }
            DidCloseTextDocument::METHOD => {
                if let Ok(params) =
                    serde_json::from_value::<lsp_types::DidCloseTextDocumentParams>(notif.params)
                {
                    let uri = params.text_document.uri;
                    self.documents.remove(&uri);
                    let clear = PublishDiagnosticsParams {
                        uri,
                        diagnostics: Vec::new(),
                        version: None,
                    };
                    if let Ok(val) = serde_json::to_value(clear) {
                        out.push(Message::Notification(Notification::new(
                            PublishDiagnostics::METHOD.to_owned(),
                            val,
                        )));
                    }
                }
            }
            _ => {}
        }
        (out, false)
    }

    fn handle_request(&mut self, req: Request) -> Vec<Message> {
        let id = req.id.clone();

        if req.method == Shutdown::METHOD {
            self.shutdown_requested = true;
            return vec![Message::Response(Response::new_ok(
                id,
                serde_json::Value::Null,
            ))];
        }

        if req.method == Initialize::METHOD {
            if let Some(lang) = req
                .params
                .get("initializationOptions")
                .and_then(|o| o.get("lang"))
                .and_then(|v| v.as_str())
                .filter(|l| matches!(*l, "zh" | "en"))
            {
                self.lang = lang.to_owned();
            }
            let init_res = InitializeResult {
                capabilities: Self::capabilities(),
                server_info: Some(ServerInfo {
                    name: "weft-ls".to_owned(),
                    version: Some(env!("CARGO_PKG_VERSION").to_owned()),
                }),
            };
            let resp = match serde_json::to_value(init_res) {
                Ok(val) => Response::new_ok(id, val),
                Err(err) => Response::new_err(id, ErrorCode::InternalError as i32, err.to_string()),
            };
            return vec![Message::Response(resp)];
        }

        let result: Result<serde_json::Value, (ErrorCode, String)> = match req.method.as_str() {
            Completion::METHOD => serde_json::from_value::<lsp_types::CompletionParams>(req.params)
                .map_err(invalid_params)
                .map(|p| self.handle_completion(p)),
            HoverRequest::METHOD => serde_json::from_value::<lsp_types::HoverParams>(req.params)
                .map_err(invalid_params)
                .map(|p| self.handle_hover(p)),
            GotoDefinition::METHOD => {
                serde_json::from_value::<lsp_types::GotoDefinitionParams>(req.params)
                    .map_err(invalid_params)
                    .map(|p| self.handle_definition(p))
            }
            References::METHOD => serde_json::from_value::<lsp_types::ReferenceParams>(req.params)
                .map_err(invalid_params)
                .map(|p| self.handle_references(p)),
            PrepareRenameRequest::METHOD => {
                serde_json::from_value::<lsp_types::TextDocumentPositionParams>(req.params)
                    .map_err(invalid_params)
                    .map(|p| self.handle_prepare_rename(p))
            }
            Rename::METHOD => serde_json::from_value::<lsp_types::RenameParams>(req.params)
                .map_err(invalid_params)
                .map(|p| self.handle_rename(p)),
            DocumentSymbolRequest::METHOD => {
                serde_json::from_value::<lsp_types::DocumentSymbolParams>(req.params)
                    .map_err(invalid_params)
                    .map(|p| self.handle_document_symbols(p))
            }
            CodeActionRequest::METHOD => {
                serde_json::from_value::<lsp_types::CodeActionParams>(req.params)
                    .map_err(invalid_params)
                    .map(|p| self.handle_code_actions(p))
            }
            Formatting::METHOD => {
                serde_json::from_value::<lsp_types::DocumentFormattingParams>(req.params)
                    .map_err(invalid_params)
                    .map(|p| self.handle_formatting(p))
            }
            _ => Err((
                ErrorCode::MethodNotFound,
                format!("unhandled method: {}", req.method),
            )),
        };

        let response = match result {
            Ok(val) => Response::new_ok(id, val),
            Err((code, msg)) => Response::new_err(id, code as i32, msg),
        };
        vec![Message::Response(response)]
    }

    fn build_publish_diagnostics(&self, uri: &Uri) -> Option<Message> {
        let doc = self.documents.get(uri)?;
        let base = base_dir_for(uri);
        let diags = service::diagnostics(&doc.text, &base, &self.lang, true);
        let index = LineIndex::new(&doc.text);
        let lsp_diags = diags
            .into_iter()
            .map(|d| to_lsp_diagnostic(&doc.text, &index, uri, d))
            .collect();
        let params = PublishDiagnosticsParams {
            uri: uri.clone(),
            diagnostics: lsp_diags,
            version: doc.version,
        };
        let val = serde_json::to_value(params).ok()?;
        Some(Message::Notification(Notification::new(
            PublishDiagnostics::METHOD.to_owned(),
            val,
        )))
    }

    fn handle_completion(&self, params: lsp_types::CompletionParams) -> serde_json::Value {
        let uri = &params.text_document_position.text_document.uri;
        let Some(doc) = self.documents.get(uri) else {
            return serde_json::Value::Null;
        };
        let src = &doc.text;
        let index = LineIndex::new(src);
        let offset = index.lsp_to_byte(src, params.text_document_position.position);
        let Some(list) = service::completions(src, offset, &self.lang, true) else {
            return serde_json::Value::Null;
        };
        let replace_range = index.span_to_lsp_range(src, list.replace_span);
        let items: Vec<CompletionItem> = list
            .items
            .into_iter()
            .enumerate()
            .map(|(idx, item)| CompletionItem {
                label: item.label,
                kind: Some(match item.kind {
                    LsCompletionKind::Keyword => CompletionItemKind::KEYWORD,
                    LsCompletionKind::Property => CompletionItemKind::PROPERTY,
                    LsCompletionKind::Entity => CompletionItemKind::CLASS,
                    LsCompletionKind::Event => CompletionItemKind::EVENT,
                    LsCompletionKind::TimeAnchor => CompletionItemKind::CONSTANT,
                    LsCompletionKind::CalculableProp => CompletionItemKind::FUNCTION,
                }),
                detail: item.detail,
                documentation: item.documentation.map(|d| {
                    lsp_types::Documentation::MarkupContent(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: d,
                    })
                }),
                insert_text_format: Some(if item.is_snippet {
                    InsertTextFormat::SNIPPET
                } else {
                    InsertTextFormat::PLAIN_TEXT
                }),
                sort_text: Some(format!("{idx:04}")),
                text_edit: Some(lsp_types::CompletionTextEdit::Edit(TextEdit {
                    range: replace_range,
                    new_text: item.insert_text,
                })),
                ..Default::default()
            })
            .collect();
        serde_json::to_value(CompletionResponse::Array(items)).unwrap_or(serde_json::Value::Null)
    }

    fn handle_hover(&self, params: lsp_types::HoverParams) -> serde_json::Value {
        let uri = &params.text_document_position_params.text_document.uri;
        let Some(doc) = self.documents.get(uri) else {
            return serde_json::Value::Null;
        };
        let src = &doc.text;
        let index = LineIndex::new(src);
        let offset = index.lsp_to_byte(src, params.text_document_position_params.position);
        let base = base_dir_for(uri);
        let Some(h) = service::hover(src, &base, offset, &self.lang) else {
            return serde_json::Value::Null;
        };
        let hover = Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: h.contents,
            }),
            range: h.span.map(|s| index.span_to_lsp_range(src, s)),
        };
        serde_json::to_value(hover).unwrap_or(serde_json::Value::Null)
    }

    fn handle_definition(&self, params: lsp_types::GotoDefinitionParams) -> serde_json::Value {
        let uri = &params.text_document_position_params.text_document.uri;
        let Some(doc) = self.documents.get(uri) else {
            return serde_json::Value::Null;
        };
        let src = &doc.text;
        let index = LineIndex::new(src);
        let offset = index.lsp_to_byte(src, params.text_document_position_params.position);
        let Some(loc) = service::goto_definition(src, offset) else {
            return serde_json::Value::Null;
        };
        let resp = GotoDefinitionResponse::Scalar(Location {
            uri: uri.clone(),
            range: index.span_to_lsp_range(src, loc.span),
        });
        serde_json::to_value(resp).unwrap_or(serde_json::Value::Null)
    }

    fn handle_references(&self, params: lsp_types::ReferenceParams) -> serde_json::Value {
        let uri = &params.text_document_position.text_document.uri;
        let Some(doc) = self.documents.get(uri) else {
            return serde_json::Value::Null;
        };
        let src = &doc.text;
        let index = LineIndex::new(src);
        let offset = index.lsp_to_byte(src, params.text_document_position.position);
        let locs: Vec<Location> = service::references(src, offset, params.context.include_declaration)
            .into_iter()
            .map(|loc| Location {
                uri: uri.clone(),
                range: index.span_to_lsp_range(src, loc.span),
            })
            .collect();
        serde_json::to_value(locs).unwrap_or(serde_json::Value::Null)
    }

    fn handle_prepare_rename(
        &self,
        params: lsp_types::TextDocumentPositionParams,
    ) -> serde_json::Value {
        let uri = &params.text_document.uri;
        let Some(doc) = self.documents.get(uri) else {
            return serde_json::Value::Null;
        };
        let src = &doc.text;
        let index = LineIndex::new(src);
        let offset = index.lsp_to_byte(src, params.position);
        let Some(prep) = service::prepare_rename(src, offset) else {
            return serde_json::Value::Null;
        };
        let resp = PrepareRenameResponse::RangeWithPlaceholder {
            range: index.span_to_lsp_range(src, prep.span),
            placeholder: prep.placeholder,
        };
        serde_json::to_value(resp).unwrap_or(serde_json::Value::Null)
    }

    fn handle_rename(&self, params: lsp_types::RenameParams) -> serde_json::Value {
        let uri = &params.text_document_position.text_document.uri;
        let Some(doc) = self.documents.get(uri) else {
            return serde_json::Value::Null;
        };
        let src = &doc.text;
        let index = LineIndex::new(src);
        let offset = index.lsp_to_byte(src, params.text_document_position.position);
        let Some(edits) = service::rename(src, offset, &params.new_name) else {
            return serde_json::Value::Null;
        };
        let lsp_edits: Vec<TextEdit> = edits
            .into_iter()
            .map(|e| TextEdit {
                range: index.span_to_lsp_range(src, e.span),
                new_text: e.new_text,
            })
            .collect();
        let mut changes = HashMap::new();
        changes.insert(uri.clone(), lsp_edits);
        let edit = WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        };
        serde_json::to_value(edit).unwrap_or(serde_json::Value::Null)
    }

    #[allow(deprecated)]
    fn handle_document_symbols(
        &self,
        params: lsp_types::DocumentSymbolParams,
    ) -> serde_json::Value {
        let uri = &params.text_document.uri;
        let Some(doc) = self.documents.get(uri) else {
            return serde_json::Value::Null;
        };
        let src = &doc.text;
        let index = LineIndex::new(src);
        let symbols: Vec<DocumentSymbol> = service::document_symbols(src)
            .into_iter()
            .map(|s| DocumentSymbol {
                name: s.name,
                detail: s.detail,
                kind: match s.kind.as_str() {
                    "story" => LspSymbolKind::MODULE,
                    "entity" => LspSymbolKind::CLASS,
                    "event" => LspSymbolKind::EVENT,
                    "events" => LspSymbolKind::ARRAY,
                    "time" => LspSymbolKind::CONSTANT,
                    "link" => LspSymbolKind::INTERFACE,
                    _ => LspSymbolKind::OBJECT,
                },
                tags: None,
                deprecated: None,
                range: index.span_to_lsp_range(src, s.full_span),
                selection_range: index.span_to_lsp_range(src, s.name_span),
                children: None,
            })
            .collect();
        serde_json::to_value(DocumentSymbolResponse::Nested(symbols))
            .unwrap_or(serde_json::Value::Null)
    }

    fn handle_code_actions(&self, params: lsp_types::CodeActionParams) -> serde_json::Value {
        let uri = &params.text_document.uri;
        let Some(doc) = self.documents.get(uri) else {
            return serde_json::Value::Null;
        };
        let src = &doc.text;
        let index = LineIndex::new(src);
        let range = index.lsp_range_to_span(src, params.range);
        let base = base_dir_for(uri);
        let actions: Vec<CodeActionOrCommand> =
            service::code_actions(src, &base, range, &self.lang)
                .into_iter()
                .map(|a| to_lsp_code_action(src, &index, uri, a))
                .collect();
        serde_json::to_value(actions).unwrap_or(serde_json::Value::Null)
    }

    fn handle_formatting(&self, params: lsp_types::DocumentFormattingParams) -> serde_json::Value {
        let uri = &params.text_document.uri;
        let Some(doc) = self.documents.get(uri) else {
            return serde_json::Value::Null;
        };
        let src = &doc.text;
        match service::format_document(src, &self.lang) {
            Ok(formatted) if formatted != *src => {
                let index = LineIndex::new(src);
                let edits = vec![TextEdit {
                    range: index.full_range(src),
                    new_text: formatted,
                }];
                serde_json::to_value(edits).unwrap_or(serde_json::Value::Null)
            }
            _ => serde_json::to_value(Vec::<TextEdit>::new()).unwrap_or(serde_json::Value::Null),
        }
    }
}

fn invalid_params(err: serde_json::Error) -> (ErrorCode, String) {
    (ErrorCode::InvalidParams, err.to_string())
}

fn base_dir_for(uri: &Uri) -> PathBuf {
    uri_to_path(uri)
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn to_lsp_diagnostic(src: &str, index: &LineIndex, uri: &Uri, d: LsDiagnostic) -> LspDiagnostic {
    let range = d
        .span
        .map(|s| index.span_to_lsp_range(src, s))
        .unwrap_or_else(|| {
            Range::new(
                lsp_types::Position::new(0, 0),
                lsp_types::Position::new(0, 0),
            )
        });
    let mut message = d.message;
    if let Some(help) = d.help {
        message.push('\n');
        message.push_str(&help);
    }
    let related_information = if d.related.is_empty() {
        None
    } else {
        Some(
            d.related
                .into_iter()
                .map(|r| DiagnosticRelatedInformation {
                    location: Location {
                        uri: uri.clone(),
                        range: index.span_to_lsp_range(src, r.span),
                    },
                    message: r.message,
                })
                .collect(),
        )
    };
    LspDiagnostic {
        range,
        severity: Some(match d.severity {
            LsSeverity::Error => DiagnosticSeverity::ERROR,
            LsSeverity::Warning => DiagnosticSeverity::WARNING,
        }),
        code: Some(NumberOrString::String(d.rule)),
        source: Some("weft".to_owned()),
        message,
        related_information,
        ..Default::default()
    }
}

fn to_lsp_code_action(
    src: &str,
    index: &LineIndex,
    uri: &Uri,
    action: LsCodeAction,
) -> CodeActionOrCommand {
    let edits: Vec<TextEdit> = action
        .edits
        .into_iter()
        .map(|e| TextEdit {
            range: index.span_to_lsp_range(src, e.span),
            new_text: e.new_text,
        })
        .collect();
    let mut changes = HashMap::new();
    changes.insert(uri.clone(), edits);
    CodeActionOrCommand::CodeAction(CodeAction {
        title: action.title,
        kind: Some(CodeActionKind::QUICKFIX),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        ..Default::default()
    })
}

fn uri_to_path(uri: &Uri) -> Option<PathBuf> {
    let raw = uri.as_str();
    let stripped = raw.strip_prefix("file://")?;
    let decoded = percent_decode(stripped);
    #[cfg(windows)]
    let decoded = decoded
        .strip_prefix('/')
        .filter(|s| s.as_bytes().get(1) == Some(&b':'))
        .map(str::to_owned)
        .unwrap_or(decoded);
    Some(PathBuf::from(decoded))
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2]))
        {
            out.push((h << 4) | l);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
