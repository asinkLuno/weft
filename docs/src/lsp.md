# 语言服务器（LSP）

`weft lsp` 是一个完整的 LSP 3.17 语言服务器：在编辑器里实时诊断、补全、悬停文档、跳转定义、查找引用、重命名、文档大纲与格式化。桌面端内置同一套语言服务，不经过子进程；外部编辑器通过 `weft lsp` 的 stdio 接入。

功能覆盖：

| 能力     | LSP 方法                          | 说明                                    |
| -------- | --------------------------------- | --------------------------------------- |
| 实时诊断 | `textDocument/publishDiagnostics` | 语法 / 语义 / lint 三层，带 Quick Fix   |
| 补全     | `textDocument/completion`         | 上下文感知，带片段与文档                |
| 悬停     | `textDocument/hover`              | 符号说明、字段文档、历法求解后的时间    |
| 跳转定义 | `textDocument/definition`         | 跳到当前文件内的声明                    |
| 查找引用 | `textDocument/references`         | 当前文件内所有引用                      |
| 重命名   | `textDocument/rename`             | 支持 `prepareRename`，自动加引号 / 转义 |
| 文档大纲 | `textDocument/documentSymbol`     | 顶层声明，供大纲、面包屑与“跳转到符号”  |
| 快速修复 | `textDocument/codeAction`         | 仅 `quickfix`                           |
| 格式化   | `textDocument/formatting`         | 整篇格式化，与 `weft fmt` 同一引擎      |

补全的触发字符是 `.`、`"`、`[`；主动请求（如 `Ctrl+Space`）不受触发字符限制。

## 启动与配置

```bash
weft lsp              # 按系统 locale 启动 stdio 服务器
weft lsp --lang zh    # 强制中文诊断与 Hover 文档
```

诊断与 Hover 的语言优先级：

1. `--lang zh|en`（启动参数）；
2. `initialize` 的 `initializationOptions.lang`；
3. `workspace/didChangeConfiguration` 的 `settings.lang`，切换后会立刻重新发布所有已打开文档的诊断；
4. 以上都没有时，按 `LC_ALL` / `LC_MESSAGES` / `LANG` 推断，默认英文。

服务器采用**全量同步**：每次 `didChange` 都会用最新全文重新分析，不维护增量解析状态。`didClose` 后会清空该文档的诊断。

## 编辑器接入

### VS Code

仓库的 `editors/vscode/` 提供官方扩展，包含 TextMate 语法高亮并自动启动 `weft lsp`。设置项：

- `weft.executablePath`：`weft` 可执行文件路径，默认 `weft`；
- `weft.lang`：`auto` / `zh` / `en`，默认跟随 VS Code 界面语言。

开发期在该目录执行 `yarn install && yarn compile`，用 VS Code 打开后按 `F5` 启动扩展开发宿主。

### Neovim（0.10+ 内置 LSP）

```lua
vim.filetype.add({ extension = { weft = "weft" } })

vim.api.nvim_create_autocmd("FileType", {
  pattern = "weft",
  callback = function(ev)
    vim.lsp.start({
      name = "weft",
      cmd = { "weft", "lsp", "--lang", "zh" },
      root_dir = vim.fs.dirname(vim.api.nvim_buf_get_name(ev.buf)),
    }, { bufnr = ev.buf })
  end,
})
```

### Helix

`languages.toml`：

```toml
[language-server.weft]
command = "weft"
args = ["lsp"]

[[language]]
name = "weft"
scope = "source.weft"
file-types = ["weft"]
comment-token = "#"
language-servers = ["weft"]
```

没有 tree-sitter 语法时高亮会缺失，但 LSP 功能不受影响。

### 其他编辑器

Zed、Sublime LSP、Emacs `lsp-mode` 等只要能把 `.weft` 关联到一个语言、并把服务器命令配置为 `weft lsp` 即可；语言服务器不依赖工作区里的其他文件。

### 桌面端

WEFT 桌面应用内嵌同一个 `LspServer`（经 Tauri 的 `lsp_send` 与 CodeMirror 的 LSP 客户端通信），无需安装或配置。快捷键：

| 操作     | 快捷键        |
| -------- | ------------- |
| 格式化   | `Shift+Alt+F` |
| 重命名   | `F2`          |
| 跳转定义 | `F12`         |
| 查找引用 | `Shift+F12`   |

界面语言切换会同步切换诊断与 Hover 语言；编辑器底部显示实时的错误 / 警告计数。

## 诊断

打开、修改、保存文档时自动发布，分三层（与 `weft check` 一致）：

1. **语法**：词法 / 语法错误，解析器按文件恢复，尽量一次报全；
2. **语义**：必填字段、未知字段、重复声明、引用存在性、归属成环、时间环、`end < start`、脚本与头像路径、可计算属性注册等；
3. **Lint**：`duplicate-attr`、`unused-time-anchor`、`unused-entity`、`unreferenced-event`、`self-link`、`duplicate-link`、`empty-block`。

每个诊断：

- `severity` 为 `Error` 或 `Warning`，`source` 为 `weft`，`code` 就是规则名（如 `unknown-time-ref`、`unused-entity`）；
- 相关的另一处位置（例如引用的声明、成环的对端）通过 `relatedInformation` 给出；
- 有帮助文本时附在消息尾部；
- “是不是想写……”这类建议会同时出现在 Quick Fix 里。

诊断会执行 `.rn` 历法与可计算属性脚本，也会检查头像文件是否存在——和完整 `weft check` 一样。文档很大或脚本较慢时，每次输入都会重新执行；需要更轻的编辑体验时可在本地用 `weft check --syntax-only` 做快速检查。

## Quick Fix

把光标放在诊断范围内，通过编辑器的“快速修复”菜单触发：

- **替换为建议的名字**：`unknown-*` 等带建议的诊断，一步改成正确的实体 / 事件 / 锚点；
- **删除整行**：`self-link`、`duplicate-link`、`unused-time-anchor` 可以直接删掉出问题的那一行。

快速修复按请求时重新跑一次完整检查，只返回与当前选区重叠的诊断对应的操作。

## 补全

| 光标位置                                           | 内容                                                                                                       |
| -------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| 文件顶层                                           | `story` / `entity` / `event` / `link` / `events` / `time` 片段；已有 `story` 时片段更短且降权              |
| 块内属性名                                         | 该块允许的字段，附字段说明与值片段；已写过的字段降权                                                       |
| `事件名.` / `实体名.`                              | 事件的 `start` / `end`（事件没声明 `end` 时 `end` 降权），实体的 `base_time`                               |
| 实体、事件、可计算属性、关系引用                   | 当前文档已声明的对应符号，排除自引用；文档里带描述时会显示                                                 |
| 时间字段（`start` / `end` / `base_time` / `time`） | `start + [...]`（仅事件的 `end`）、`start`、已有时间锚点、`事件名.start`、`事件名.end`、`实体名.base_time` |
| 字符串内                                           | 自动补齐引号并转义内容，避免名字里出现引号时破坏语法                                                       |

片段使用 `${1:占位符}` 语法，编辑器的 `Tab` 可以逐个跳转。补全项带 `detail`（如“实体”“事件开始时间”）和 `documentation`。

## 悬停

悬停内容按位置分三类：

- **符号**：`entity` 显示描述、基准时间（按 `date_mode` 历法换算后的人类可读形式）和归属；`event` 显示描述、`start → end` 时间区间和参与实体；`events` 显示包含的事件数；`link` 显示边数；`time` 锚点显示名字；可计算属性显示对应的 `.rn` 脚本。
- **内置字段名**：显示字段文档，必填字段标注“（必填）”。
- **顶层关键字**：显示该声明的用途。

符号悬停会尝试编译文档，以便给出历法求解后的时间；编译失败时退回静态声明信息（描述、归属、参与实体），悬停仍然可用。

## 定义、引用与重命名

- 支持符号：实体、事件、事件列表、时间锚点、关系组、可计算属性；
- **跳转定义**跳到本文件内的声明处；**查找引用**列出本文件内的全部出现，可包含或不包含声明；
- **重命名**只对已在文档中声明的符号生效：先由 `prepareRename` 返回可编辑范围与占位名，再一次性改写所有出现；
- 新名字会自动规范化：必要时加引号、转义特殊字符，因此可以直接输入中文或带空格的名字。

这些操作都以**当前文档**为范围，不做跨文件索引。

## 文档大纲

`documentSymbol` 返回顶层声明的名称、类型、细节与完整范围：`story` 映射为 Module，`entity` 为 Class，`event` 为 Event，`events` 为 Array，`time` 为 Constant，`link` 为 Interface。桌面端的大纲、点击跳转和“在编辑器中定位”都基于它。

## 格式化

`textDocument/formatting` 与 `weft fmt` 共用同一个格式化器：整篇重写、幂等、保留注释。内容已经符合格式时返回空编辑；文件存在语法错误时**不返回任何编辑**（不会把半成品改坏），错误本身由诊断通道给出。不提供选区格式化。

## 实现与限制

- 语言服务器复用 `crates/weft-ls`，桌面端进程内调用，`weft lsp` 只负责 stdio 传输；两者的能力完全一致。
- 语义分析是**单文件**的：定义、引用、重命名、补全都只看当前文档，`weft check` 的多文件检查能力不在 LSP 里。
- 使用全量文本同步，没有增量解析；大文档的补全 / 悬停 / 诊断成本随文件大小增长。
- 位置编码按 LSP 默认的 UTF-16 计算，中文、emoji 的列号与编辑器一致。
- 暂不提供：语义高亮（semantic tokens）、折叠范围、inlay hints、签名帮助、选区范围、Code Lens、工作区符号、跨文件编辑、拉取式诊断（pull diagnostics）、`completionItem/resolve`。
- 除上表列出的方法外，其余请求返回 `MethodNotFound`；`shutdown` 正常应答后，收到 `exit` 通知即退出。
