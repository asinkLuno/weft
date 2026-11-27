import {
  findReferencesKeymap,
  formatDocument,
  formatKeymap,
  hoverTooltips,
  jumpToDefinition,
  jumpToDefinitionKeymap,
  LSPClient,
  LSPPlugin,
  renameKeymap,
  serverCompletion,
  serverDiagnostics,
  type Transport,
} from "@codemirror/lsp-client";
import { EditorView, keymap } from "@codemirror/view";
import { invoke } from "@tauri-apps/api/core";
import type * as lsp from "vscode-languageserver-protocol";
import type { Lang } from "@/i18n";

export type WeftDeclKind = "entity" | "event" | "events" | "link";

export type DiagnosticCounts = {
  errors: number;
  warnings: number;
};

const WEFT_IDENT_VALID_RE = /^[\p{L}\p{N}_·・]*$/u;
const BACKSLASH_RE = /\\/g;
const WINDOWS_DRIVE_RE = /^[A-Za-z]:\//;

const DECL_KIND_TO_SYMBOL_KIND: Record<WeftDeclKind, lsp.SymbolKind> = {
  entity: 5, // Class (matches lsp_types::SymbolKind::CLASS)
  event: 24, // Event (matches lsp_types::SymbolKind::EVENT)
  events: 18, // Array (matches lsp_types::SymbolKind::ARRAY)
  link: 11, // Interface (matches lsp_types::SymbolKind::INTERFACE)
};

/** 将本地文件路径转换为标准 `file://` URI（兼容 POSIX 与 Windows 路径）。 */
export function pathToFileUri(filePath: string): string {
  const normalized = filePath.replace(BACKSLASH_RE, "/");
  const prefixed = WINDOWS_DRIVE_RE.test(normalized)
    ? `/${normalized}`
    : normalized.startsWith("/")
      ? normalized
      : `/${normalized}`;
  return `file://${encodeURI(prefixed)}`;
}

/**
 * 进程内 Tauri LSP Transport：
 * 串行化 JSON-RPC 投递顺序，确保 `initialize` / `didOpen` / `didChange` / 请求严格按序执行。
 */
export function createTauriLspTransport(): Transport {
  const handlers = new Set<(value: string) => void>();
  let queue: Promise<void> = Promise.resolve();

  return {
    send(message: string) {
      queue = queue
        .then(async () => {
          const replies = await invoke<string[]>("lsp_send", { message });
          for (const reply of replies) {
            for (const handler of handlers) {
              handler(reply);
            }
          }
        })
        .catch((err) => {
          console.error("lsp_send failed:", err);
        });
    },
    subscribe(handler) {
      handlers.add(handler);
    },
    unsubscribe(handler) {
      handlers.delete(handler);
    },
  };
}

export function createWeftLspClient(
  lang: Lang,
  onDiagnosticCounts: (counts: DiagnosticCounts) => void,
): LSPClient {
  return new LSPClient({
    initializationOptions: { lang },
    extensions: [
      serverCompletion({ override: true, validFor: WEFT_IDENT_VALID_RE }),
      hoverTooltips(),
      keymap.of([
        ...formatKeymap,
        ...renameKeymap,
        ...jumpToDefinitionKeymap,
        ...findReferencesKeymap,
      ]),
      serverDiagnostics(),
    ],
    notificationHandlers: {
      "textDocument/publishDiagnostics": (
        _client,
        params: lsp.PublishDiagnosticsParams,
      ) => {
        let errors = 0;
        let warnings = 0;
        for (const diag of params.diagnostics) {
          if ((diag.severity ?? 1) === 1) errors += 1;
          else warnings += 1;
        }
        onDiagnosticCounts({ errors, warnings });
        // 返回 false，继续交由内置 serverDiagnostics() 渲染编辑器下划线与提示
        return false;
      },
    },
  });
}

export function notifyLspLanguage(client: LSPClient, lang: Lang): void {
  if (!client.connected) return;
  client.notification("workspace/didChangeConfiguration", {
    settings: { lang },
  });
}

export async function revealSymbolViaLsp(
  client: LSPClient,
  view: EditorView,
  uri: string,
  kind: WeftDeclKind,
  name: string,
): Promise<boolean> {
  if (!client.connected) return false;
  await client.initializing;
  client.sync();
  const plugin = LSPPlugin.get(view);
  if (!plugin) return false;

  const symbols = await client
    .request<lsp.DocumentSymbolParams, lsp.DocumentSymbol[] | null>(
      "textDocument/documentSymbol",
      { textDocument: { uri } },
    )
    .catch(() => null);
  if (!symbols) return false;

  const expectedKind = DECL_KIND_TO_SYMBOL_KIND[kind];
  const matched = symbols.find(
    (s) => s.kind === expectedKind && s.name === name,
  );
  if (!matched) return false;

  const from = plugin.fromPosition(matched.selectionRange.start);
  const to = plugin.fromPosition(matched.selectionRange.end);
  view.dispatch({
    selection: { anchor: from, head: to },
    effects: EditorView.scrollIntoView(from, { y: "center" }),
  });
  view.focus();
  return true;
}

export { formatDocument, jumpToDefinition };
