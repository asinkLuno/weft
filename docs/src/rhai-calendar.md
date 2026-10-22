# Rhai 历法脚本

## 注册与选用

在故事 YAML 顶层注册脚本，再用 `story.date_mode` 选用。`source` 相对于故事 YAML 所在目录；`runtime`、`kind`、`api` 必须分别是 `rhai`、`calendar`、`1`。

```yaml
story:
  title: 示例
  date_mode: my_calendar

calendar:
  my_calendar:
    runtime: rhai
    kind: calendar
    api: 1
    source: my_calendar.rhai
```

不指定 `date_mode` 时使用内置 `gregorian`，无需注册。内置公历脚本在 `src-tauri/rhai/gregorian.rhai`，其显示语言跟随应用设置（`zh` 或 `en`），无需使用带语言后缀的历法名。

## 必须实现的公开函数

加载时会检查以下函数名和参数个数；函数必须是公开的。Rhai 的声明没有返回类型，因此返回值仍在实际调用时校验。

| 函数 | 输入 | 返回值 | 用途 |
| --- | --- | --- | --- |
| `metadata()` | 无 | map | 历法的语言无关元数据，至少含 `components` |
| `normalize(values)` | 整数数组 | 等长的整数数组 | 处理进位等归一化规则 |
| `to_tick(values)` | 整数数组 | 64 位整数 | 把时间转成可比较、可相减的刻度 |
| `humanizer(kind, value, language)` | 种类、数据、语言 | 见下表 | 生成显示内容 |

`metadata().components` 经 `humanizer("metadata", ...)` 处理后必须是 **1 到 64** 的正整数。它定义时间数组的分量数与顺序。用户写的数组可以省略末尾分量，后端会补 `0`；超过该长度会报错。`normalize` 必须返回恰好 `components` 个整数。`to_tick` 的数值用于事件排序、起止先后判断和叙事线相对刻度，须保持同一历法内的时间顺序，并处于 64 位整数范围内。

`humanizer` 在一个函数中处理所有语言分支：

| `kind` | `value` | 返回值 | 调用时机 |
| --- | --- | --- | --- |
| `metadata` | `metadata()` 的 map | map，保留 `components`，可添加 `title`、`description` | 加载历法时 |
| `time` | 按 `components` 补齐的整数数组 | 字符串 | 展示实体和事件时间、相对偏移 |

`language` 是应用设置中的 `zh` 或 `en`。应用切换语言后会重新解析当前文档，因此 `humanizer` 会收到新的语言。脚本也可调用宿主提供的 `language()`，但建议只在 `humanizer` 中选择显示文字，让 `metadata`、`normalize` 和 `to_tick` 的结果不受语言影响。

内置公历还使用 `humanizer("duration", seconds, language)`：`seconds` 是两个 tick 的差，返回时长字符串。**目前只有内置 `gregorian` 会收到 `duration`**；自定义历法的相对偏移是逐分量相减、归一化后按 `time` 展示。内置脚本中的 `extra(values)` 和 `humanizer("extra", ...)` 不是当前宿主要求的接口。

## 最小脚本示例

下面的三分量历法把每年视作 10 个月、每月 30 天。示例仅演示接口形状；`to_tick` 的单位为天。

```rhai
fn metadata() {
    #{ components: 3 }
}

fn normalize(values) {
    let total = values[0] * 300 + (values[1] - 1) * 30 + values[2] - 1;
    let year = total / 300;
    let remainder = total % 300;
    if remainder < 0 {
        year -= 1;
        remainder += 300;
    }
    [year, remainder / 30 + 1, remainder % 30 + 1]
}

fn to_tick(values) {
    values[0] * 300 + (values[1] - 1) * 30 + values[2] - 1
}

fn humanizer(kind, value, language) {
    if kind == "metadata" {
        let result = value;
        result.title = if language == "zh" { "十月历" } else { "Ten Month Calendar" };
        return result;
    }
    if kind == "time" {
        if language == "zh" {
            return `${value[0]}年${value[1]}月${value[2]}日`;
        }
        return `Year ${value[0]}, month ${value[1]}, day ${value[2]}`;
    }
    throw `unknown display kind: ${kind}`;
}
```

脚本读取、编译或接口检查失败时，文档解析会报错。`metadata` 会在加载时调用；其余函数在解析使用该历法的时间时调用。脚本运行受操作数、调用层数及容器大小等资源限制。
