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
| `calculable_tag` | 可计算标签注册表，见 [Calculable tags 脚本](rhai-calculable-tags.md) |

除 `story` 外都可以省略。反序列化时未声明的顶层键会被静默滤掉。

## id 与引用

实体可以通过可选的 `avatar` 指定本地头像路径，相对路径以 YAML 所在目录为基准，也支持绝对路径：

```yaml
entity:
  哈利·波特:
    avatar: images/harry.png
    description: 霍格沃茨的学生。
```

头像会显示在实体卡片和归属悬停预览中。路径必须指向已存在的图片文件（按扩展名判断，支持 bmp、gif、ico、jpeg、jpg、png、svg、webp），否则解析报错。未配置或图片加载失败时，显示实体 ID 缩写：多个词取前两个词的首字，单个词取前两个字符（如 `Harry Potter` → `HP`，`霍格沃茨` → `霍格`）。

文档内部靠 id 互相引用，引用不存在的 id 会解析报错：

- **实体 id** 是 `entity` 下的键。实体的 `belong_to`（归属）、`entity_link` 的关系对、事件的 `entities`（参与实体）、叙事线的 `observer` 都引用实体 id。
- **事件 id** 是 `组名/标题`。叙事线的 `subject` 引用事件 id；事件标题全局唯一（跨组同名也算重复），因此引用可以省略组名只写标题。

归属图（`belong_to`）不许成环，否则按层级展示无法终止。

## 运行时归属森林

YAML 输入中的 `RawEntity` 保留 `belong_to`。构建后的 `Entity` 只保存实体自身的数据，归属关系单独输出到 `entity_forest`：

```rust
pub struct EntityBelongTo {
    pub entity: Arc<Entity>,
    pub children: EntityForest,
}

pub type EntityForest = Vec<EntityBelongTo>;
```

树从父实体指向直接归属于它的子实体，无归属的实体（包括独立实体）作为根节点。多重归属的实体出现在各个父节点下，节点共享实体数据。根节点和同级子节点按 ID 排序，同一父子关系只保留一次。

序列化给前端时，节点的 `entity` 是实体 ID，完整实体数据统一保存在顶层 `entity` 表中。例如 `哈利·波特` 的 `belong_to: [格兰芬多]` 会生成：

```json
{
  "entity": {
    "格兰芬多": { "id": "格兰芬多", "base_time": null, "description": null },
    "哈利·波特": { "id": "哈利·波特", "base_time": null, "description": null }
  },
  "entity_forest": [
    {
      "entity": "格兰芬多",
      "children": [{ "entity": "哈利·波特", "children": [] }]
    }
  ]
}
```

## 解析流程

后端读取文档后依次执行：

1. 反序列化成 `RawWeftDoc` 等输入类型，未知键滤掉；
2. 按段检查：story → entity → event → narrative，每段先引用后结构，断开的引用按段批量报出——
   - story：通过 `date_mode` 的脚本路径加载历法，留空则使用内置公历，并提前加载可计算标签插件；
   - entity：校验 `belong_to` 与 `entity_link` 的实体引用及归属环，先构建实体表，执行可计算标签创建链上实体并复检归属环，再构建归属森林和实体关系；
   - event：参与实体存在，事件标题全局唯一；
   - narrative：`observer` 与 `subject` 引用的事件、实体存在；
3. 使用已加载的历法，把所有时间归一化、算出可比较的刻度与展示文案；
4. 序列化成 JSON 交给前端，归属关系由 `entity_forest` 提供。

任何一步失败都会阻止展示。
