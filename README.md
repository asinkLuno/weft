<div align="center">

# WEFT

**W**ords **E**ngineered **F**or **T**ales.

*专为叙事创作设计的结构化故事描述语言与工具链*

[在线文档](https://asinkluno.github.io/weft/) · [语言指南](https://asinkluno.github.io/weft/weft-syntax.html) · [完整示例 (`examples/story.weft`)](./examples/story.weft)

</div>

______________________________________________________________________

## 简介

**WEFT** 是一套以工程化思维重构故事创作的工具链。它通过简洁、可读的 `.weft` 纯文本文件来描述与管理复杂的叙事结构——包括**元信息、实体与归属、实体关系、事件及事件列表**。

作者无需在散乱的笔记与大纲间来回切换：写好 `.weft` 源文件后，既可通过 `weft` 命令行工具进行语法检查与自动格式化，也能借助 LSP 在主流编辑器中获得实时诊断与跳转体验，并由上层应用将其渲染为完整的故事视图。

## 核心特性

- **声明式故事建模 (`.weft`)**：用纯文本清晰定义故事的世界观要素——元信息、角色/物品等实体及其归属、多维实体关系，以及严谨的时间线事件流。
- **命令行工具 (`weft` CLI)**：提供开箱即用的语法校验（Check）与代码级统一格式化（Format），让长篇大纲像代码一样易于版本控制（Git）与协作。
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
  belong_to   = [霍格沃茨魔法学校]
}

entity 哈利·波特 {
  description = "大难不死的男孩。"
  belong_to   = [格兰芬多]
}

event 分院帽晚宴 {
  start       = [1991, 9, 1, 19]
  end         = start + [0, 0, 0, 2] # 宴会两小时后结束
  entities    = [哈利·波特]
  description = "开学宴会上分院帽把新生分入四大学院。"
}

events 哈利的第一学年 = [分院帽晚宴]
```

### 3. 检查与格式化

使用 `weft` CLI 校验叙事逻辑与语法，并统一排版格式：

```bash
# 检查语法与引用完整性
weft check story.weft

# 统一格式化文档
weft fmt story.weft
```

## 生态与集成

- **编辑器集成（LSP）**：配置 WEFT 语言服务器后，即可在 VS Code、Neovim、Zed 等支持 LSP 的编辑器中享受边写边看的诊断、补全、悬停与跨文件跳转体验。详细配置见 [语言服务器（LSP）文档](https://asinkluno.github.io/weft/lsp.html)。
- **AI 客户端集成**：支持将 WEFT Skill 接入你的 AI 助手，直接通过自然语言指令生成合法的 `.weft` 故事骨架。配置方法见 [在 AI 客户端安装 Skill](https://asinkluno.github.io/weft/skills.html)。
- **完整参考示例**：查看仓库中的 [`examples/story.weft`](./examples/story.weft)，了解完整故事项目的组织方式。

## 文档导航

完整的语法规范、CLI 参数说明及编辑器配置指南，请访问 **[WEFT 用户文档](https://asinkluno.github.io/weft/)**。

## License

[Apache-2.0](./LICENSE) © WEFT Contributors
