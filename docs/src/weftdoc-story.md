# story

`story` 承载故事元信息，是唯一必填的顶层键。

```yaml
story:
  title: 哈利·波特与魔法石
  author: J.K. 罗琳
  description: >-
    十一岁的哈利·波特进入霍格沃茨魔法学校，
    与同伴一起揭开魔法石的秘密。
```

| 字段 | 必填 | 说明 |
| --- | --- | --- |
| `title` | 是 | 故事标题 |
| `author` | 否 | 作者名 |
| `description` | 否 | 故事简介 |
| `date_mode` | 否 | Rune 脚本路径，缺省或留空使用内置公历 |
| `calculatable_props` | 否 | 可计算属性注册表（ID → .rn 路径），见 [Rune calculable props 脚本](rune-calculable-props.md) |

## date_mode：选择历法

`date_mode` 决定文档里所有时间数组的解释方式：实体的 `base_time`、事件的 `start_time` / `end_time` 都按该历法的时间分量表（`components`）解读，可以省略末尾分量，也接受单个整数（等价于只写一个分量的列表）；可计算属性也在同一历法下计算（星座脚本要求 `base_time` 写全 年 月 日）。

省略、空值或空字符串表示内置公历（6 分量：年 月 日 时 分 秒）。自定义历法直接填写相对故事文件的 Rune 脚本路径，例如 `date_mode: calendars/custom.rn`，无需注册 id。脚本接口见 [Rune 历法脚本](rune-calendar.md)。
