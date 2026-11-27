# Weft VS Code Extension

Provides syntax highlighting (`.weft` TextMate grammar) and full LSP support powered by `weft lsp`:

- Real-time syntax, semantic, and lint diagnostics (with quick fixes)
- Context-aware completions and snippets
- Hover documentation and resolved calendar date/time previews
- Go to Definition, Find All References, and Rename Symbol
- Document Outline and formatting (`weft fmt`)

## Prerequisites

Install the `weft` CLI (bundled with the Weft desktop app, or via `cargo install --path crates/weft-cli`). The extension runs `weft lsp` as a language server; see [Language Server (LSP)](../../docs/src/lsp.md) for the full feature list.

Settings:

- `weft.executablePath` — path to the `weft` binary (default `weft`).
- `weft.lang` — `auto`, `zh`, or `en`; controls diagnostics and hover docs. `auto` follows the VS Code display language.

## Development

```bash
cd editors/vscode
yarn install
yarn compile
```

Then open `editors/vscode` in VS Code and press `F5` to launch an Extension Development Host.
