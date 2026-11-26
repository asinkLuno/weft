# 在各 AI 客户端安装 weft-writer skill

`weft-writer` 是一个 [Agent Skills](https://agentskills.io/specification) 技能：模型识别到你要写、改、检查 `.weft` 文档时会自动加载它，按其中的流程调用 `weft check` / `weft fmt`。各客户端复用同一份技能，区别只在于安装方式。

| 客户端      | 安装方式                         |
| ----------- | -------------------------------- |
| Claude Code | 插件市场，两行命令               |
| pi          | `pi install` 装成一个包          |
| 其他客户端  | 把技能目录放进客户端的技能目录里 |

## 前置条件：装好 `weft` 命令

技能里的检查都通过 `weft` 命令执行，所以先确保它在 `PATH` 上：

```bash
cargo install --git https://github.com/asinkLuno/weft weft-cli
weft --help
```

安装与用法见 [weft CLI](weft-cli.md)。

## Claude Code

```text
/plugin marketplace add asinkLuno/weft
/plugin install weft-writer@weft
```

装好后在对话里直接用自然语言即可（“帮我写一个 .weft 章节”“跑一下 weft check”），不需要手动提及技能名。

## pi

```bash
pi install git:github.com/asinkLuno/weft        # 用户级：所有项目可用
pi install git:github.com/asinkLuno/weft -l     # 项目级：写进当前目录的 .pi/settings.json
pi remove git:github.com/asinkLuno/weft         # 卸载
```

`pi install` 会克隆仓库并装一遍仓库依赖，约 1 分钟。装好后技能随包生效，可用 `/skill:weft-writer` 强制加载，或让模型自行路由。

## 其他客户端

遵循 Agent Skills 规范的客户端（Cursor、Codex 等）都能用同一个目录：把仓库里的 `.agents/skills/weft-writer/` 整个复制到客户端的技能目录。通用位置是 `~/.agents/skills/`，只认 Claude 目录的客户端用 `~/.claude/skills/`。

没有 skill 机制时也可以照做：读 `.agents/skills/weft-writer/SKILL.md` 的流程，自己跑 `weft check --deny-warnings` 和 `weft fmt`。
