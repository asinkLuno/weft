# WeftDoc 总览

WeftDoc 是 Weft 的故事文档格式：一个 YAML 文件（习惯命名 `story.yaml`）描述一个故事的元信息、实体、实体关系、事件与叙事线，并注册 Rhai 插件。仓库中的 `examples/story.yaml` 是一份完整示例。

## 顶层键

| 键 | 内容 |
| --- | --- |
| `story` | 故事元信息，唯一必填的顶层键，见 [story](weftdoc-story.md) |
| `entity` | 实体表，键为实体 id；人物、团体、地点、道具都算实体 |
| `entity_link` | 实体关系对，按组组织 |
| `event` | 事件，按组组织，事件 id 为 `组名/标题` |
| `narrative` | 叙事线：以观察者实体的时间为基准展开的事件序列 |

除 `story` 外都可以省略。反序列化时未声明的顶层键会被静默滤掉；实体内部的未声明键不丢弃，收进 `extra_props`（见下文）。

## id 与引用

实体可以通过可选的 `avatar` 指定本地头像路径，相对路径以 YAML 所在目录为基准，也支持绝对路径：

```yaml
entity:
  哈利·波特:
    avatar: images/harry.png
    description: 霍格沃茨的学生。
```

头像会显示在实体卡片和归属悬停预览中。路径必须指向已存在的图片文件（按扩展名判断，支持 bmp、gif、ico、jpeg、jpg、png、svg、webp），否则解析报错。未配置或图片加载失败时，显示实体 ID 缩写：多个词取前两个词的首字，单个词取前两个字符（如 `Harry Potter` → `HP`，`霍格沃茨` → `霍格`）。

## extra_props：自定义字段

实体上不属于内置键（`base_time`、`description`、`avatar`、`belong_to`、`calculable_props`）的字段不参与校验，原样收进 `extra_props` 透传给前端：

```yaml
entity:
  哈利·波特:
    description: 霍格沃茨的学生。
    gender: 男
    wand: 冬青木魔杖，凤凰羽毛杖芯
```

构建后的 `Entity.extra_props` 为 `{ "gender": "男", "wand": "冬青木魔杖，凤凰羽毛杖芯" }`；值为空时序列化中省略该字段。可计算属性（`calculable_props`）的计算结果也写进 `extra_props`，键为属性注册 ID，与输入字段同名时以计算结果为准，见 [Rhai calculable props 脚本](rhai-calculable-props.md)。

文档内部靠 id 互相引用，引用不存在的 id 会解析报错：

- **实体 id** 是 `entity` 下的键。实体的 `belong_to`（归属）、`entity_link` 的关系对、事件的 `entities`（参与实体）、叙事线的 `observer` 都引用实体 id。
- **事件 id** 是 `组名/标题`。叙事线的 `subject` 引用事件 id；事件标题全局唯一（跨组同名也算重复），因此引用可以省略组名只写标题。

归属图（`belong_to`）不许成环，否则按层级展示无法终止。

## 运行时归属

YAML 输入中的 `RawEntity` 保留 `belong_to`。构建时用 DAG 校验归属（引用存在、不成环、重复边去重），再把每个实体的直接上级反查写回 `Entity.belong_to`；DAG 只作为构建期的校验结构，不进入输出。

```rust
pub struct Entity {
    pub id: String,
    pub belong_to: Vec<String>,
    // ...
}
```

`belong_to` 已排序去重，没有上级时省略该字段，不再单独输出 `entity_forest`。例如 `哈利·波特` 的 `belong_to: [格兰芬多]` 会生成：

```json
{
  "entity": {
    "格兰芬多": { "id": "格兰芬多", "base_time": null, "description": null },
    "哈利·波特": {
      "id": "哈利·波特",
      "belong_to": ["格兰芬多"],
      "base_time": null,
      "description": null
    }
  }
}
```

## 解析流程

后端读取文档后依次执行：

1. 反序列化成 `RawWeftDoc` 等输入类型，未知键滤掉；
2. 按段检查：story → entity → event → narrative，每段先引用后结构，断开的引用按段批量报出——
   - story：通过 `date_mode` 的脚本路径加载历法，留空则使用内置公历，并提前加载可计算属性脚本；
   - entity：校验 `belong_to` 与 `entity_link` 的实体引用及归属环，构建实体表并执行可计算属性（结果写进 `extra_props`），再构建归属 DAG 并把直接上级反查写回实体；`entity_link` 只校验 ID，原样保留；
   - event：参与实体存在，事件标题全局唯一；
   - narrative：`observer` 与 `subject` 引用的事件、实体存在；
3. 使用已加载的历法，把所有时间归一化、算出可比较的刻度与展示文案；
4. 序列化成 JSON 交给前端，归属关系由实体的 `belong_to` 提供。

任何一步失败都会阻止展示。
