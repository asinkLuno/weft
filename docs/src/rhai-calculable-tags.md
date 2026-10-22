# Rhai calculable tags 脚本

Calculable tag 根据实体数据算出一条归属链。计算先返回稳定的字符串 ID，再由 `humanizer` 把它们变为当前语言下的名称。`constellation` 的脚本实现见 `examples/constellation.rhai`。

## 注册与使用

在 `story.calculatable_tags` 中用 `ID: 路径` 注册脚本。路径相对于故事 YAML 所在目录，实体通过 `calculable_tags` 引用注册 ID。

```yaml
story:
  title: 示例
  calculatable_tags:
    age_group: age_group.rhai
    constellation: constellation.rhai

entity:
  Alice:
    base_time: [2000, 1, 1]
    calculable_tags: [age_group, constellation]
```

所有标签都必须显式注册，包括 `constellation`。Story 先加载脚本，并将名称和描述保存到 `story.calculatable_tags`，集合使用已编译的运行时计算归属，再构建实体；运行时实体只保存 `belong_to` 引用。

## 必须实现的公开函数

加载时检查函数名和参数个数，函数必须公开。返回值在调用时检查。

| 函数 | 输入 | 返回值 | 用途 |
| --- | --- | --- | --- |
| `serialize(language)` | 语言代码 | `{ name, description }` | 标签本地化名称和描述 |
| `calculable_tag(ctx)` | 上下文 map | 字符串 ID 数组 | 计算归属链，不依赖显示语言 |
| `humanizer(kind, value, language)` | 种类、计算结果、语言 | 字符串数组 | 把归属链转成可显示的名称 |

当前宿主调用 `humanizer("chain", calculable_tag(ctx) 的结果, ctx.language)`。返回的数组按顺序表示 `实体 → chain[0] → chain[1] → …`。空数组表示不添加归属。后端按返回的字符串 ID 查找实体，并为尚不存在的 ID 创建实体；链首并入实体的 `belong_to`，撞到已有实体就停止创建且不改动它自己的归属。计算结束后会检查归属关系是否成环。

`humanizer` 返回的每个元素都必须是字符串。归属链在内存中生成，不会写回 YAML；切换应用语言后会重新解析文档，用新的名称生成归属实体。

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
| `entity.belong_to` | 实体当前的归属 ID 数组，可能包含此前已应用的 calculable tag 结果 |

`calculable_tag` 可以读取 `ctx.language` 来生成本地化的错误信息，但计算结果应使用稳定 ID；正常显示由 `humanizer` 处理。

## 最小脚本示例

```rhai
fn serialize(language) {
    #{ name: if language == "zh" { "纪元" } else { "Era" }, description: "" }
}

fn calculable_tag(ctx) {
    let time = ctx.entity.base_time;
    if time == () { return []; }
    let id = if time[0] >= 2000 { "new_era" } else { "old_era" };
    [id, "era_group"]
}

fn humanizer(kind, value, language) {
    if kind != "chain" { throw `unknown display kind: ${kind}`; }
    let result = [];
    for id in value {
        if language == "zh" {
            if id == "new_era" { result.push("新纪元"); }
            else if id == "old_era" { result.push("旧纪元"); }
            else if id == "era_group" { result.push("纪元分类"); }
            else { throw `unknown tag ID: ${id}`; }
        } else {
            if id == "new_era" { result.push("New Era"); }
            else if id == "old_era" { result.push("Old Era"); }
            else if id == "era_group" { result.push("Era Group"); }
            else { throw `unknown tag ID: ${id}`; }
        }
    }
    result
}
```

所有已注册的 calculable tag 在加载时都会读取、编译并检查接口；只有被实体引用的脚本才会执行。脚本运行受操作数、调用层数及容器大小等资源限制。
