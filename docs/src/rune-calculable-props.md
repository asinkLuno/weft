# Rune calculable props 脚本

Calculable prop 用一段 Rune 脚本从实体数据算出一个属性值：`calculable_prop(ctx)` 的返回值直接写进该实体 `extra_props` 的属性注册 ID 下，不生成实体，也不改归属关系。`constellation` 的脚本实现见 `examples/constellation.rn`。

## 注册与使用

在 `story.calculatable_props` 中用 `ID: 路径` 注册脚本。路径相对于故事 YAML 所在目录，实体通过 `calculable_props` 引用注册 ID：

```yaml
story:
  title: 示例
  calculatable_props:
    age_group: age_group.rn
    constellation: constellation.rn

entity:
  Alice:
    base_time: [2000, 1, 1]
    calculable_props: [age_group, constellation]
```

所有属性都必须显式注册，包括 `constellation`。加载时读取、编译并检查接口；构建实体时逐个执行被引用的脚本，把返回值写进 `extra_props`。例如上面的 Alice 最终得到 `extra_props: { constellation: "摩羯座" }`。

## 必须实现的公开函数

Rune 中未被引用的私有函数不会进入编译产物，因此接口函数必须声明 `pub`。加载时检查 `calculable_prop` 是否以 `pub` 函数定义；参数个数在运行期确定，返回值在调用时检查。

| 函数 | 输入 | 返回值 | 用途 |
| --- | --- | --- | --- |
| `calculable_prop(ctx)` | 上下文对象 | 属性值 | 计算属性值，直接进入 `extra_props`，可按 `ctx.language` 本地化 |

返回值支持字符串、整数、浮点、布尔，以及数组与对象的组合（复合结构里的空值落为 `null`）；返回 `()` 表示本次不写入。写入 `extra_props` 的键就是注册 ID，与输入字段同名时以计算结果为准。

```rune
pub fn calculable_prop(ctx) {
    let zh = ctx.language == "zh";
    let time = ctx.entity.base_time;
    if time == () { return (); }
    if time[0] >= 2000 {
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
| `date_mode` | 自定义历法脚本路径；使用内置公历时为空值 |
| `language` | 应用设置语言，`zh` 或 `en` |
| `entity.id` | 实体 ID |
| `entity.base_time` | 按当前历法归一化后的整数数组；未填写时为空值 |
| `entity.description` | 实体描述；未填写时为空值 |
| `entity.belong_to` | 实体输入的归属 ID 数组 |

计算可以读取 `ctx.language` 生成本地化的错误信息并调用 `panic(message)` 报错，消息会出现在文档错误里。结果只在内存中生成，不会写回 YAML；切换应用语言后会重新解析文档，用新语言重新计算。只有被实体引用的脚本才会执行计算。脚本执行受指令预算限制，超限中断并报错。

数据不齐是常态，别把「算不出来」做成报错。没写 `base_time` 的实体没有星座，脚本应返回 `()` 让属性不写入，而不是 `panic`——否则同一个属性就没法挂到整批实体上，只能逐个筛查谁写了日期。反过来，历法语义变了（自定义历法下 `bt[1]` 不再是公历月份）属于脚本前提被破坏，此时必须报错：返回一个错的值比报错更糟。

两个容易踩的坑：

- **归一化会补齐缺失分量**。`base_time: 1980` 经归一化后是 `[1980, 1, 1, 0, 0, 0]`，与「真的一月一日出生」无法区分。所以脚本里写 `bt.len() < 3` 或校验 `bt[1]` / `bt[2]` 的范围都没用：长度恒等于历法分量数，月日恒合法。要算星座就必须在 YAML 里写全 年 月 日。
- **`if` 里调 `panic` 会丢掉消息**。`panic(message)` 返回 unit，写在 `if` 分支里会被推断成该分支的值，于是整个函数变成「有时返回 unit、有时返回 String」，实际报的是 `Expected type String but found Tuple`，本地化消息根本看不到。把 panic 放进 `match` 的分支才正常：

```rune
fn require_gregorian(date_mode) {
    match date_mode {
        () => (),
        path => panic(path),
    }
}
```
