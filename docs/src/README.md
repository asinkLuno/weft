# Weft 开发文档

这本书记录 Weft 故事文档（WeftDoc）的数据格式与 Rune 插件接口。一个历法或 calculable prop 各对应一份 `.rn` 文件；历法负责时间的计算与展示，可计算属性按语言返回属性值。

- [数据格式](weftdoc.md)：故事 YAML 的顶层结构、id 引用与解析流程。
- [历法脚本](rune-calendar.md)：时间分量、归一化、排序和日期展示。
- [Calculable props 脚本](rune-calculable-props.md)：从实体数据计算属性值，写进 `extra_props`。
- [后端设计](design/README.md)：从 Phase 时间模型开始，说明领域对象、历法运行时与文档解析管线，并提供 UML 图。

内置历法见 `src-tauri/rune/gregorian.rn`；星座属性示例见 `examples/constellation.rn`，需要在故事中显式注册。
