---
name: entity
description: Weft 故事文档的 entity（实体）：什么是实体、如何在 story.yaml 的 entity 段定义实体（base_time、avatar、belong_to、calculable_props、extra_props）、entity_link 关系对、归属 DAG 与解析校验规则。当用户询问实体概念、要编写或修改 YAML 中的 entity / entity_link 段、声明可计算属性或排查实体解析报错时使用。
---

# Weft Entity：什么是实体，如何定义

实体是 Weft 故事文档中的**名词**：人物、团体、地点、道具，一切能被事件「参与」、能互相归属或建立关系的东西都是实体。实体在 `story.yaml` 顶层 `entity` 键下定义，键即实体 id（实体 id 在全文档内被 `belong_to`、`entity_link`、事件的 `entities`、叙事线的 `observer` 引用，必须唯一且存在）。

权威参考：`docs/src/weftdoc.md`（输入规范）、`docs/src/design/entity.md`（构建流程）、`src-tauri/src/weft/entity.rs`（实现）。完整示例见 `examples/story.yaml`。

## 最小定义

```yaml
entity:
  哈利·波特:
    description: 大难不死的男孩。
```

实体所有字段都是可选的——只有一个 id 也算合法实体（如纯粹的地点、团体节点）。

## 内置字段

`RawEntity`（`entity.rs:22`）恰好有五个内置输入键，**其余任何键原样收进 `extra_props` 透传给前端**，不参与校验：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `base_time` | Phase：标量整数 / 分量数组 / 锚点偏移值 | 基准时间（如出生日期），详见 `/phase` 技能与下节 |
| `description` | 字符串 | 描述文案 |
| `avatar` | 字符串 | 头像图片路径，相对 YAML 所在目录（也可绝对路径） |
| `belong_to` | 字符串列表 | 归属的上级实体 id 列表 |
| `calculable_props` | 字符串列表 | 要执行的属性脚本注册 ID 列表 |

### base_time

`base_time` 接受完整的 Phase 表达——**可以是偏移值**（首项为数组锚点 + 逐分量偏移），与事件 `start_time` / `end_time` 共用解析规则（`phase.rs:46-47`）。Phase 的形状、展平与进位规则统一见 `/phase` 技能（`.claude/skills/phase/SKILL.md`），这里只列实体侧要点：

- 标量：`base_time: 1881` 等价于单分量 `[1881]`。
- 列表：按当前历法的分量顺序写整数（从大到小），公历下 `[年, 月, 日, 时, 分, 秒]` 的前缀，如 `base_time: [1980, 7, 31]`。
- 偏移值：`base_time: [[1980, 7, 31, 0], 0, 0, 1, 12]` = 锚点日 +1、时 +12。合法但少见——`base_time` 习惯写绝对时间，偏移形态主要用在事件时间上。
- 分量必须是整数（浮点/字符串报错），任何一层个数不能超过历法分量数（内置公历 6，由历法脚本 `components()` 决定）。
- 省略或 `null` 表示没有基准时间。
- 构建时展平并经历法归一化，产出展示文案 `base_time_humanizer`；原始 `Phase` 保留给偏移运算。

### avatar

路径必须指向已存在的图片文件，按扩展名判断：`bmp gif ico jpeg jpg png svg webp`，否则解析报错。空字符串视为未配置（静默忽略）。未配置或加载失败时前端显示 id 缩写（`Harry Potter` → `HP`，`霍格沃茨` → `霍格`）。输出时解析为绝对路径。

### belong_to 与归属 DAG

```yaml
entity:
  霍格沃茨魔法学校:
    description: 英国的魔法学校。
  格兰芬多:
    belong_to: [霍格沃茨魔法学校]
  阿不思·邓布利多:
    belong_to: [格兰芬多, 霍格沃茨魔法学校]   # 多父合法
```

规则：

- 引用的 id 必须存在，否则报错（批量报出）。
- **不许成环**，否则构建失败、文档无法展示。
- 重复边自动合并；输入顺序无意义——输出侧 `belong_to` 排序去重。
- 归属关系**不留在实体上**：构建期建 DAG 校验后反查直接上级，输出为顶层 `belong_dag` 数组（`BelongNode { id, belong_to? }`，按实体 id 排序，无上级时省略 `belong_to`）。前端只从 `belong_dag` 读归属。

## entity_link：实体关系

归属用 `belong_to`；非层级的两两关系（母子、朋友、敌对……）用 `entity_link`。按组组织，每组是一批关系对：

```yaml
entity_link:
  波特一家:
    - entities: [哈利·波特, 莉莉·波特]
      relations: 母子
    - entities: [哈利·波特, 詹姆·波特]
      relations: 父子
```

- `entities` 恰好是**两个**实体 id（serde 元组，多写少写都反序列化失败）。
- `relations` 是自由文本的关系类型。
- 只校验 ID 存在，数据原样输出，不做语义处理。

## calculable_props：可计算属性

让脚本从实体数据（基准时间、描述、归属）算出一个属性值，结果写进该实体 `extra_props`：

```yaml
story:
  calculatable_props:          # 注意：story 级拼写是 calculatable（带 ta）
    constellation: constellation.rn

entity:
  哈利·波特:
    base_time: [1980, 7, 31]
    calculable_props: [constellation]   # 实体级拼写是 calculable（不带 ta）
```

- 两级拼写**不同**，这是最常见的拼写陷阱：注册在 `story.calculatable_props`（`ID: 脚本路径`，路径相对 YAML），引用在 `entity.calculable_props`（注册 ID 列表）。见 `src-tauri/src/weft/story.rs:19` 与 `entity.rs:30`。
- 脚本必须公开实现 `calculable_prop(ctx)`；返回字符串/数/布尔/数组/map 写入 `extra_props[注册ID]`，返回 `()` 则不写入；与输入字段同名时**以计算结果为准**。
- `ctx` 含 `language`、`date_mode`、`entity.id / base_time（归一化后整数数组）/ description / belong_to`。
- 只有被实体引用的脚本才执行；所有属性必须先注册（包括 `constellation`）。脚本接口详见 `docs/src/rune-calculable-props.md`，示例 `examples/constellation.rn`。

## extra_props：自定义字段

内置键之外的任意字段收进 `extra_props` 原样透传，适合性别、魔杖、别名这类自由数据：

```yaml
entity:
  哈利·波特:
    gender: 男
    wand: 冬青木魔杖，凤凰羽毛杖芯
```

值为空时序列化中省略该字段。

## 校验失败会怎样

实体段按「先引用后结构」检查，断开的引用**批量**报出；任何一步失败都阻止文档展示：

| 症状 | 原因 |
| --- | --- |
| `EntityUnknownParent` | `belong_to` 引用了不存在的实体 id |
| `EntityCycle` | 归属成环 |
| `UnknownLinkEntity` | `entity_link` 两端引用了不存在的 id |
| `InvalidAvatarPath` | `avatar` 不是已存在的图片文件 |
| `TimeIntegerComponent` / `TimeNonemptyList` / `TooManyTimeComponents` | `base_time` 含非整数 / 空列表 / 超出历法分量数 |
| 反序列化失败 | `entity_link.entities` 不是恰好两个 id |

注意：顶层未声明键被静默滤掉，但**实体内部**的未声明键进 `extra_props` 不会丢——拼错内置键（如 `baseTime`）不会报错，只会变成一个多余的自定义字段。

## 完整示例

以下模式均来自 `examples/story.yaml`，可直接照抄：

```yaml
story:
  title: 示例
  calculatable_props:
    constellation: constellation.rn

entity:
  霍格沃茨魔法学校:                     # 无字段的纯节点（归属树根）
    description: 英国的魔法学校。
  格兰芬多:
    description: 以勇气著称的学院。
    belong_to: [霍格沃茨魔法学校]
  阿不思·邓布利多:
    description: 霍格沃茨校长。
    base_time: 1881                     # 标量：只有年
    belong_to: [格兰芬多, 霍格沃茨魔法学校]
  哈利·波特:
    description: 大难不死的男孩。
    base_time: [1980, 7, 31]            # 列表：年月日
    belong_to: [格兰芬多]
    calculable_props: [constellation]
    gender: 男                          # 自定义字段 → extra_props
    wand: 冬青木魔杖，凤凰羽毛杖芯

entity_link:
  波特一家:
    - entities: [哈利·波特, 詹姆·波特]
      relations: 父子
```
