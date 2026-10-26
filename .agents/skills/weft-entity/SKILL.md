---
name: weft-entity
description: 从文章中提炼 Weft 实体并写 entity 段：什么算实体（人物、团体、地点、道具）、id 命名与引用、base_time / description / avatar / belong_to / calculable_props 五个内置键、extra_props 自定义字段。当用户在把小说或设定整理成 weft YAML、拿不准某个名词该不该建实体、或要写改 entity 段与排查实体解析报错时使用。归属层级与两两关系的设计见 weft-relation 技能。
---

# Weft entity 段：从名词到实体

实体是故事里的**名词**：人物、团体、地点、道具——一切会被事件「参与」、会互相归属或建立关系的东西。`entity` 是一张表：键即实体 id（可中文），值是字段；id 被 `belong_to`、`entity_link`、事件的 `entities` 引用，全文档统一。

## 第一遍怎么圈实体

读到名词时问三个问题，任一答「是」就值得建实体：

1. **它会被事件引用吗？**——这个人会参与某件事，这个地点会发生什么。
2. **它会归属某个组织，或与其他实体有关系吗？**
3. **它值得一张独立卡片吗？**——读者会想单独查看它。

| 类别 | 建 | 不建（写进 description / extra_props） |
| --- | --- | --- |
| 人物 | 有名字、参与情节或关系的人 | 龙套、「一个卖魔杖的老人」式一次性提及 |
| 团体 | 学院、家族、势力、团队（常作归属根） | 泛称（「人们」「学生们」） |
| 地点 | 被多个事件引用或本身是地标的场所（礼堂、禁林） | 只出现一场的过场房间（写进事件描述） |
| 道具 | 推动情节的关键物品（魔法石） | 日常物件 |
| 动物 | 有名字、参与事件或与人物结成固定关系的动物（信使猫头鹰、三头犬、龙） | 无名的动物背景 |

三问之外的两条裁决线，专治「似建非建」：

- **具名龙套**：有名字但只在一场戏做事、后文不再被引用的角色（酒吧老板、一次出场的球员），用硬线裁决——**有名字 × 实际做了至少一件事 × 后文还会被引用**，缺一就不建，只作事件的参与者字符串或写进别人 description。
- **未命名但推动情节者**：能用类别名稳定指称的就建（一头巨怪 → `巨怪`）；无法指称的匿名者（匿名寄衣人）不建，写进事件描述。

类别只是整理用的标签，weft 不存储、不校验类别；需要展示时自己加自定义字段（如 `kind: 人物`），会收进 `extra_props` 透传给前端。

id 用原文最常用称呼，全文档统一；别名记进自定义字段（如 `real_name: 汤姆·里德尔`）。

## 五个内置键

| 键 | 类型 | 说明 |
| --- | --- | --- |
| `base_time` | 整数 / 整数列表 | 基准时间（生日、成立日），见下节 |
| `description` | 字符串 | 一句话描述 |
| `avatar` | 字符串 | 头像路径，相对 YAML 所在目录（也可绝对路径）；须是已存在的 bmp/gif/ico/jpeg/jpg/png/svg/webp 文件，否则解析报错；未配置时显示 id 缩写 |
| `belong_to` | 字符串列表 | 归属上级 id，怎么设计见 weft-relation 技能 |
| `calculable_props` | 字符串列表 | 引用 `story.calculatable_props` 注册的 ID（此处不带 ta） |

**其余任何键原样收进 `extra_props` 透传给前端**，适合性别、魔杖、别名这类自由数据：

```yaml
entity:
  哈利·波特:
    description: 大难不死的男孩。
    base_time: [1980, 7, 31]
    belong_to: [格兰芬多]
    calculable_props: [constellation]
    gender: 男                       # → extra_props
    wand: 冬青木魔杖，凤凰羽毛杖芯    # → extra_props
```

所有字段都可省——纯节点（如只当归属根的组织）只有一个 id 也合法。拼错内置键（如 `baseTime`）不报错、静默变成自定义字段，交付前核对拼写。

## base_time 写法

- 标量 = 只写第一分量（年）：`base_time: 1881`。
- 列表 = 按历法从大到小、尾部可省：公历为 `[年, 月, 日, 时, 分, 秒]`，`base_time: [1980, 7, 31]` 即 1980-7-31 00:00:00。
- 分量必须是整数（浮点、字符串都报错）；任何一层个数不得超过历法分量数（公历 6）。
- 习惯写绝对时间；锚点偏移形态（`[[1980, 7, 31], 0, 0, 1]`）合法但少见。递归锚点、不进位借位等完整规则见 `docs/src/design/phase.md`。
- 文本只给相对表述（「去年庆祝了六百六十五岁生日」）而无绝对年份时，**省略** `base_time`，把线索写进 description——不要推算一个不可靠的年份凑数。

## 常见报错

| 症状 | 原因 |
| --- | --- |
| `EntityUnknownParent` | `belong_to` 引用不存在的实体 id |
| `EntityCycle` | 归属成环 |
| `InvalidAvatarPath` | avatar 路径不存在或非图片扩展名 |
| `TimeIntegerComponent` / `TimeNonemptyList` / `TooManyTimeComponents` | base_time 含非整数 / 空列表或字符串 / 超出历法分量数 |

权威参考：`docs/src/weftdoc.md`、`docs/src/design/entity.md`、`examples/story.yaml` 的 entity 段。
