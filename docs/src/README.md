# Weft 开发文档

这本书记录 Weft 故事文档（WeftDoc）的数据格式与 Rhai 插件接口。一个历法或 calculable tag 各对应一份 `.rhai` 文件；计算使用与语言无关的数据，显示内容交给 `humanizer(kind, value, language)`。

- [数据格式](weftdoc.md)：故事 YAML 的顶层结构、id 引用与解析流程。
- [历法脚本](rhai-calendar.md)：时间分量、归一化、排序和日期展示。
- [Calculable tags 脚本](rhai-calculable-tags.md)：根据实体数据计算归属链。

内置历法见 `src-tauri/rhai/gregorian.rhai`；星座标签示例见 `examples/constellation.rhai`，需要在故事中显式注册。
