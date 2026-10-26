# Rune 历法脚本

## 注册与选用

在 `story.date_mode` 中直接填写 Rune 脚本路径，路径相对于故事 YAML 所在目录，无需注册名或顶层 `calendar` 注册表。

```yaml
story:
  title: 示例
  date_mode: my_calendar.rn
```

省略 `date_mode`、填写空值或空字符串时使用内置公历。内置脚本在 `src-tauri/rune/gregorian.rn`，以应用资源文件打包并统一按路径加载。story 阶段检查路径必须指向现有的 `.rn` 文件；脚本读取、编译与接口校验在 Calendar 加载时执行。显示语言跟随应用设置（`zh` 或 `en`）。

## 必须实现的公开函数

以下是脚本接口；Rune 中未被引用的私有函数不会进入编译产物，因此接口函数必须声明 `pub`。加载时检查 `from_tick`、`to_tick`、`serialize`、`humanizer` 和 `components` 是否以 `pub` 函数定义；参数个数在运行期确定，不匹配会在调用时报错。返回值仍在实际调用时校验。

| 函数 | 输入 | 返回值 | 用途 |
| --- | --- | --- | --- |
| `components()` | 无 | 整数 | 时间分量数，取值 **1 到 64** |
| `serialize(language)` | 语言代码 | 对象 | Story 页展示信息，如本地化的 `name`、`description` |
| `from_tick(tick)` | 64 位整数刻度 | 归一化的日期数组 | 将刻度转换为合法日期 |
| `to_tick(values)` | 整数数组 | 64 位整数 | 把时间转成可比较、可相减的刻度 |
| `humanizer(values, language)` | 已归一化的日期数组、语言 | 字符串 | 仅格式化日期 |

`components()` 在加载时调用一次，宿主据此校验分量数，不执行其余函数来推导长度。用户写的数组可以省略末尾分量，后端会补 `0`；超过该长度会报错。`from_tick` 必须返回恰好 `components` 个整数。宿主在 `calendar.rs` 中统一实现 `normalize(values) = from_tick(to_tick(values))`；脚本无需定义 `normalize`。`to_tick` 必须接受未归一化的日期分量，支持进位和借位。`to_tick` 的数值用于事件排序、起止先后判断和叙事线相对刻度，须保持同一历法内的时间顺序，并处于 64 位整数范围内。

公历 `to_tick(date)` 返回日期相对零年一月一日的秒数；全零数组也表示原点。月、日为零时按 1 计算，`normalize` 会输出补齐后的合法日期。日期间隔为 `to_tick(end) - to_tick(start)`，除以 `86400` 得到天数，闰年和跨月由日期转换处理。`humanizer` 不再调用 `normalize`。

宿主通过 `calendar_components` 直接输出时间分量数，不包含本地化文字。`serialize(language)` 生成的名称与描述保存到 `story.date`，前端从中读取 `name` 和 `description`；原始 `date_mode` 路径只存在于 YAML 输入。

`language` 是应用设置中的 `zh` 或 `en`。应用切换语言后会重新解析当前文档，因此 `humanizer` 会收到新的语言。建议只在 `serialize` 和 `humanizer` 中按语言选择显示文字，让 `from_tick` 和 `to_tick` 的结果不受语言影响。

## 语法要点

Rune 的整数运算沿用 Rust 语义：溢出和除零都会报错，`/` 向零截断，`%` 的符号跟随被除数。变量默认可变且不支持 `mut` 修饰符；函数参数是只读绑定，函数内重绑同名变量即可更新值。模板字符串是宏调用，不能直接跟在 `return` 之后，先赋值再返回。脚本主动报错调用 `panic(message)`，消息会出现在文档错误里。

## 最小脚本示例

下面的三分量历法把每年视作 10 个月、每月 30 天。示例仅演示接口形状；`to_tick` 的单位为天。

```rune
pub fn components() { 3 }

pub fn from_tick(total) {
    let mut year = total / 300;
    let mut remainder = total % 300;
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
        description: if language == "zh" { "每年十个月，每月三十天。" } else { "Ten months per year, thirty days per month." },
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

脚本读取、编译或接口检查失败时，文档解析会报错；编译诊断带源码行列定位。加载时调用 `components()` 并检查函数接口；函数在实际计算或展示时调用。脚本执行受指令预算限制，超限中断并报错。

## 宿主代码层级

- `src-tauri/src/calendar.rs`：统一加载与校验 Rune 插件，执行分量校验、历算、时间展示和 Story 页序列化。
- `src-tauri/src/plugin_loader.rs`：Rune 宿主的公共上下文、编译、接口检查与限额调用。
- `src-tauri/src/weft/phase.rs`：定义 `TimeUnit`（不定长整数数组），解析 `time_unit` 或 `[phase, 偏移分量…]` 的递归结构。锚点在首项，负数偏移表示减法；递归 flatten 只逐分量求和并检查溢出，不做进位。例如 `[[2025, 4, 2, 23, 50, 0], 0, 0, 0, 20]` 展平为 `[2025, 4, 2, 23, 70, 0]`，最后由历法 `normalize` 处理进位。
