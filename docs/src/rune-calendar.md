# Rune 历法脚本

## 注册与选用

在 `story.date_mode` 中直接填写 Rune 脚本路径，路径相对于故事 YAML 所在目录，无需顶层注册表。

```yaml
story:
  title: 示例
  date_mode: my_calendar.rn
```

省略 `date_mode`、填写空值或空字符串时使用内置公历。内置脚本在 `src-tauri/rune/gregorian.rn`，以应用资源文件打包并统一按路径加载。story 阶段检查路径必须指向现有的 `.rn` 文件；脚本读取、编译与接口校验在 Calendar 加载时执行。显示语言跟随应用设置（`zh` 或 `en`）。

## 必须实现的公开函数

Rune 中未被引用的私有函数不会进入编译产物，因此接口函数必须声明 `pub`。加载时检查 `meta`、`from_tick`、`to_tick` 和 `humanizer` 是否以 `pub` 函数定义；参数个数在运行期确定，不匹配会在调用时报错。

| 函数 | 输入 | 返回值 | 用途 |
| --- | --- | --- | --- |
| `meta(ctx)` | `#{ api: 1, language }` | 对象 | 返回历法稳定 `id`、本地化 `name` / `description`、分量数 `components`（1..=64）与各分量起始值 `minimums` |
| `from_tick(tick)` | 64 位整数刻度 | 长度为 `components` 的整数数组 | 将刻度转换为合法归一化日期 |
| `to_tick(values)` | 长度为 `components` 的整数数组 | 64 位整数 | 把未归一化或已归一化时间转成可比较、可相减的刻度 |
| `humanizer(ctx)` | `#{ api: 1, values, precision, language }` | 字符串 | 按模糊精度与语言格式化已归一化日期 |

### `meta(ctx)` 返回字段

| 字段 | 必填 | 类型 | 说明 |
| --- | --- | --- | --- |
| `id` | 是 | 非空字符串 | 稳定的语义标识（如 `"gregorian"`、`"lunar-settlement"`），同时透传给 `story.date.id` 与可计算属性的 `ctx.calendar.id` |
| `name` | 是 | 非空字符串 | 本地化历法名称，显示在 Story 页 |
| `description` | 否 | 字符串 | 本地化历法说明，显示在 Story 页 |
| `components` | 是 | 整数（1..=64） | 时间分量数 |
| `minimums` | 是 | 长度等于 `components` 的整数数组 | 绝对时间缺省尾部分量时的补齐起始值（公历为 `[0, 1, 1, 0, 0, 0]`）；非首位分量小于对应起始值会被宿主拒绝 |

宿主在 `calendar.rs` 中统一实现 `normalize(values) = from_tick(to_tick(values))`，并核对 `to_tick(from_tick(tick)) == tick` 的往返一致性；脚本无需定义 `normalize`。`to_tick` 必须接受未归一化的日期分量，支持进位和借位。`humanizer(ctx)` 通过 `ctx.precision` 获知作者实际写了几个分量（如只写了年份时 `precision == 1`），用于保留模糊时间展示。

`ctx.language` 为应用设置的 `zh` 或 `en`。应用切换语言后会重新解析文档，从而刷新 `meta(ctx)` 与 `humanizer(ctx)` 的本地化结果。建议让 `from_tick` 与 `to_tick` 保持语言无关。

## 语法要点

Rune 的整数运算沿用 Rust 语义：溢出和除零都会报错，`/` 向零截断，`%` 的符号跟随被除数。变量默认可变且不支持 `mut` 修饰符；函数参数是只读绑定，函数内重绑同名变量即可更新值。模板字符串是宏调用，不能直接跟在 `return` 之后，先赋值再返回。脚本主动报错调用 `panic(message)`，消息会出现在文档错误里。

## 最小脚本示例

下面的三分量历法把每年视作 10 个月、每月 30 天，`to_tick` 的单位为天：

```rune
pub fn meta(ctx) {
    let zh = ctx.language == "zh";
    #{
        id: "ten-month",
        name: if zh { "十月历" } else { "Ten Month Calendar" },
        description: if zh { "每年十个月，每月三十天。" } else { "Ten months per year, thirty days per month." },
        components: 3,
        minimums: [0, 1, 1],
    }
}

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

pub fn humanizer(ctx) {
    let value = ctx.values;
    let precision = ctx.precision;
    if ctx.language == "zh" {
        let stamp = if precision <= 1 {
            `${value[0]}年`
        } else if precision == 2 {
            `${value[0]}年${value[1]}月`
        } else {
            `${value[0]}年${value[1]}月${value[2]}日`
        };
        return stamp;
    }
    if precision <= 1 {
        `Year ${value[0]}`
    } else if precision == 2 {
        `Year ${value[0]}, month ${value[1]}`
    } else {
        `Year ${value[0]}, month ${value[1]}, day ${value[2]}`
    }
}
```

脚本读取、编译或接口检查失败时，文档解析会报错；编译诊断带源码行列定位。脚本编译产物按 `(路径, 源码)` 缓存，修改 YAML 时未改动的脚本免于重编译；每次调用受指令预算限制，超限中断并报错。

## 宿主代码层级

- `src-tauri/src/calendar.rs`：统一加载与校验 Rune 插件，执行分量/起始值校验、历算、时间展示和元信息输出。
- `src-tauri/src/plugin_loader.rs`：Rune 宿主的全局复用运行时、编译缓存、接口检查与限额调用。
- `src-tauri/src/weft/phase.rs`：定义 `Phase`，解析 `time_unit` 或 `[phase, 偏移分量…]` 的递归结构；由 `Calendar::resolve` 按由内到外的顺序应用每层偏移并保留最大有效精度。
