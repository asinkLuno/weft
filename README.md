<div align="center">

# WEFT

**W**ords **E**ngineered **F**or **T**ales.

*专为叙事创作设计的结构化故事描述语言与工具链*

[在线文档](https://asinkluno.github.io/weft/) · [语言指南](https://asinkluno.github.io/weft/weft-syntax.html) · [完整示例 (`examples/story.weft`)](./examples/story.weft)

</div>

______________________________________________________________________

## 简介

**WEFT** 是一套以工程化思维重构故事创作的工具链。它通过简洁、可读的 `.weft` 纯文本文件来描述与管理复杂的叙事结构——包括**元信息、时间锚点、实体、剧情事件、叙事线编排，以及带图论代数约束的实体时变图与剧情伏笔/阻塞图**。

作者无需在散乱的笔记与大纲间来回切换：写好 `.weft` 源文件后，既可通过 `weft` 命令行工具进行语法与防穿帮检查、自动格式化，也能借助 LSP 在主流编辑器中获得实时诊断与跳转体验，并由上层应用将其渲染为完整的故事视图。

## 核心特性

- **声明式双层图故事建模 (`.weft`)**：用 6 个正交顶层声明清晰定义世界观与剧情——`entity` 时变状态关系图（防阵营/道具吃书）与 `event` 剧情拓扑图（`acyclic + chronological` 校验伏笔与前置阻塞防穿帮）。
- **命令行工具 (`weft` CLI)**：提供开箱即用的语法与图论校验（Check）以及代码级统一格式化（Format），让长篇大纲像代码一样易于版本控制（Git）与协作。
- **编辑器智能支持 (LSP)**：内置语言服务器协议（Language Server Protocol），在编辑器中实时提供错误诊断、自动补全、悬停提示与定义跳转。
- **AI 协同创作 (AI Skill)**：支持在主流 AI 客户端中安装专属 Skill，让 AI 真正理解 `.weft` 语法规范，辅助你生成、扩写或梳理复杂剧情线。

## 快速开始

### 1. 安装

```bash
# 从源码安装 weft CLI
cargo install --path crates/weft-cli
```

其他安装方式（桌面端附带、`.deb` / `.rpm` / `.dmg` 安装包）见 [weft CLI 文档](https://asinkluno.github.io/weft/weft-cli.html)。

### 2. 编写你的第一个 `.weft` 文件

创建一个 `story.weft` 文件，描述故事的基本要素（详细语法请参阅 [语言指南](https://asinkluno.github.io/weft/weft-syntax.html) 中的“最小文档”章节）：

```text
story {
  title  = "哈利·波特与魔法石"
  author = "J.K. 罗琳"
}

entity 霍格沃茨魔法学校 {
  description = "英国的魔法学校，下设四大学院。"
}

entity 格兰芬多 {
  description = "以勇气著称的学院，院徽为狮子。"
}

entity 哈利·波特 {
  description = "大难不死的男孩。"
}

entity 隐形衣 {
  description = "波特家世代相传的隐形斗篷。"
}

event 分院帽晚宴 {
  start       = [1991, 9, 1, 19]
  end         = start + [0, 0, 0, 2] # 宴会两小时后结束
  entities    = [哈利·波特]
  description = "开学宴会上分院帽把新生分入四大学院。"
}

event 圣诞节礼物 {
  start       = [1991, 12, 25, 8]
  entities    = [哈利·波特, 隐形衣]
  description = "哈利在圣诞早晨收到父亲留下的隐形衣。"
}

event 夜探禁书区 {
  start       = [1991, 12, 25, 21]
  entities    = [哈利·波特, 隐形衣]
  description = "哈利披着隐形衣潜入禁书区，误入厄里斯魔镜房间。"
}

events 哈利的第一学年 = [分院帽晚宴, 圣诞节礼物, 夜探禁书区]

link 组织归属 {
  acyclic    = true
  transitive = true

  格兰芬多 -> 霍格沃茨魔法学校
  哈利·波特 -> 格兰芬多 {
    start = 分院帽晚宴.start
  }
}

link 剧情阻塞 {
  acyclic       = true
  chronological = true

  圣诞节礼物 -- "获得隐形衣" -> 夜探禁书区
}
```

### 3. 检查与格式化

使用 `weft` CLI 校验叙事逻辑与语法，并统一排版格式：

```bash
# 检查语法、时变图代数约束与剧情时序一致性
weft check story.weft

# 统一格式化文档
weft fmt story.weft
```

## 生态与集成

- **编辑器集成（LSP）**：配置 WEFT 语言服务器后，即可在 VS Code、Neovim、Zed 等支持 LSP 的编辑器中享受边写边看的诊断、补全、悬停与跳转体验。详细配置见 [语言服务器（LSP）文档](https://asinkluno.github.io/weft/lsp.html)。
- **AI 客户端集成**：支持将 WEFT Skill 接入你的 AI 助手，直接通过自然语言指令生成合法的 `.weft` 故事骨架。配置方法见 [在 AI 客户端安装 Skill](https://asinkluno.github.io/weft/skills.html)。
- **完整参考示例**：查看仓库中的 [`examples/story.weft`](./examples/story.weft)，了解完整故事项目的组织方式。

## 文档导航

完整的语法规范、CLI 参数说明及编辑器配置指南，请访问 **[WEFT 用户文档](https://asinkluno.github.io/weft/)**。

## License

[Apache-2.0](./LICENSE) © WEFT Contributors
