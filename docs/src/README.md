# Weft 开发文档

这本书记录故事文档使用的 Rhai 插件接口。一个历法或 calculable tag 各对应一份 `.rhai` 文件；计算使用与语言无关的数据，显示内容交给 `humanizer(kind, value, language)`。

- [历法脚本](rhai-calendar.md)：时间分量、归一化、排序和日期展示。
- [Calculable tags 脚本](rhai-calculable-tags.md)：根据实体数据计算归属链。

内置实现可参考仓库中的 `src-tauri/rhai/gregorian.rhai` 和 `src-tauri/rhai/constellation.rhai`。
