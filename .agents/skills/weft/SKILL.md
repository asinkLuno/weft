---
name: weft
description: 把小说、文章或世界观设定整理成 Weft 故事文档（story.yaml）的总纲：YAML 顶层结构（story / entity / entity_link / event / narrative）与「三遍阅读法」——第一遍提炼 story 和 entity（含层级关系），第二遍提炼 event，第三遍组织 narrative。当用户想把一篇文章、小说、剧本、设定集变成 weft 格式的 story.yaml，要从零写或大改一份故事文档，或询问 weft YAML 的整体结构时使用。各段细节按阶段读子技能：weft-story（日历与插件）、weft-entity、weft-relation、weft-event-narrative。
---

# Weft 故事文档写作总纲

WeftDoc 用一个 YAML 文件（习惯命名 `story.yaml`）描述一个故事：元信息、实体、实体关系、事件、叙事线，并注册 Rune 插件（自定义历法、可计算属性）。除 `story` 外所有顶层段都可省略。完整范例：`examples/story.yaml`；输入规范：`docs/src/weftdoc.md`。

## 顶层骨架

| 顶层键 | 内容 | 哪一遍写 |
| --- | --- | --- |
| `story` | 元信息（title 必填）+ 历法与插件注册 | 第一遍 |
| `entity` | 实体表，键即实体 id | 第一遍 |
| `entity_link` | 实体两两关系，按组 | 第一遍 |
| `event` | 事件表，键即事件标题 | 第二遍 |
| `narrative` | 叙事线：名称 → 事件 id 列表 | 第三遍 |

最小骨架（只填必填项即可解析）：

```yaml
story:
  title: 故事标题
entity:
  主角:
    description: 一句话介绍。
event:
  开端:
    start_time: [1991, 9, 1]
    entities: [主角]
narrative:
  主角线: [开端]
```

## 三遍阅读法

把一篇文章转成 story.yaml 时**读三遍**，每遍只干一件事；进入每一遍之前先读对应子技能。

### 第一遍：提炼 story 和 entity

定框架，只看**名词**。

1. 写 `story`：标题、作者、简介；架空纪年或需要属性脚本时一并配置历法与插件 → 先读 `.agents/skills/weft-story/SKILL.md`
2. 圈出所有值得建卡的名词：人物、团体、地点、道具，逐个建实体 → 读 `.agents/skills/weft-entity/SKILL.md`
3. 搭结构：组织归属（`belong_to`）与两两关系（`entity_link`）→ 读 `.agents/skills/weft-relation/SKILL.md`

产出 `story` + `entity` + `entity_link` 三段。**收尾必须归一 id**：同一实体的多种叫法选一个、跨章节合并同一对象、统一容器命名——事件参与者全靠这套 id，归一不干净后面全是断引。

### 第二遍：提炼 event

只看**动词**：某一刻发生了什么。给每个有时间、有参与者的动态发生建条目，写 `start_time` / `end_time` / `entities` → 读 `.agents/skills/weft-event-narrative/SKILL.md` 的事件部分。前提：`entity` 表已定型——配角建不建实体直接决定事件参与者怎么写。

### 第三遍：组织 narrative

选**视角**串线：以某个人物或主题为线，把事件按讲述顺序排成叙事线 → 读 `.agents/skills/weft-event-narrative/SKILL.md` 的叙事线部分。

## 全局规则：id 与引用

- **实体 id** 是 `entity` 下的键，被 `belong_to`、`entity_link`、事件的 `entities` 引用；引用不存在的 id 解析失败。
- **事件 id** 是 `event` 下的键（即事件标题），被 `narrative` 引用。
- 归属不许成环；事件 `end_time` 不得早于 `start_time`。
- 时间分量按当前历法从大到小排列（内置公历为 年 月 日 时 分 秒），尾部可省略、必须是整数。
- 拼写陷阱：story 级注册键是 `calculatable_props`（带 ta），实体级引用键是 `calculable_props`（不带 ta）。
- 顶层未声明的键被静默丢弃；实体内部的未声明键进 `extra_props` 透传——拼错内置键不会报错，只会变成多余的自定义字段。
- 任何校验失败都会阻止整份文档展示；断开的引用按段批量报出。

## 交付前自检

- [ ] `story.title` 已填
- [ ] 所有被引用的实体 id / 事件 id 都存在
- [ ] `belong_to` 无环；`entity_link.entities` 恰好两个
- [ ] 每个事件 `end_time` 不早于 `start_time`
- [ ] 内置键拼写正确：`base_time` / `description` / `avatar` / `belong_to` / `calculable_props`
- [ ] 时间分量全为整数且不超过历法分量数（公历 6）
