# 基本约定

- 处于快速开发阶段，可以破坏性重构；不要写任何测试用例。
- 增删 Rust 依赖用 `cargo add` / `cargo remove`；永远不要手工修改 Cargo.lock。
- 前端用 daisyUI 组件类 + Tailwind 工具类，只用组件库默认样式；不写自定义 CSS 文件，样式一律写在 JSX 的 className 里。
- 不要自己启动 `tauri dev` / `tauri build`；应用由用户自己启动、目视验证。改动验证到 `cargo check` / `yarn build` / `yarn lint` 为止。
