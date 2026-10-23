# Weft 开发文档

这本书记录 Weft 故事文档（WeftDoc）的数据格式与 Rhai 插件接口。一个历法或 calculable prop 各对应一份 `.rhai` 文件；历法负责时间的计算与展示，可计算属性按语言返回属性值。

- [数据格式](weftdoc.md)：故事 YAML 的顶层结构、id 引用与解析流程。
- [历法脚本](rhai-calendar.md)：时间分量、归一化、排序和日期展示。
- [Calculable props 脚本](rhai-calculable-props.md)：从实体数据计算属性值，写进 `extra_props`。

内置历法见 `src-tauri/rhai/gregorian.rhai`；星座属性示例见 `examples/constellation.rhai`，需要在故事中显式注册。
