# WEFT 语言指南

WEFT（`.weft`）是面向叙事创作的结构化故事描述语言。一个文件由 **6 个正交的顶层声明**构成——元信息（`story`）、时间锚点（`time`）、实体（`entity`）、事件（`event`）、事件列表（`events`）与带代数结构的同质关系图（`link`）——由 `weft-core` 校验并编译成 JSON 交给上层应用。

语法是类 HCL 的声明式块语言：顶层声明 + `key = value` 属性 + 关系边语句 + 注释。不需要分号，属性与边之间用换行分隔，可选逗号或分号。写完后用 [`weft check` / `weft fmt`](weft-cli.md) 检查与格式化。

## 最小文档

```weft
story {
  title = "无题"
}

entity 学院 {
  description = "故事发生的舞台。"
}

entity 主角 {
  description = "一个还没有故事的人。"
}

event 开场 {
  start    = [2024, 9, 1]
  entities = [主角]
}

events 主线 = [开场]

link 组织归属 {
  acyclic    = true
  transitive = true

  主角 -> 学院 {
    start = 开场.start
  }
}
```

只有 `story`（及 `story.title`）和每个 `event` 的 `start` 是必填项，其余声明与字段均可按需省略。文档支持**任意顺序声明与前向引用**，所有跨块引用会在编译期统一解析。

## 核心模型：统一命名空间与同质双层图

WEFT 采用**实体状态层**与**剧情拓扑层**分离的双层建模思路：

1. **统一节点命名空间**：`entity` 与 `event` 共享同一张节点符号表，不允许重名（重名报 `duplicate-symbol`）。
2. **实体保持纯粹**：`entity` 只描述实体本身（简介、基准时间、头像、可计算属性及任意自定义扩展字段 `extra_props`），不再内置从属或标签字段；所有实体间结构化关系（归属、空间包含、持有、人际、等价别名等）统一收口到 `link` 图。
3. **跨层出场唯一入口**：实体参与某个事件，唯一通过 `event.entities` 声明，不通过 `link` 跨层连边。
4. **严格同质的 `link` 关系图**：每张 `link` 图只能全连 `entity`（世界观状态演化图，支持 `{ start, end }` 时变区间与 `equivalence` 商图）或全连 `event`（剧情拓扑图，如伏笔与前置阻塞，禁止写 `start`/`end` 与 `equivalence`）。在同一张图里混连实体与事件会直接报 `mixed-link-node-kinds`。

## 词法

- **注释**：`# ...` 与 `// ...` 到行尾；`/* ... */` 跨行。字符串字面量里的 `#`、`//`、`/*` 不算注释。所有注释都会被 `weft fmt` 保留。
- **名字（标识符）**：
  - **裸名字**：Unicode 字母或 `_` 开头，后续可以是 Unicode 字母、数字、`_`、`·`、`・`。中文可以直接裸写（如 `entity 哈利·波特`、`entity 霍格沃茨魔法学校`）。
  - **引号名字**：名字包含空格、`·`/`・` 以外的标点或斜杠、以数字开头，或与关键字 `true` / `false` / `null` 同名时，必须用双引号包住：`entity "9又3/4站台"`。
- **字符串**：
  - 单行 `"..."` 支持转义 `\\`、`\"`、`\n`、`\r`、`\t`。
  - 多行 `"""..."""` 会自动去掉首尾边界换行，并剥离所有非空行的公共前导缩进，适合写多行 `description`。
- **数字与字面量**：
  - 支持带符号整数（`42`、`-44`）与浮点数（`3.14`、`-0.5`）。
  - 布尔值 `true`、`false` 与空值 `null`。可选字符串字段和可选时间字段显式写 `null` 等价于省略。
- **运算符**：
  - 关系边箭头：`->`、`<->`、`--`、`<-`。
  - 时间偏移运算：`+`、`-`。
- **列表与块分隔符**：
  - 列表（`[a, b]`、`events` 列表、`link` 批量端点与团列表）的元素用 `,` 分隔，允许尾逗号。
  - 属性块 / 对象（`{ key = "value" }`）与 `link` 块内的条目用换行或空白分隔，条尾可选 `,` 或 `;`。
- **字符串与裸标识符的区分**：
  - 字符串字段（`title`、`author`、`description`、`date_mode`、`avatar`）与 `link` 边标签**必须加双引号**；裸写会触发类型错误或语法错误。
  - 名字列表（`entities`、`calculable_props`、`events`、`link` 端点与团成员）支持裸名字或引号名字。
  - 在 `entity` 的自定义扩展字段（`extra_props`）里，裸标识符会被按字符串值透传，但不支持 `.` 成员访问或 `+`/`-` 表达式（表达式仅用于时间字段）。

## 顶层声明

| 声明                           | 作用                                                                      |
| ------------------------------ | ------------------------------------------------------------------------- |
| `story { ... }`                | 故事元信息与 `.rn` 脚本注册表（必填且全局唯一）                           |
| `time <name> = <expr>`         | 命名时间锚点，可在任意时间字段里复用                                      |
| `entity <name> { ... }`        | 实体（人物、组织、地点、道具等，与 `event` 共享节点命名空间）             |
| `event <name> { ... }`         | 事件 / 剧情点（必填 `start`，与 `entity` 共享节点命名空间）               |
| `events <name> = [event, ...]` | 有序事件列表（叙事线展示顺序）                                            |
| `link <graph> { ... }`         | 带代数约束的同质关系图（纯 `entity` 时变图或纯 `event` 剧情图，图名唯一） |

### story

全局必填且只能声明一次（缺失报 `missing-story`，重复报 `duplicate-story`）。

```weft
story {
  title            = "哈利·波特与魔法石"
  author           = "J.K. 罗琳"
  description      = """
    十一岁的哈利·波特进入霍格沃茨魔法学校，
    与同伴一起揭开魔法石的秘密。
  """
  date_mode        = "calendars/custom.rn" # 省略则用内置公历
  calculable_props = {
    constellation = "constellation.rn"
  }
}
```

| 字段               | 必填 | 类型                      | 说明                                                   |
| ------------------ | ---- | ------------------------- | ------------------------------------------------------ |
| `title`            | 是   | 字符串                    | 故事标题                                               |
| `author`           | 否   | 字符串                    | 作者                                                   |
| `description`      | 否   | 字符串                    | 故事简介                                               |
| `date_mode`        | 否   | 字符串                    | 自定义历法 `.rn` 脚本路径（相对当前 `.weft` 文件）     |
| `calculable_props` | 否   | 对象 `{ id = "path.rn" }` | 可计算属性注册表：ID → `.rn` 路径（相对 `.weft` 文件） |

出现上表之外的字段会报 `unknown-field`。自定义历法与可计算属性脚本的写法见[自定义历法与可计算属性](rune-plugins.md)。

### time

声明独立的命名时间锚点，可在任意时间字段（`entity.base_time`、`event.start`、`event.end`、`entity` 关系边的 `start`/`end` 或其他 `time` 锚点）中复用。同名重复声明报 `duplicate-time-anchor`。

```weft
time 开学夜 = [1991, 9, 1, 19]

event 分院帽晚宴 {
  start = 开学夜
  end   = 开学夜 + [0, 0, 0, 2]
}
```

### entity

声明故事中的人物、组织、地点、道具等实体。同名重复报 `duplicate-entity`，与 `event` 同名报 `duplicate-symbol`。

```weft
entity 哈利·波特 {
  description      = "大难不死的男孩。"
  base_time        = [1980, 7, 31]
  calculable_props = [constellation]
  avatar           = "images/harry.png"

  # 表外字段原样透传到编译结果的 extra_props
  kind   = "人物"
  gender = "男"
  wand   = "冬青木魔杖，凤凰羽毛杖芯"
}
```

| 字段               | 类型         | 说明                                                                                     |
| ------------------ | ------------ | ---------------------------------------------------------------------------------------- |
| `description`      | 字符串       | 实体简介                                                                                 |
| `base_time`        | 时间表达式   | 出生 / 创立 / 起始基准时间；可供时间引用、可计算属性输入，并自动计算与参与事件的相对时差 |
| `avatar`           | 字符串       | 头像图片路径（相对 `.weft` 所在目录），必须是已存在的 bmp/gif/ico/jpeg/jpg/png/svg/webp  |
| `calculable_props` | 属性 ID 列表 | 启用的可计算属性列表（须先在 `story.calculable_props` 注册），计算结果写入 `extra_props` |

不在上表中的任意键（支持标量、列表、嵌套对象）都会原样收进 `extra_props`（例如常用 `kind = "人物"` 区分实体类别），不会触发 `unknown-field`。

### event

声明时间轴上的事件或剧情节点。同名重复报 `duplicate-event`，与 `entity` 同名报 `duplicate-symbol`。

```weft
event 分院帽晚宴 {
  start       = [1991, 9, 1, 19]
  end         = start + [0, 0, 0, 2]
  entities    = [哈利·波特, 罗恩·韦斯莱, 赫敏·格兰杰, 礼堂]
  description = "开学宴会上分院帽把新生分入四大学院。"
}
```

| 字段          | 必填 | 类型         | 说明                                                                           |
| ------------- | ---- | ------------ | ------------------------------------------------------------------------------ |
| `start`       | 是   | 时间表达式   | 事件开始时间                                                                   |
| `end`         | 否   | 时间表达式   | 事件结束时间；省略表示为瞬时点事件。若早于 `start` 报 `event-end-before-start` |
| `entities`    | 否   | 实体名字列表 | 参与本事件的实体列表（须为已声明 `entity`，否则报 `unknown-event-entity`）     |
| `description` | 否   | 字符串       | 事件说明                                                                       |

`event` 块不允许写表外字段（报 `unknown-field`）。对于 `entities` 中声明了 `base_time` 的实体，编译器会自动按当前历法计算事件起止时刻相对该实体 `base_time` 的差值（如角色当时的年龄或组织成立年限）。

### events

声明一条有序事件列表（叙事线 / 章节序列）。同名重复报 `duplicate-events`，包含未声明事件报 `unknown-events-event`。

```weft
events 哈利的主线 = [分院帽晚宴, 圣诞节礼物, 厄里斯魔镜, 冥想盆真相]
```

列表顺序决定叙事展示顺序，不影响事件本身的时间坐标。

### link

**一张 `link` 块 = 一张有唯一名字、明确代数结构的同质关系图**（图名重复报 `duplicate-link-graph`）。图级的数学结构约束与单条边上的语义标签完全正交：约束写在块内属性里，具体语义写在箭头标签里。

#### 图级代数属性

块内所有图级属性均可选，布尔属性默认 `false`，出现表外字段报 `unknown-field`：

| 字段                 | 类型   | 语义与校验规则                                                                                               |
| -------------------- | ------ | ------------------------------------------------------------------------------------------------------------ |
| `acyclic`            | 布尔值 | 无环（DAG）；强制有向边，隐含反自反与非对称；任意原子时间切片内出现自环或有向环报 `graph-cycle`              |
| `asymmetric`         | 布尔值 | 非对称；强制有向边，禁止同切片内双向互指（含自环），允许长度 ≥ 3 的环；违规报 `asymmetry-conflict`           |
| `intransitive`       | 布尔值 | 反传递；禁止跳级直连（同切片内已有 `A -> B` 和 `B -> C` 时再连 `A -> C`）；违规报 `intransitivity-violation` |
| `functional`         | 布尔值 | 多对一（函数关系）；同切片内每个源节点出度 ≤ 1（指向的不同目标 ≤ 1）；违规报 `functional-conflict`           |
| `inverse_functional` | 布尔值 | 一对多（逆函数关系）；同切片内每个目标节点入度 ≤ 1（来自的不同源 ≤ 1）；违规报 `inverse-functional-conflict` |
| `symmetric`          | 布尔值 | 对称图；强制整张图使用无向边或团（`<->` / `<- "..." ->`），查重忽略方向                                      |
| `transitive`         | 布尔值 | 声明可传递，供前端视图与查询层计算传递闭包                                                                   |
| `equivalence`        | 布尔值 | 等价图（商图坍缩）；仅限 `entity` 图，强制 `<->` / 团，禁止边带 `start`/`end`                                |
| `chronological`      | 布尔值 | 时序单调；强制有向边，校验 `from` 节点时间 `<=` `to` 节点时间，违规报 `chronological-violation`              |
| `description`        | 字符串 | 关系图说明，供 LSP Hover 与界面展示                                                                          |

**互斥属性组合**：以下组合在代数上矛盾，同时设为 `true` 会报 `invalid-graph-config`：

- 有向约束（`acyclic`、`asymmetric`、`chronological`）与对称约束（`symmetric`、`equivalence`）互斥；
- `transitive` 与 `intransitive` 互斥；
- `equivalence` 与 `intransitive`、`functional`、`inverse_functional` 互斥。

#### 边与团语法

边支持单端点与非空列表批量端点，双引号内为可选语义标签（标签必须加双引号）：

| 结构           | 无标签          | 带标签                   | 说明                                                             |
| -------------- | --------------- | ------------------------ | ---------------------------------------------------------------- |
| 有向边         | `A -> B`        | `A -- "怀疑" -> B`       | 从 `A` 指向 `B`                                                  |
| 无向对称边     | `A <-> B`       | `A <- "夫妻" -> B`       | `A` 与 `B` 对称互连                                              |
| 批量端点       | `[A, B] -> C`   | `[A, B] -- "属于" -> C`  | 左右端均可写列表，展开为笛卡尔积；空列表报 `empty-link-endpoint` |
| 团（完全子图） | `<-> [A, B, C]` | `<- "好友" -> [A, B, C]` | 至少 2 个节点（否则报 `invalid-clique`），展开为所有两两无向对   |

**箭头方向校验（`arrow-direction-mismatch`）**：

- **自由图**（未启用任何方向/对称约束）：允许有向边、无向边与团自由混写；
- **有向约束图**（启用 `acyclic` / `asymmetric` / `chronological` 之一）：只允许有向边（`->` / `-- "..." ->`）；
- **对称约束图**（启用 `symmetric` / `equivalence` 之一）：只允许无向边与团（`<->` / `<- "..." ->`）。

#### 边尾属性块与时变区间

每条边或团语句末尾都可以跟一个可选属性块 `{ ... }`，支持 `start`、`end`、`description` 三个字段（表外字段报 `unknown-field`）：

- **纯 `entity` 非等价图**：允许写 `start`、`end`、`description`。
  - 区间语义为左闭右开 `[start, end)`；当 `start == end` 时视作瞬时点 `[t, t]`。
  - `start` 省略表示 `-∞`，`end` 省略表示 `+∞`；若 `end < start` 报 `link-end-before-start`。
  - 块内支持全部时间表达式，且可直接用 `start` / `end` 自引用本条边的另一端点。
  - 批量边与团的尾部属性块会共享应用到展开后的每一条边。
- **纯 `event` 图**：事件本身已有时间坐标，禁止在边上写 `start` / `end`（报 `event-link-time-bounds`），仅允许写 `description`；且不允许启用 `equivalence`（报 `event-graph-equivalence`）。
- **`equivalence` 图**：等价身份合并在整个故事中全局生效，禁止写 `start` / `end`（报 `equivalence-link-time-bounds`），仅允许写 `description`。

#### 原子时间切片与时序校验

- **原子时间切片校验**：对于启用了 `acyclic`、`asymmetric`、`intransitive`、`functional`、`inverse_functional` 的图，编译器会收集该图所有边的 `start` / `end` 刻度，把时间轴切分为互不重叠的原子时间切片（及瞬时点），在每个切片内独立构建活跃子图进行校验。
  - 因此，唯一物品在不同时段先后归属不同持有者（如 `end = [1981, 10, 31]` 与 `start = [1991, 12, 25]`）在 `inverse_functional = true` 下完全合法；只有当两条边的生效区间发生重叠时才会报错。
  - 在 `weft check --syntax-only` 模式下（不执行 `.rn` 历法脚本），编译器仍会对所有不带时间边界的静态边执行图约束校验与查重。
- **`chronological` 时序校验**：
  - 在 `event` 图中，校验每条有向边 `A -> B` 满足 `A.start <= B.start`（防止伏笔回收早于埋设、或前置条件晚于后续剧情）。
  - 在 `entity` 图中，当两端实体都声明了 `base_time` 时，校验 `A.base_time <= B.base_time`。

#### 常见建模模式

| 建模场景          | 节点域   | 推荐属性组合                             | 效果                                             |
| ----------------- | -------- | ---------------------------------------- | ------------------------------------------------ |
| 组织 / 阵营隶属   | `entity` | `acyclic = true`, `transitive = true`    | 时变 DAG，防循环隶属，支持传递闭包查询           |
| 物理空间 / 单亲树 | `entity` | `acyclic = true`, `functional = true`    | 同一时刻每个节点最多归属一个父容器，严格成树     |
| 唯一物品持有      | `entity` | `inverse_functional = true`              | 同一时刻一件道具只能由一名角色持有，换手不冲突   |
| 严格直属层级      | `entity` | `acyclic = true`, `intransitive = true`  | 禁止越级直连（有 `A -> B -> C` 则禁止 `A -> C`） |
| 身份别名 / 化名   | `entity` | `equivalence = true`                     | 声明同一实体的多重身份，支持商图节点合并         |
| 纯对称关系        | `entity` | `symmetric = true`                       | 强制无向边或团，查重忽略方向                     |
| 人际关系网        | `entity` | *无约束（自由图）*                       | 混写有向情绪、无向关系、好友团，用边标签区分语义 |
| 剧情阻塞 / 伏笔   | `event`  | `acyclic = true`, `chronological = true` | 防循环依赖且确保前置事件 / 伏笔不晚于后续剧情    |

#### 完整 `link` 示例

```weft
link 组织归属 {
  acyclic     = true
  transitive  = true
  description = "组织或阵营隶属关系（时变 DAG）"

  [格兰芬多, 斯莱特林, 阿不思·邓布利多] -> 霍格沃茨魔法学校
  [哈利·波特, 罗恩·韦斯莱, 赫敏·格兰杰] -> 格兰芬多 {
    start = 分院帽晚宴.start
  }
}

link 物品持有 {
  inverse_functional = true
  description        = "唯一物品持有关系（同一时刻仅能由一人持有）"

  詹姆·波特 -> 隐形衣 {
    end = [1981, 10, 31]
  }
  哈利·波特 -> 隐形衣 {
    start = 圣诞节礼物.start
  }
}

link 人物关系 {
  莉莉·波特 <- "夫妻" -> 詹姆·波特
  <- "好友" -> [哈利·波特, 罗恩·韦斯莱, 赫敏·格兰杰]
  哈利·波特 -- "怀疑" -> 西弗勒斯·斯内普 {
    start       = 分院帽晚宴.start
    end         = 冥想盆真相.start
    description = "一直以为斯内普在帮伏地魔偷魔法石。"
  }
}

link 身份等价 {
  equivalence = true

  伏地魔 <-> 汤姆·里德尔
}

link 剧情阻塞 {
  acyclic       = true
  chronological = true

  圣诞节礼物 -- "获得隐形衣" -> 厄里斯魔镜
}
```

## 时间表达式

以下位置接受时间表达式：`time` 锚点、`entity.base_time`、`event.start`、`event.end`，以及非等价 `entity` 关系边的 `start` / `end`。

### 绝对时间与模糊精度

- **整数列表**：按历法从高位到低位书写，例如内置公历为 `[年, 月, 日, 时, 分, 秒]`，最多不超过历法声明的分量数（超长报 `too-many-time-components`）。
- **单整数简写**：单个整数 `1881` 等价于 `[1881]`。
- **模糊精度**：不需要写满全部分量。未写的尾部分量在计算刻度时自动用历法 `minimums` 补齐（公历月、日补 `1`，时、分、秒补 `0`），同时保留作者输入的分量长度 `precision`，展示时只渲染到对应精度（例如 `[1980, 7]` 展示为 `1980年7月`）。
- **下界规则**：首项（最高位，如公历年份）没有下界，可以写负数（如 `[-44, 3, 15]`）；其余分量的绝对值不能低于历法规定的最小值（例如公历月/日写 `0` 会报 `time-component-below-minimum`）。

### 时间引用与自引用

- **命名锚点**：直接写 `time` 声明的名字，如 `开学夜`。
- **跨块成员引用**：
  - `事件名.start`：事件的开始时间；
  - `事件名.end`：事件的结束时间（若该事件未显式写 `end`，自动退化为其 `start`）；
  - `实体名.base_time`：实体的基准时间。
  - 基名含特殊字符时给基名加引号：`"穿越9又3/4站台".start`。
- **块内自引用**：在 `event` 块内和 `link` 边属性块内，可直接写裸 `start` 或 `end` 引用同一块的另一端点：

```weft
event 分院帽晚宴 {
  start = [1991, 9, 1, 19]
  end   = start + [0, 0, 0, 2]
}
```

### 相对偏移运算

- 使用左结合的 `+` / `-` 运算符，右侧必须是整数或非空整数列表，可连续叠加：

```weft
time 彩排 = 分院帽晚宴.start - [0, 0, 1] + [0, 0, 0, 3]
```

- 偏移会先按分量加减，再交由历法脚本的 `to_tick` / `from_tick` 归一化进位（如加 8 小时跨午夜会自动进位到次日），最终展示精度取基准与偏移中更精细的一方。
- 引用必须存在（否则报 `unknown-time-ref`），禁止直接或间接循环引用（报 `time-cycle`），嵌套求解深度上限为 64（超限报 `time-nesting-too-deep`）。

## 校验与 lint

`weft check` 执行三层校验：

1. **语法层**：词法与语法解析，按文件与块级恢复，尽量一次报全。
2. **语义与图论层**：必填字段、未知字段、重复声明（含实体与事件重名）、图属性互斥、同质分层、事件/等价图时间限制、箭头方向、引用存在性、时间环与起止先后、原子时间切片图校验（无环 / 非对称 / 反传递 / 出度 / 入度）、时序单调性、头像与 `.rn` 脚本加载执行等。
3. **Lint 层**：默认作为警告输出，可通过 `--ignore <rule>` 按名屏蔽，或通过 `--deny-warnings` 视为错误：

| 规则                 | 触发条件                                                                 |
| -------------------- | ------------------------------------------------------------------------ |
| `duplicate-attr`     | 同一块内重复写同一个键（后者覆盖前者）                                   |
| `unused-time-anchor` | `time` 锚点已声明但从未被任何时间表达式引用                              |
| `unused-entity`      | 文档存在事件或 `link` 边时，实体未被任何事件或关系边引用                 |
| `unreferenced-event` | 文档声明了 `events` 列表时，事件未出现在任何事件列表中                   |
| `self-link`          | 关系边两端是同一节点（启用了 `acyclic` / `asymmetric` 已报错误时不重报） |
| `duplicate-link`     | 同图、同起终点、同方向、同标签且生效时间区间有交集（无向边忽略方向）     |
| `empty-block`        | 不含任何边的 `link` 块，或空的 `events` 列表                             |

运行 `weft rules` 可随时打印当前全部 Lint 规则名。

## 格式化约定

`weft fmt` 是严格幂等、不改变语义的源码格式化器：

- **缩进与空行**：2 空格缩进；顶层声明之间空一行，属性组与边之间最多保留一个空行。
- **等号对齐**：同一连续属性组的 `=` 按 Unicode 显示宽度对齐；空行或独立注释行会切分新的对齐组，行尾注释不打断对齐。
- **`link` 块排版**：图级属性统一排在块顶部并对齐 `=`，空一行后再排边与团语句；箭头运算符（`->`、`<->`、`-- "标签" ->`、`<- "标签" ->`）两侧保留单空格，边标签统一使用双引号；尾部 `{ ... }` 块遵循标准属性块对齐规则。
- **列表折行**：
  - 名字列表（`events`、团成员、`link` 批量端点）：无内部注释、不超过 4 项且行宽不超过 88 列时写单行，否则多行展开并补尾逗号。
  - 普通表达式列表（如 `entities`、时间列表等）：无内部注释且行宽不超过 88 列时写单行，否则多行展开并补尾逗号。
- **字符串与名字**：满足无损回读条件的多行字符串写成 `"""..."""`，其余写单行引号并统一转义；名字仅在无法裸写时加双引号。
- **注释与容错**：原样保留所有注释（含行尾注释与块注释）；文件存在语法错误时拒绝格式化并输出诊断，不破坏原文。

## 常见错误

### 语法与声明

| 规则名                                                                                                         | 原因 / 处理                                                                    |
| -------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| `syntax-error`                                                                                                 | 少 `}` / `]`、缺 `=`、字符串未闭合、边标签未加引号等，按报错位置修复           |
| `missing-story` / `duplicate-story`                                                                            | 缺少或重复声明 `story` 块                                                      |
| `duplicate-entity` / `duplicate-event` / `duplicate-events` / `duplicate-link-graph` / `duplicate-time-anchor` | 同类顶层声明重名                                                               |
| `duplicate-symbol`                                                                                             | `entity` 与 `event` 重名（两者共享节点命名空间）                               |
| `missing-field`                                                                                                | 缺少必填字段，如 `story.title`、`event.start`                                  |
| `unknown-field`                                                                                                | `story` / `event` / `link` 图或边块写了表外字段（`entity` 会进 `extra_props`） |
| `invalid-field`                                                                                                | 字段值类型不符，如字符串字段漏写引号、布尔属性写成字符串                       |

### 关系图配置与图论校验

| 规则名                                                    | 原因 / 处理                                                           |
| --------------------------------------------------------- | --------------------------------------------------------------------- |
| `invalid-graph-config`                                    | `link` 图同时启用了互斥代数属性（如 `acyclic` 与 `symmetric`）        |
| `mixed-link-node-kinds`                                   | 同一 `link` 图混连了 `entity` 与 `event` 节点；需拆分为两张同质图     |
| `event-graph-equivalence`                                 | `event` 图启用了 `equivalence`（商图等价仅适用于 `entity` 图）        |
| `event-link-time-bounds` / `equivalence-link-time-bounds` | `event` 图或 `equivalence` 图的边写了 `start` / `end`                 |
| `invalid-clique` / `empty-link-endpoint`                  | 团成员不足 2 个，或批量端点列表为空 `[]`                              |
| `arrow-direction-mismatch`                                | 边箭头方向与图约束冲突（有向约束图需 `->`，对称/等价图需 `<->` 或团） |
| `graph-cycle`                                             | `acyclic` 图在某原子时间切片内出现自环或有向环                        |
| `asymmetry-conflict`                                      | `asymmetric` 图在某时间切片内出现双向互指或自环                       |
| `intransitivity-violation`                                | `intransitive` 图存在跳级直连 `A -> C`（已有 `A -> B -> C`）          |
| `functional-conflict` / `inverse-functional-conflict`     | `functional` 源节点出度 > 1，或 `inverse_functional` 目标节点入度 > 1 |
| `chronological-violation`                                 | `chronological` 图出现时间倒流（前置节点时间晚于后续节点）            |

### 时间、引用与外部脚本

| 规则名                                                                           | 原因 / 处理                                                                  |
| -------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| `unknown-time-ref` / `time-cycle` / `time-nesting-too-deep`                      | 时间引用不存在、形成循环，或嵌套超过 64 层                                   |
| `event-end-before-start` / `link-end-before-start`                               | 事件或关系边的 `end` 早于 `start`                                            |
| `too-many-time-components` / `time-component-below-minimum`                      | 时间分量超过历法上限，或非首项分量低于历法 `minimums`                        |
| `unknown-link-entity` / `unknown-event-entity` / `unknown-events-event`          | 引用了未声明的实体或事件（诊断通常会附 `did you mean` 拼写建议）             |
| `invalid-avatar-path`                                                            | 头像路径不存在或不是受支持的图片格式                                         |
| `invalid-calendar-path` / `calendar-*`                                           | 历法 `.rn` 脚本路径无效、编译失败、缺接口或 `to_tick`/`from_tick` 往返不一致 |
| `invalid-calculable-prop-path` / `unknown-calculable-prop` / `calculable-prop-*` | 可计算属性脚本路径无效、实体引用了未注册 ID，或脚本执行/返回异常             |

下一步：[自定义历法与可计算属性](rune-plugins.md)，或直接使用 [`weft check` / `weft fmt`](weft-cli.md)。
