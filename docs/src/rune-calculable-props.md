# Rune calculable props 脚本

Calculable prop 用一段 Rune 脚本从实体数据算出一个属性值：`calculable_prop(ctx)` 的返回值直接写进该实体 `extra_props` 的属性注册 ID 下，不生成实体，也不改归属关系。`constellation` 的脚本实现见 `examples/constellation.rn`。

## 注册与使用

在 `story.calculable_props` 中用 `ID: 路径` 注册脚本。路径相对于故事 YAML 所在目录，实体通过 `calculable_props` 引用注册 ID：

```yaml
story:
  title: 示例
  calculable_props:
    age_group: age_group.rn
    constellation: constellation.rn

entity:
  Alice:
    base_time: [2000, 1, 1]
    calculable_props: [age_group, constellation]
```

所有属性都必须显式注册，包括 `constellation`。加载时按注册 ID 排序依次校验（必须是已存在的 `.rn` 文件）、读取、编译并检查接口；构建实体时按实体 ID 排序逐个执行被引用的脚本，把返回值写进 `extra_props`。例如上面的 Alice 最终得到 `extra_props: { constellation: "摩羯座" }`。

## 必须实现的公开函数

Rune 中未被引用的私有函数不会进入编译产物，因此接口函数必须声明 `pub`。加载时检查 `calculable_prop` 是否以 `pub` 函数定义；参数个数在运行期确定，返回值在调用时检查。

| 函数 | 输入 | 返回值 | 用途 |
| --- | --- | --- | --- |
| `calculable_prop(ctx)` | 上下文对象 | 属性值 | 计算属性值，直接进入 `extra_props`，可按 `ctx.language` 本地化 |

返回值支持字符串、整数、浮点、布尔，以及数组与对象的组合（复合结构里的空值落为 `null`）；返回 `()` 表示本次不写入。写入 `extra_props` 的键就是注册 ID，与输入字段同名时以计算结果为准。

```rune
pub fn calculable_prop(ctx) {
    let zh = ctx.language == "zh";
    let bt = match ctx.entity.base_time {
        () => return (),
        bt => bt,
    };
    if bt.values[0] >= 2000 {
        if zh { "新纪元" } else { "New Era" }
    } else {
        if zh { "旧纪元" } else { "Old Era" }
    }
}
```

## `ctx` 字段

`ctx` 是对象，当前包含：

| 字段 | 内容 |
| --- | --- |
| `api` | 数字 `1` |
| `language` | 应用设置语言，`zh` 或 `en` |
| `calendar.id` | 当前历法 `meta(ctx)` 声明的稳定标识（内置公历为 `"gregorian"`） |
| `calendar.components` | 当前历法的时间分量数 |
| `calendar.minimums` | 当前历法各分量的起始值数组 |
| `entity.id` | 实体 ID |
| `entity.base_time` | 已解析基准时间对象 `#{ values, precision, tick }`；未填写时为空值 `()` |
| `entity.base_time.values` | 按当前历法补齐并归一化后的整数数组 |
| `entity.base_time.precision` | 基准时间的有效分量数（如只写年份 `1881` 时为 `1`，写全年月日为 `3`） |
| `entity.base_time.tick` | 基准时间对应的 64 位整数刻度 |
| `entity.description` | 实体描述；未填写时为空值 `()` |
| `entity.belong_to` | 实体直接上级归属 ID 数组（已排序去重） |
| `entity.extra_props` | 实体原始自定义输入字段的只读快照（不受同实体其他计算属性执行顺序影响） |

计算可以读取 `ctx.language` 生成本地化的错误信息并调用 `panic(message)` 报错，消息会出现在文档错误里。结果只在内存中生成，不会写回 YAML；切换应用语言后会重新解析文档，用新语言重新计算。只有被实体引用的脚本才会执行计算。脚本执行受指令预算限制，超限中断并报错。

数据不齐是常态，别把「算不出来」做成报错。没写 `base_time` 或精度不足（例如只写了出生年份 `bt.precision < 3`）的实体没有星座，脚本应返回 `()` 让属性不写入，而不是 `panic`——否则同一个属性就没法挂到整批实体上，只能逐个筛查谁写了日期。反过来，历法语义变了（如 `ctx.calendar.id != "gregorian"`）属于脚本前提被破坏，此时必须报错：返回一个错的值比报错更糟。

注意两点 Rune 语法细节：

- 判断可能为空的对象字段（如 `ctx.entity.base_time`）请用 `match ... { () => return (), v => v }`，不要写 `bt == ()`（Rune 中 `Object` 与 `()` 做 `==` 比较会报类型不匹配）。
- 把 `panic(message)` 写在 `if` 分支里会被推断成 unit 类型从而丢失消息，放在 `match` 分支中才会把消息完整抛给宿主：

```rune
fn require_gregorian(calendar_id) {
    match calendar_id {
        "gregorian" => (),
        other => panic(`requires gregorian, got ${other}`),
    }
}
```
