---
name: weft-story
description: Weft 故事文档的 story 段写法：元信息（title/author/description）与日历、插件配置——date_mode 选择内置公历或自定义 Rune 历法脚本（components 与 from_tick/to_tick/serialize/humanizer 五函数接口），calculatable_props 注册可计算属性脚本。当用户要写 story 段、选历法、写自定义日历 .rn 脚本、注册星座这类可计算属性，或排查历法/插件加载报错时使用。
---

# Weft story 段：元信息、日历与插件

`story` 是唯一必填的顶层键，除元信息外还负责两件事：**选历法**（决定全文档所有时间数组怎么解释）和**注册插件**（可计算属性脚本）。

```yaml
story:
  title: 哈利·波特与魔法石
  author: J.K. 罗琳
  description: >-
    十一岁的哈利·波特进入霍格沃茨魔法学校，
    与同伴一起揭开魔法石的秘密。
  date_mode: my_calendar.rn          # 省略 = 内置公历
  calculatable_props:
    constellation: constellation.rn
```

| 字段 | 必填 | 说明 |
| --- | --- | --- |
| `title` | 是 | 故事标题 |
| `author` / `description` | 否 | 作者、简介 |
| `date_mode` | 否 | 历法脚本路径；缺省、空值或空字符串 = 内置公历 |
| `calculatable_props` | 否 | 可计算属性注册表，`ID: .rn 路径` |

## date_mode：选历法

先问：故事用公历（「某年某月某日」能对上）吗？是就**不写** `date_mode`。内置公历 6 分量：年 月 日 时 分 秒。

架空世界观、非公历纪年（干支、自创历法）才需要自定义：填相对 YAML 所在目录的 `.rn` Rune 脚本路径，无需注册名：

```yaml
story:
  title: 月之都编年
  date_mode: calendars/lunar.rn
```

历法决定全文档时间语义：实体 `base_time`、事件 `start_time` / `end_time`、可计算属性都按该历法的分量表解释，分量数也随之变化（公历 6 → 自定义 N）。中途换历法等于所有时间重写——第一遍写 story 时就要定好。

### 自定义历法脚本接口

Rune 中未被引用的私有函数不会进入编译产物，所以接口函数**必须声明 `pub`**。加载时检查以下五个 `pub` 函数是否存在（参数个数在运行期确定）：

| 函数 | 输入 → 返回 | 职责 |
| --- | --- | --- |
| `components()` | 无 → 整数（1–64） | 时间分量数，决定时间数组长度 |
| `serialize(language)` | 语言码 → 对象 | Story 页展示的本地化 `name` / `description` |
| `from_tick(tick)` | 刻度 → 长度恰为 N 的日期数组 | 刻度 → 合法日期 |
| `to_tick(values)` | 整数数组 → i64 | 日期 → 可比较刻度；须接受未归一化输入、保持时间顺序 |
| `humanizer(values, language)` | 归一化数组、语言 → 字符串 | 仅格式化展示，不做历算 |

最小示例（每年 10 个月、每月 30 天）：

```rune
pub fn components() { 3 }

pub fn from_tick(total) {
    let year = total / 300;
    let remainder = total % 300;
    if remainder < 0 {
        year -= 1;
        remainder += 300;
    }
    [year, remainder / 30 + 1, remainder % 30 + 1]
}

pub fn to_tick(values) {
    values[0] * 300 + (values[1] - 1) * 30 + values[2] - 1
}

pub fn serialize(language) {
    #{
        name: if language == "zh" { "十月历" } else { "Ten Month Calendar" },
        description: if language == "zh" { "每年十个月，每月三十天。" } else { "Ten months per year." },
    }
}

pub fn humanizer(value, language) {
    if language == "zh" {
        let stamp = `${value[0]}年${value[1]}月${value[2]}日`;
        return stamp;
    }
    `Year ${value[0]}, month ${value[1]}, day ${value[2]}`
}
```

`normalize` 由宿主实现（`from_tick(to_tick(v))`），脚本不用写。`language` 只会是 `zh` 或 `en`。Rune 语法要点：变量默认可变（不写 `mut`）、整数运算沿用 Rust 语义（溢出和除零报错，`/` 向零截断，`%` 符号跟随被除数）、模板字符串是宏调用不能直接跟在 `return` 后（先赋值再返回）、脚本主动报错用 `panic(消息)`。完整规范：`docs/src/rune-calendar.md`；干支历示例：`examples/lunar.rn`。脚本读取、编译或接口检查失败都会让文档解析报错。

## calculatable_props：可计算属性注册

让脚本从实体数据（基准时间、描述、归属）算出一个属性值，写进实体 `extra_props`。两级配合，**拼写不同**：

```yaml
story:
  calculatable_props:                      # story 级：calculatable（带 ta），ID → 脚本路径
    constellation: constellation.rn

entity:
  哈利·波特:
    base_time: [1980, 7, 31]
    calculable_props: [constellation]      # 实体级：calculable（不带 ta），引用注册 ID
```

- 所有属性（包括现成的 `constellation`）都必须先注册才能引用；路径相对 YAML 所在目录。
- 脚本须公开 `pub fn calculable_prop(ctx)`：返回字符串/数/布尔/数组/对象写进 `extra_props[注册ID]`，返回 `()` 不写入；与实体输入字段同名时以计算结果为准；本地化报错用 `panic(消息)`。
- `ctx` 含 `language`（zh/en）、`date_mode`、`entity.id / base_time（按当前历法归一化后的整数数组）/ description / belong_to`。
- 只有被实体 `calculable_props` 引用的脚本才执行；切换应用语言会重新计算。

接口细节：`docs/src/rune-calculable-props.md`；星座示例：`examples/constellation.rn`（注意它要求公历历法、且 `base_time` 写全 年 月 日，否则 panic）。

## 常见报错

| 症状 | 原因 |
| --- | --- |
| story 段路径检查失败 | `date_mode` / 脚本路径不指向已存在的 `.rn` 文件 |
| 脚本接口检查失败 | 缺五个 `pub` 历法函数之一、缺 `pub fn calculable_prop(ctx)`，或函数未声明 `pub` |
| 编译诊断 | Rune 脚本语法错误，带源码行列定位 |
| 属性计算 panic | 脚本主动报错，如星座脚本遇到自定义历法或 base_time 不全 |
