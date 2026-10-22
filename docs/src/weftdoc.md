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

文档内部靠 id 互相引用，引用不存在的 id 会解析报错：

- **实体 id** 是 `entity` 下的键。实体的 `belong_to`（归属）、`entity_link` 的关系对、事件的 `entities`（参与实体）、叙事线的 `observer` 都引用实体 id。
- **事件 id** 是 `组名/标题`。叙事线的 `subject` 引用事件 id；事件标题全局唯一（跨组同名也算重复），因此引用可以省略组名只写标题。

归属图（`belong_to`）不许成环，否则按层级展示无法终止。

## 解析流程

后端读取文档后依次执行：

1. 反序列化成 `WeftDoc`，未知键滤掉；
2. 按段检查：story → entity → event → narrative，每段先引用后结构，断开的引用按段批量报出——
   - story：通过 `date_mode` 的脚本路径加载历法，留空则使用内置公历，并提前加载可计算标签插件；
   - entity：`belong_to` 与 `entity_link` 引用的实体存在，归属图无环；
   - event：参与实体存在，事件标题全局唯一；
   - narrative：`observer` 与 `subject` 引用的事件、实体存在；
3. 使用已加载的历法，把所有时间归一化、算出可比较的刻度与展示文案；
4. 执行可计算标签，把算出的归属链并入实体的 `belong_to`；
5. 序列化成 JSON 交给前端。

任何一步失败都会阻止展示。
