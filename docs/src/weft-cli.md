# weft CLI：检查、编译、格式化与 LSP

`crates/weft-cli` 提供 `weft` 命令，负责五件事：编译检查（`check`）、编译导出 JSON（`build`）、格式化（`fmt`）、启动语言服务器（`lsp`）、列出 lint 规则（`rules`）。语言本身见 [WEFT 语言指南](weft-syntax.md)。

## 安装与调用

### 方式一：随 WEFT 桌面端自动附带（推荐）

安装 WEFT 桌面端安装包时会自动附带独立的 `weft` CLI 二进制：

- **Windows（`.exe` / `.msi`）**：安装程序会自动将 `weft` 所在目录加入 `PATH`，重开终端即可直接运行 `weft check` / `weft fmt`。
- **Linux（`.deb` / `.rpm`）**：安装后直接提供 `/usr/bin/weft`。如使用 **AppImage**，在应用菜单点击 **设置 → 安装 weft 命令行工具…**（会复制到 `~/.local/bin/weft`）。
- **macOS（`.dmg`）**：拖入 `/Applications` 后，在应用菜单点击 **设置 → 安装 weft 命令行工具…**（软链到 `/usr/local/bin/weft`）。

### 方式二：从源码安装或开发期调用

仓库里已经把它设为 workspace 的默认成员，所以在根目录直接跑：

```bash
cargo run -- check .          # 编译检查
cargo run -- build story.weft # 编译为结构化 JSON
cargo run -- fmt .            # 原地格式化
cargo run -- lsp              # 启动 LSP 3.17 语言服务器（stdio）
cargo run -- rules            # 列出 lint 规则
```

也可以用 Cargo 单独装成全局命令（适合 CI 或无桌面环境）：

```bash
cargo install --path crates/weft-cli
weft check .
```

`check` 和 `fmt` 都接受多个文件或目录；不给路径时默认 `.`。目录会递归查找 `*.weft`，遵守 `.gitignore`，并按路径排序去重。用 `-` 表示标准输入。

## check：编译检查

```bash
weft check story.weft                 # 单文件，默认 full 输出
weft check chapters/                  # 整个目录
weft check . --output-format concise  # 一行一条，适合编辑器
weft check - < story.weft             # 从 stdin 读
```

检查分三层，任一失败都会报诊断：

1. **语法**：词法/语法错误。解析器按文件恢复，尽量一次报全。
2. **语义**：必填字段、未知字段、重复声明、引用存在性、归属成环、时间环、`end < start`、脚本与头像路径、可计算属性注册等。
3. **Lint**：默认只是警告，不阻止退出码，除非加 `--deny-warnings`。

| 选项                                          | 作用                                                                                         |
| --------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `--output-format full\|concise\|github\|json` | 输出格式，默认 `full`                                                                        |
| `--syntax-only`                               | 只做语法/静态语义，不加载、不执行 `.rn` 脚本；也会跳过头像存在性检查，适合快速编辑和 CI 预检 |
| `--lang zh\|en`                               | 诊断语言；默认按 `LC_ALL` / `LC_MESSAGES` / `LANG` 推断，中文环境为 `zh`                     |
| `--ignore <rule>`                             | 忽略指定 lint 规则，可逗号分隔或重复传                                                       |
| `--deny-warnings`                             | 把警告当失败，适合 CI                                                                        |
| `--no-color`                                  | 关闭颜色                                                                                     |

输出格式：

- `full`：带源码片段、下划线和高亮，给人看。
- `concise`：`file:line:col: error[rule] message`，给编辑器/脚本解析。
- `github`：`::warning file=...,line=...,col=...,title=rule::message`，GitHub Actions 会自动把它渲染成注解。
- `json`：诊断数组，字段稳定，适合自己写工具消费。

没有诊断时退出码为 0；有错误时为 1；`--deny-warnings` 下出现警告也是 1。路径无法收集、文件读不了等环境问题退出码为 2，和“代码有问题”区分开。

## build：编译导出 JSON

```bash
weft build story.weft                     # 编译并输出美化 JSON 到 stdout
weft build story.weft -o dist/story.json  # 写入文件
weft build story.weft --compact           # 单行紧凑 JSON
weft build story.weft --lang en           # 指定历法与诊断语言
weft compile story.weft                   # compile 是 build 的别名
```

会执行完整的 `.rn` 历法与可计算属性脚本，输出供外部工具、脚本或静态站点消费的 `WeftDoc` JSON。存在编译错误时打印诊断并以退出码 1 退出。

## lsp：语言服务器（LSP 3.17）

```bash
weft lsp              # 按系统 locale 启动 stdio LSP 服务器
weft lsp --lang zh    # 强制中文诊断与 Hover 文档
```

完整的 LSP 3.17 服务器：实时诊断与 Quick Fix、上下文补全、Hover（含历法求解后的时间）、跳转定义、查找引用、重命名、文档大纲、整篇格式化。桌面端已内置同一套语言服务；VS Code 扩展在 `editors/vscode/`。编辑器配置、各功能细节与限制见[语言服务器](lsp.md)。

## rules：规则清单

```bash
weft rules
```

打印 `duplicate-attr`、`unused-time-anchor`、`unused-entity`、`unreferenced-event`、`self-link`、`duplicate-link`、`empty-block` 七条规则。各规则的触发条件见[语言指南的 lint 表](weft-syntax.md#%E6%A0%A1%E9%AA%8C%E4%B8%8E-lint)。

## fmt：格式化

```bash
weft fmt .                       # 原地重写所有 .weft
weft fmt story.weft --check      # 只检查，不写文件
weft fmt . --diff                # 打印 unified diff
weft fmt --stdin < story.weft    # 格式化 stdin，结果写到 stdout
weft format .                    # format 是 fmt 的别名
```

| 选项         | 作用                                                  |
| ------------ | ----------------------------------------------------- |
| `--check`    | 不写文件；有文件需要改时打印 `Would reformat: <path>` |
| `--diff`     | 不写文件；打印 unified diff                           |
| `--stdin`    | 从 stdin 读，格式化结果写 stdout（`-` 等价）          |
| `--no-color` | 关闭 diff 颜色                                        |

格式化规则（对齐、88 列折行、多行字符串、引号策略、注释保留等）见[语言指南的格式化约定](weft-syntax.md#%E6%A0%BC%E5%BC%8F%E5%8C%96%E7%BA%A6%E5%AE%9A)。两个关键性质：

- **幂等**：格式化过的文件再跑一次不会变。
- **不猜**：文件有语法错误时拒绝格式化并打印诊断；无法无损表达的布局保持原样。

退出码：0 表示无需改动/已全部改好；1 表示有文件需要改动（`--check`/`--diff`）或存在语法错误；2 表示读写失败。

## 推荐工作流

编辑时先跑便宜的检查，提交前跑完整的：

```bash
# 快速反馈：不执行 Rune 脚本
cargo run -q -- check . --syntax-only --output-format concise

# 提交前：完整检查 + 格式一致性
cargo run -q -- check . --deny-warnings
cargo run -q -- fmt --check .
```

CI 里用 `github` 格式，诊断会直接长在 diff 上：

```yaml
- run: cargo run -q -- check . --output-format github --deny-warnings
- run: cargo run -q -- fmt --check .
```

`fmt` 会保留注释，所以放心让它改；但它是“先检查再重写”，语法错误时不会动文件，避免把半成品改坏。

## 排错

| 现象                         | 处理                                                                            |
| ---------------------------- | ------------------------------------------------------------------------------- |
| 退出码 2                     | 路径写错、文件无权限，或 JSON 输出序列化失败；先确认路径存在                    |
| 改了 `.rn` 但 `check` 没反应 | 用了 `--syntax-only`，它不执行脚本；去掉再跑                                    |
| 文件名带空格/中文导致路径错  | 用引号包住，或用 `--stdin`                                                      |
| 警告太多想先专注错误         | `--ignore unused-entity,unused-time-anchor`，或先只看 `--output-format concise` |
| 格式化结果不符合预期         | 先 `--diff` 看差异；`fmt` 只做布局，不改语义，若语义变了那是 bug                |
