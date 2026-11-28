# 自定义历法与可计算属性（Rune 脚本）

WEFT 内置了沙箱化的 [Rune](https://rune-rs.github.io/) 脚本引擎（`.rn`），用于扩展两类能力，路径均相对于当前 `.weft` 文件解析：

1. **自定义历法 / 时间轴（`story.date_mode`）**：定义任意离散线性时间坐标（架空纪年、农历/殖民历、财年季度周、任务计时等）。省略时默认使用内置公历（`gregorian`）。
2. **实体可计算属性（`story.calculable_props`）**：基于实体的 `base_time`、`extra_props` 与历法信息，动态派生附加属性（如星座、年代标签、阵营批次），写入编译后的 `entity.extra_props`。

## 沙箱与运行限制

- **无 I/O**：脚本运行在无标准输入输出、无文件与网络访问的沙箱中，**不能**使用 `println!`、`print!`、`dbg!`。
- **资源限额**：单次入口调用上限 **1,000,000** 条指令；脚本文件上限 **1 MB**；扩展名必须为 `.rn`。
- **上下文约定**：
  - `ctx.api` 固定为 `1`。
  - `ctx.language` 为 `"zh"` 或 `"en"`（随 CLI `--lang` 或界面语言切换），建议同时兼容中英文输出。
- **Rune 常见注意点**：
  - JSON 的 `null` 对应 Rune 的单元值 `()`；匿名对象语法为 `#{ key: value }`；模板字符串使用反引号 `` `${expr}` ``。
  - Rune 的整数 `/` 和 `%` 向零截断；当刻度或偏移可能为负数时，建议自写向下取整除法 `floor_div` 与正余数 `pos_mod`。
  - 在需要触发运行期错误（`panic(msg)`）的分支中，优先使用 `match` 而非返回非单元值的 `if`，避免类型推断掩盖错误消息。

## 自定义历法（`date_mode`）

在 `story` 中指定相对路径：

```weft
story {
  title     = "月面编年史"
  date_mode = "lunar.rn"
}
```

### 导出接口

历法 `.rn` 脚本必须导出前 4 个 `pub fn`，第 5 个可选：

| 函数                            | 参数与返回值                                                                                               | 说明                                                                                              |
| ------------------------------- | ---------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| `meta(ctx)`                     | `ctx: #{ api, language }` → `#{ id, name, description?, components, minimums }`                            | 返回历法元信息、分量数与各分量下界                                                                |
| `to_tick(values)`               | `values: Vec<i64>`（长度 = `components`）→ `i64`                                                           | 将分量数组映射为线性整数刻度（须能处理未归一化的溢出/负偏移分量）                                 |
| `from_tick(tick)`               | `tick: i64` → `Vec<i64>`（长度 = `components`）                                                            | 将线性刻度反算为归一化后的合法分量数组                                                            |
| `humanizer(ctx)`                | `ctx: #{ api, values, precision, language }` → `String`                                                    | 按 `ctx.precision` 格式化归一化时刻                                                               |
| `delta_humanizer(ctx)` *(可选)* | `ctx: #{ api, language, from: #{ values, tick, precision }, to: #{ values, tick, precision } }` → `String` | 格式化有符号时间差 `to - from`（用于事件与实体 `base_time` 相对时差；省略时退化为带符号 tick 差） |

### 编译器强制校验的不变量

1. **`meta` 校验**：`id` 与 `name` 必须为非空字符串；`components` 必须在 `1..=64` 之间；`minimums` 长度必须等于 `components`。其中 `minimums[0]` 仅用于展示与补全（最高位允许负数），第 2 项起若绝对时间小于对应 `minimums[i]` 会报 `time-component-below-minimum`。
2. **归一化与进位**：求解 `anchor + offset` / `anchor - offset` 时，宿主先按分量相加减，再直接传给 `to_tick(values)`，随后调用 `from_tick(tick)` 得到归一化日期。因此 `to_tick` 必须能线性折算超界或负分量（例如月份 `13` 或 `0`、小时 `-5`）。
3. **往返一致性**：对每次归一化结果 `norm = from_tick(tick)`，宿主都会核对 `to_tick(norm) == tick`，不一致会报 `calendar-round-trip-mismatch`。
4. **模糊精度（`precision`）**：`ctx.precision` 表示作者写到的最细分量位置（相对时间取基准与偏移的最大值），`humanizer` 应据此决定展示粒度。

### 最小示例：固定进制历法（`[财年, 季度, 周]`）

```rune
// 每年 4 个季度，每季度 13 周，共 52 周。

pub fn meta(ctx) {
    let zh = ctx.language == "zh";
    #{
        id: "fiscal-quarter-week",
        name: if zh { "财年季度周历" } else { "Fiscal Quarter-Week Calendar" },
        description: if zh {
            "每年 4 个季度，每季度 13 周。"
        } else {
            "4 quarters of 13 weeks each (52 weeks per year)."
        },
        components: 3,
        minimums: [1, 1, 1],
    }
}

const WEEKS_PER_QUARTER = 13;
const WEEKS_PER_YEAR = 52;

fn floor_div(a, b) {
    let q = a / b;
    if a % b < 0 { q - 1 } else { q }
}

fn pos_mod(a, b) {
    let r = a % b;
    if r < 0 { r + b } else { r }
}

pub fn to_tick(values) {
    let year = values[0];
    let quarter = values[1];
    let week = values[2];
    (year - 1) * WEEKS_PER_YEAR + (quarter - 1) * WEEKS_PER_QUARTER + (week - 1)
}

pub fn from_tick(tick) {
    let year = floor_div(tick, WEEKS_PER_YEAR) + 1;
    let rem = pos_mod(tick, WEEKS_PER_YEAR);
    let quarter = rem / WEEKS_PER_QUARTER + 1;
    let week = rem % WEEKS_PER_QUARTER + 1;
    [year, quarter, week]
}

pub fn humanizer(ctx) {
    let v = ctx.values;
    let p = ctx.precision;
    let y = v[0];
    let q = v[1];
    let w = v[2];
    if ctx.language == "zh" {
        if p <= 1 {
            `${y}财年`
        } else if p == 2 {
            `${y}财年 Q${q}`
        } else {
            `${y}财年 Q${q} 第${w}周`
        }
    } else if p <= 1 {
        `FY${y}`
    } else if p == 2 {
        `FY${y} Q${q}`
    } else {
        `FY${y} Q${q} W${w}`
    }
}
```

带闰年/闰月的不规则历法与 `delta_humanizer` 完整写法，可参考仓库中的内置公历 `crates/weft-core/rune/gregorian.rn` 和月面殖民历示例 `examples/lunar.rn`。

## 可计算属性（`calculable_props`）

先在 `story` 中注册属性 ID 与脚本路径，再在需要计算的 `entity` 上启用：

```weft
story {
  title            = "哈利·波特与魔法石"
  calculable_props = {
    constellation = "constellation.rn"
  }
}

entity 哈利·波特 {
  base_time        = [1980, 7, 31]
  calculable_props = [constellation]
}
```

### 导出接口

脚本必须导出 `pub fn calculable_prop(ctx)`。宿主传入的 `ctx` 结构如下：

```json
{
  "api": 1,
  "language": "zh",
  "calendar": {
    "id": "gregorian",
    "components": 6,
    "minimums": [0, 1, 1, 0, 0, 0]
  },
  "entity": {
    "id": "哈利·波特",
    "base_time": {
      "values": [1980, 7, 31, 0, 0, 0],
      "precision": 3,
      "tick": 62501385600
    },
    "description": "大难不死的男孩。",
    "extra_props": {
      "kind": "人物"
    }
  }
}
```

- 实体未填写 `base_time` 或 `description` 时，对应字段在 Rune 中为 `()`。
- `ctx.entity.extra_props` 仅包含实体的原始静态扩展字段，不包含其他可计算属性的输出。
- **返回值**：
  - 返回 `()` 表示跳过不写入（适用于缺少 `base_time` 或精度不足的实体，不视为错误）；
  - 返回字符串、整数、浮点、布尔值、数组 `[...]` 或匿名对象 `#{ ... }` 时，宿主会将结果写入 `entity.extra_props[<属性ID>]`。

### 最小示例

```rune
fn require_gregorian(calendar_id, zh) {
    match calendar_id {
        "gregorian" => (),
        other => {
            let msg = if zh {
                `该属性需要公历，当前历法是 ${other}`
            } else {
                `this property requires gregorian calendar, got ${other}`
            };
            panic(msg)
        }
    }
}

pub fn calculable_prop(ctx) {
    let zh = ctx.language == "zh";
    require_gregorian(ctx.calendar.id, zh);

    let bt = match ctx.entity.base_time {
        () => return (),
        bt => bt,
    };
    if bt.precision < 2 {
        return ();
    }

    let year = bt.values[0];
    let month = bt.values[1];
    let quarter = (month - 1) / 3 + 1;
    if zh {
        `${year}年 Q${quarter}`
    } else {
        `${year}-Q${quarter}`
    }
}
```

完整示例见仓库中的 `examples/constellation.rn`。

## 验证插件

写完 `.rn` 后，使用**不带 `--syntax-only`** 的 `weft check` 分别验证中英文与边界输入（缺省 `base_time`、低精度时间、跨周期与负偏移等）：

```bash
weft check story.weft --deny-warnings --lang zh
weft check story.weft --deny-warnings --lang en
```
