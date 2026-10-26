# WeftDoc：解析与文档生命周期

`src-tauri/src/weft_doc.rs` 是文档边界与应用命令实现所在模块。解析逻辑由 YAML 字符串生成运行时 `WeftDoc`，文件绑定、保存前校验、外部修改监听和语言切换都复用这条逻辑。

```mermaid
classDiagram
    class RawWeftDoc {
        +RawStory story
        +RawEntity[] entity
        +EntityLink[] entity_link
        +RawEvent[] event
        +Narrative[] narrative
    }
    class WeftDoc {
        +Story story
        +Entity[] entity
        +BelongNode[] belong_dag
        +EntityLink[] entity_link
        +Event[] event
        +Narrative[] narrative
        +calendar_components usize?
    }
    class DocumentState {
        +path PathBuf?
        +last_written String?
        +watcher Debouncer?
    }
    class DocumentPayload {
        +doc JSON?
        +content String?
        +error DocumentError?
    }
    class Calendar
    RawWeftDoc ..> WeftDoc : validate and build
    WeftDoc *-- Story
    WeftDoc *-- Entity
    WeftDoc *-- Event
    WeftDoc *-- Narrative
    Story *-- Calendar : runtime only
    DocumentState ..> WeftDoc : parse current content
    DocumentState --> DocumentPayload : command/event response
```

## 单一解析管线

```mermaid
sequenceDiagram
    participant Caller as bind / save / watcher / language refresh
    participant Parse as parse()
    participant Raw as RawWeftDoc
    participant Story as Story::from_raw
    participant Entity as build_entities
    participant Refs as event / narrative checks
    participant Times as apply_times
    participant Frontend as 前端
    Caller->>Parse: content + YAML 所在目录 + language
    Parse->>Raw: YAML syntax + serde deserialize
    Parse->>Story: 历法与可计算属性脚本
    Story-->>Parse: Story + runtime
    Parse->>Entity: 归属、关系、实体与计算属性
    Entity-->>Parse: Entity + belong_dag
    Parse->>Refs: 事件参与者与叙事事件 ID
    Refs-->>Parse: 校验并构建领域对象
    Parse->>Times: 事件归一化、tick、文案
    Times-->>Parse: 完整 WeftDoc
    Parse-->>Frontend: JSON / error payload
```

反序列化忽略未声明的顶层键，实体未声明字段由 `flatten` 收入 `extra_props`。构建阶段依次为 Story、Entity、引用检查及事件构建；随后 `parse` 填入 `calendar_components`、计算事件时间并序列化 JSON。错误会阻断该次新文档输出。

## 文件与语言更新

- `bind_document` 读取现有文件并绑定路径，监听其父目录以覆盖原子 rename 写入方式；监听事件按目标路径过滤并做 300 ms 防抖。
- `save_document` 先通过完整解析管线校验，再原子写入临时文件并 rename。记录最近写入文本，抑制文件 watcher 对自身保存的重复回声。
- 外部变更通过 `document-changed` 推送最新内容与解析结果；解析错误也推给前端，避免旧视图被误认为新内容。
- 切换应用语言会重新读取当前文件并解析，以刷新历法与计算属性的本地化文案。
- `unbind_document` 清除绑定路径和 watcher；应用启动及关闭由 Tauri 管理。
