---
name: weft-event-narrative
description: Weft 的 event 与 narrative 提炼：什么算事件（有时间、有参与者的动态发生，而非背景状态）、事件时间写法（标量、年月日列表、YAML 锚点 &id/*id 复用加偏移表达持续时段）、end 不得早于 start；第三遍如何站在观察者视角把事件按叙事顺序串成 narrative（名称 → 事件 ID 列表）。当用户要写或改 event / narrative 段、拿不准情节该记成事件还是描述，或排查 UnknownEventEntity / EventEndBeforeStart / UnknownNarrativeEvent 报错时使用。
---

# Weft event 与 narrative：第二遍提炼事件，第三遍组织叙事线

## 第二遍：提炼 event

事件是**某一刻发生了什么**：有时间、有参与者、推动情节。判断标准：

- **建**：开学宴分院、魔镜现身、袭击发生——有时间可考、有实体参与、写上时间线有意义的动态事实。
- **不建**：状态与背景（某人很勇敢、两个学院世仇）→ 写进实体 `description` 或 `extra_props`；没有时间坐标的设定描写。
- 拿不准时问：它应该出现在时间线上吗？

`event` 是扁平映射：键即事件标题（同时也是事件 id，被 narrative 引用），值四个字段：

```yaml
event:
  分院帽晚宴:
    start_time: &feast [1991, 9, 1, 19]    # 必填
    end_time: [*feast, 0, 0, 0, 2]         # 可选：锚点 +2 小时后结束
    entities: [哈利·波特, 罗恩·韦斯莱]     # 参与实体 id，须已定义
    description: 开学宴会上分院帽把新生分入四大学院。
```

标题用短名词短语（「分院帽晚宴」），它就是 id，需要全文档唯一；同类场景反复出现时标题加限定词消歧（`速胜赫奇帕奇` vs `魁地奇比赛扫帚失控`）。

### 粒度与转述

- 一个事件 = 一段**可独立讲述的转折**。同夜的连环小转折（伤疤灼痛、宣布禁令）并入主事件写进 description；连续闯关（一夜连过四道机关）是同一晚的一件事，建一个事件。
- 跨多日的持续事态（信件连日轰炸）按转折点拆分，或用一个事件的 start/end 表达跨度。
- 无现场场景、只在对话中被转述的大事（多年前的灭门案）照样建——它值得上时间线，description 注明是回溯。
- 公告类动态（开学宴上宣布禁令）通常并入所发生场合的主事件，不单列。
- 参与者按**事件实际发生时的在场者**列：幕后主使算在场（哪怕文中尚未揭晓）；仅被怀疑但不在场的不列。不必列全所有在场者，列对事件有意义的。

前提：第二遍开始前 `entity` 表必须已定型——配角建不建实体直接决定事件的 `entities` 怎么写。

### 时间写法

- 与实体 `base_time` 同一套规则：标量、按历法从大到小的整数列表（尾部可省）、首项为数组的锚点偏移；分量必须整数、不超历法分量数（公历 6）。完整规则见 `docs/src/design/phase.md`。
- 持续时段的标准姿势是 **YAML 锚点**：`start_time: &名 [绝对时间]`，`end_time: [*名, 逐年, 逐月, 逐日, 逐时, …]`。
- 锚点可以**跨事件链式复用**：选定一个文本依据最硬的日期（生日、开学日）作主锚点，第一个事件写 `&birthday [1991, 7, 31]`，其余事件全部相对它偏移（`[*birthday, 0, 0, -7]`、`[*birthday, 0, 0, 0, 8, 30]`）——整条时间链只需一个绝对日期。
- 偏移从**第一个分量（年槽）对齐**：加 2 小时要写 `[*feast, 0, 0, 0, 2]`，写 `[*feast, 2]` 是加 2 年。负数 = 减。
- 原文没有时长依据就**省略** `end_time`，不要编；有「宴会两小时后结束」这类依据才写。
- `end_tick < start_tick` 报 `EventEndBeforeStart`，整份文档解析失败。

### 参与者

`entities` 只存实体 id；引用不存在的 id 报 `UnknownEventEntity`（断引统一批量报出）。不必列全所有在场者，列对事件有意义的参与者。

## 第三遍：组织 narrative

叙事线 = **一个视角下的有序事件列表**。`narrative` 是 名称 → 事件 id 列表，书写顺序原样保留：

```yaml
narrative:
  哈利的第一学年: [分院帽晚宴, 厄里斯魔镜, 蛇怪袭击]
```

- 名称体现视角：一条线跟随一个观察者（「哈利的第一学年」）或一条主题（「魔法石争夺战」）。
- 列表顺序是**叙事顺序**（讲述的先后），不必等于时间顺序——倒叙、插叙靠顺序表达。
- 同一事件可出现在多条线里；一条线通常三五到十来个事件，宁缺毋滥。
- 引用必须存在，否则 `UnknownNarrativeEvent`。

## 常见报错

| 症状 | 原因 |
| --- | --- |
| `UnknownEventEntity` | 事件 `entities` 引用未定义实体 |
| `EventEndBeforeStart` | `end_time` 早于 `start_time` |
| `UnknownNarrativeEvent` | 叙事线引用未定义事件 |
| `TimeNonemptyList` / `TimeIntegerComponent` / `TooManyTimeComponents` | 时间是空列表或字符串 / 分量非整数 / 超出历法分量数 |

权威参考：`docs/src/weftdoc.md`、`docs/src/design/event-narrative.md`、`examples/story.yaml` 的 event / narrative 段。
