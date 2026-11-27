import * as vscode from "vscode";
import {
  LanguageClient,
  type LanguageClientOptions,
  type ServerOptions,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

export function activate(_context: vscode.ExtensionContext): void {
  const config = vscode.workspace.getConfiguration("weft");
  const command = config.get<string>("executablePath", "weft");
  const langSetting = config.get<string>("lang", "auto");
  const lang =
    langSetting === "zh" || langSetting === "en"
      ? langSetting
      : vscode.env.language.toLowerCase().startsWith("zh")
        ? "zh"
        : "en";

  const serverOptions: ServerOptions = {
    command,
    args: ["lsp", "--lang", lang],
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "weft" }],
    initializationOptions: { lang },
  };

  client = new LanguageClient(
    "weftLanguageServer",
    "WEFT Language Server",
    serverOptions,
    clientOptions,
  );
  void client.start();
}

export async function deactivate(): Promise<void> {
  if (client) {
    await client.stop();
    client = undefined;
  }
}
