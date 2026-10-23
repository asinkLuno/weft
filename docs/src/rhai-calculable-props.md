# Rhai calculable props 脚本

Calculable prop 用一段 Rhai 脚本从实体数据算出一个属性值：`calculable_prop(ctx)` 的返回值直接写进该实体 `extra_props` 的属性注册 ID 下，不生成实体，也不改归属关系。`constellation` 的脚本实现见 `examples/constellation.rhai`。

## 注册与使用

在 `story.calculatable_props` 中用 `ID: 路径` 注册脚本。路径相对于故事 YAML 所在目录，实体通过 `calculable_props` 引用注册 ID：

```yaml
story:
  title: 示例
  calculatable_props:
    age_group: age_group.rhai
    constellation: constellation.rhai

entity:
  Alice:
    base_time: [2000, 1, 1]
    calculable_props: [age_group, constellation]
```

所有属性都必须显式注册，包括 `constellation`。加载时读取、编译并检查接口；构建实体时逐个执行被引用的脚本，把返回值写进 `extra_props`。例如上面的 Alice 最终得到 `extra_props: { constellation: "摩羯座" }`。

## 必须实现的公开函数

加载时检查函数名和参数个数，函数必须公开。返回值在调用时检查。

| 函数 | 输入 | 返回值 | 用途 |
| --- | --- | --- | --- |
| `calculable_prop(ctx)` | 上下文 map | 属性值 | 计算属性值，直接进入 `extra_props`，可按 `ctx.language` 本地化 |

返回值支持字符串、整数、浮点、布尔，以及数组与 map 的组合（复合结构里的空值落为 `null`）；返回 `()` 表示本次不写入。写入 `extra_props` 的键就是注册 ID，与输入字段同名时以计算结果为准。

```rhai
fn calculable_prop(ctx) {
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

`ctx` 是 map，当前包含：

| 字段 | 内容 |
| --- | --- |
| `api` | 数字 `1` |
| `date_mode` | 自定义历法脚本路径；使用内置公历时为空值 |
| `language` | 应用设置语言，`zh` 或 `en` |
| `entity.id` | 实体 ID |
| `entity.base_time` | 按当前历法归一化后的整数数组；未填写时为空值 |
| `entity.description` | 实体描述；未填写时为空值 |
| `entity.belong_to` | 实体输入的归属 ID 数组 |

计算可以读取 `ctx.language` 生成本地化的错误信息并直接 `throw`。结果只在内存中生成，不会写回 YAML；切换应用语言后会重新解析文档，用新语言重新计算。只有被实体引用的脚本才会执行计算。脚本运行受操作数、调用层数及容器大小等资源限制。
