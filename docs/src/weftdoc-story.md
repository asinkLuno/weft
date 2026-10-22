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
| `date_mode` | 否 | 历法名，缺省内置 `gregorian` |

## date_mode：选择历法

`date_mode` 决定文档里所有时间数组的解释方式：实体的 `base_time`、事件的 `start_time` / `end_time` 都按该历法的时间分量表（`components`）解读，可以省略末尾分量；可计算标签也在同一历法下计算（内置星座标签要求 `base_time` 写全 年 月 日）。

取值有两种：

- `gregorian`：内置公历（6 分量：年 月 日 时 分 秒），无需注册；
- 顶层 `calendar` 注册过的历法名，注册方式与脚本接口见 [Rhai 历法脚本](rhai-calendar.md)。

写了未注册的名字会解析报错；省略 `date_mode` 即用公历。
